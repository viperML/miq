use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::hash::Hash;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;

use color_eyre::eyre::{bail, Context};
use color_eyre::{Report, Result};
use daggy::petgraph::algo::toposort;
use daggy::petgraph::dot::{Config, Dot};
use daggy::petgraph::visit::Topo;
use daggy::{petgraph, Dag, NodeIndex, Walker};
use schema_eval::Unit;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{info, trace};

use crate::*;

#[derive(Debug, Clone)]
pub struct UnitNode {
    inner: Unit,
    visited: bool,
}

impl UnitNode {
    fn new(inner: Unit) -> Self {
        Self {
            inner,
            visited: false,
        }
    }

    fn visit(&mut self) {
        self.visited = true;
    }
}

type UnitNodeDag = Dag<UnitNode, ()>;
type UnitDag = Dag<Unit, ()>;

#[derive(Debug, clap::Args)]
/// Evaluate packages
pub struct Args {
    /// Unitref to evaluate
    // #[clap(value_parser = clap::value_parser!(UnitRef))]
    unit_ref: UnitRef,
    /// Write the resulting graph to this file
    #[arg(short, long)]
    output_file: Option<PathBuf>,
    #[arg(short, long)]
    no_dag: bool,
}

#[delegatable_trait]
pub trait RefToUnit {
    fn ref_to_unit(&self) -> Result<Unit>;
}

#[derive(Debug, Clone, Delegate)]
#[delegate(RefToUnit)]
pub enum UnitRef {
    /// Already evaluated unit.toml
    Serialized(PathBuf),
    /// Dispatch to the internal evaluator
    Lua(lua::LuaRef),
}

impl RefToUnit for PathBuf {
    fn ref_to_unit(&self) -> Result<Unit> {
        todo!();
    }
}

impl FromStr for UnitRef {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("/miq/eval") {
            return Ok(Self::Serialized(PathBuf::from(s)));
        };

        if s.contains("#") {
            return Ok(Self::Lua(lua::LuaRef::new(s)?));
        }

        bail!(format!("Couldn't match {} as a valid UnitRef", s));
    }
}

impl crate::Main for Args {
    fn main(&self) -> Result<()> {
        let root_unit = self.unit_ref.ref_to_unit()?;

        if self.no_dag {
            return Ok(());
        };

        let dag = dag(root_unit)?;

        let dot = Dot::with_attr_getters(
            // -
            &dag,
            &[Config::EdgeNoLabel, Config::NodeNoLabel],
            &|_, _| String::new(),
            &|_, (_, weight)| match weight {
                Unit::PackageUnit(inner) => {
                    if let Some(ref v) = inner.version {
                        format!("label = \"{}-{}\" ", inner.name, v)
                    } else {
                        format!("label = \"{}\" ", inner.name)
                    }
                }
                Unit::FetchUnit(inner) => format!("label = \"{}\" ", inner.name),
            },
        );
        println!("{:?}", dot);

        if let Some(path) = &self.output_file {
            std::fs::write(path, format!("{:?}", dot))?;
        }

        let schedule = toposort(&dag, None).expect("Couldn't sort dag");
        let schedule: Vec<_> = schedule
            .iter()
            .map(|&n| dag.node_weight(n).unwrap())
            .collect();
        info!(?schedule);

        Ok(())
    }
}

#[tracing::instrument(skip_all, ret, err, level = "trace")]
pub fn dag(input: Unit) -> Result<UnitDag> {
    let mut dag = UnitNodeDag::new();
    // let root_n_weight: Unit = input.try_into()?;
    let root_n_weight = UnitNode::new(input);
    let root_n = dag.add_node(root_n_weight);

    let max_cycles = 10;
    let mut cycle = 0;
    let mut size: usize = 1;

    while size > 0 && cycle <= max_cycles {
        let old_dag = dag.clone();
        let search = petgraph::visit::Dfs::new(&old_dag, root_n);

        cycle_dag(&mut dag, root_n)?;

        size = search
            .iter(&old_dag)
            .fold(0, |acc, n| if !old_dag[n].visited { acc + 1 } else { acc });

        trace!(?size);

        cycle += 1;
    }

    let result = dag.map(
        // -
        |_, unit_node| unit_node.inner.to_owned(),
        |_, _| (),
    );

    Ok(result)
}

fn cycle_dag(dag: &mut UnitNodeDag, node: NodeIndex) -> Result<()> {
    let old_dag = dag.clone();
    trace!("Cycling at node {:?}", old_dag[node]);
    let node_weight = old_dag.node_weight(node).unwrap();

    if !dag[node].visited {
        dag[node].visit();

        match &node_weight.inner {
            Unit::PackageUnit(inner) => {
                for elem in &inner.deps {
                    trace!("I want to create {:?}", elem);

                    // let target = Unit::from_result(elem)?;
                    // let target = elem.read_unit()?;
                    let target: Unit = elem.try_into()?;

                    for parent in Topo::new(&old_dag).iter(&old_dag) {
                        let p = &old_dag[parent].inner;
                        trace!("Examining G component {:?}", p);

                        if p == &target.clone() {
                            dag.add_edge(parent, node, ())?;

                            return Ok(());
                        }
                    }

                    dag.add_parent(
                        node,
                        (),
                        UnitNode {
                            inner: target,
                            visited: false,
                        },
                    );
                }
            }
            Unit::FetchUnit(_inner) => {}
        }
    }

    for (_, n) in old_dag.parents(node).iter(&old_dag) {
        cycle_dag(dag, n)?;
    }

    Ok(())
}

#[derive(
    Debug,
    Clone,
    Hash,
    Serialize,
    Deserialize,
    PartialEq,
    JsonSchema,
    Default,
    PartialOrd,
    Eq,
    Ord,
    Educe,
)]
#[educe(Deref)]
pub struct MiqResult(String);

impl MiqResult {
    pub fn create<H: Hash>(name: &str, hashable: &H) -> MiqResult {
        let mut hasher = fnv::FnvHasher::default();
        hashable.hash(&mut hasher);
        let hash_result = std::hash::Hasher::finish(&hasher);
        let hash_string = format!("{:x}", hash_result);
        MiqResult(format!("{}-{}", name, hash_string))
    }
}

impl From<Unit> for MiqResult {
    fn from(value: Unit) -> Self {
        match value {
            Unit::PackageUnit(inner) => inner.result,
            Unit::FetchUnit(inner) => inner.result,
        }
    }
}

impl TryFrom<&MiqResult> for Unit {
    type Error = Report;

    fn try_from(value: &MiqResult) -> std::result::Result<Self, Self::Error> {
        let path: MiqEvalPath = value.into();
        trace!(?path);
        let raw_text =
            std::fs::read_to_string(&path).wrap_err(format!("Reading eval path {:?}", path))?;
        let result = toml::from_str(&raw_text)?;
        Ok(result)
    }
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq, JsonSchema, Default)]
pub struct MiqEvalPath(PathBuf);

impl From<&MiqResult> for MiqEvalPath {
    fn from(value: &MiqResult) -> Self {
        let path_str = &["/miq/eval/", &value.0, ".toml"].join("");
        MiqEvalPath(PathBuf::from(path_str))
    }
}

#[test]
fn test_get_evalpath() {
    let input = MiqResult("hello-world-AAAA".into());
    let output: MiqEvalPath = (&input).into();
    let output_expected = MiqEvalPath("/miq/eval/hello-world-AAAA.toml".into());
    assert_eq!(output, output_expected);
}

impl AsRef<Path> for MiqEvalPath {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq, JsonSchema, Default)]
pub struct MiqStorePath(PathBuf);

impl From<&MiqResult> for MiqStorePath {
    fn from(value: &MiqResult) -> Self {
        let path_str = &["/miq/store/", &value.0].join("");
        MiqStorePath(PathBuf::from(path_str))
    }
}
#[test]
fn test_get_storepath() {
    let input = MiqResult("hello-world-AAAA".into());
    let output: MiqStorePath = (&input).into();
    let output_expected = MiqStorePath("/miq/store/hello-world-AAAA".into());
    assert_eq!(output, output_expected);
}

impl AsRef<Path> for MiqStorePath {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for MiqStorePath {
    fn as_ref(&self) -> &OsStr {
        self.0.as_os_str()
    }
}

impl MiqStorePath {
    pub fn try_exists(&self) -> std::io::Result<bool> {
        self.0.try_exists()
    }
}

impl Unit {
    pub fn write_to_disk(&self) -> Result<()> {
        let prefix = "#:schema /miq/eval-schema.json";
        let serialized = toml::to_string_pretty(self)?;
        let result: MiqResult = MiqResult::from(self.clone());
        let eval_path: MiqEvalPath = (&result).into();

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&eval_path)
            .wrap_err(format!("Opening serialisation file for {:?}", eval_path))?;

        file.write_all(prefix.as_bytes())?;
        file.write_all("\n".as_bytes())?;
        file.write_all(serialized.as_bytes())?;

        Ok(())
    }
}
