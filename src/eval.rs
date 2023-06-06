use std::ffi::OsStr;
use std::hash::Hash;
use std::path::Path;
use std::str::FromStr;

use color_eyre::eyre::{bail, Context, ContextCompat};
use color_eyre::{Report, Result};
use daggy::petgraph::algo::toposort;
use daggy::petgraph::dot::{Config, Dot};
use daggy::petgraph::visit::Topo;
use daggy::{petgraph, Dag, NodeIndex, Walker};
use schema_eval::Unit;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument, trace};

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
pub struct Args {
    /// Unitref to evaluate
    #[clap(value_parser = clap::value_parser!(UnitRef))]
    unit_ref: UnitRef,
    /// Write the resulting graph to this file
    #[arg(short, long)]
    output_file: Option<PathBuf>,
    #[arg(short, long)]
    no_dag: bool,
}

#[derive(Debug, Clone)]
pub enum UnitRef {
    /// Already evaluated unit.toml
    Serialized(PathBuf),
    /// Dispatch to the internal evaluator
    Lua(LuaRef),
}

impl FromStr for UnitRef {
    type Err = Report;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let components: Vec<_> = s.split('#').collect();

        let result = match *components {
            [path] => {
                if path.starts_with("/miq/eval") {
                    Self::Serialized(path.into())
                } else {
                    bail!("Input is not a valid UnitRef")
                }
            }
            [path, element] => Self::Lua(LuaRef {
                main: path.into(),
                element: element.into(),
            }),
            _ => bail!("Input is not a valid UnitRef"),
        };
        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub struct LuaRef {
    /// Luafile to evaluate
    main: PathBuf,
    /// element to evaluate
    element: String,
}

#[instrument(ret, err, level = "info")]
pub fn dispatch(unit_ref: &UnitRef) -> Result<MiqResult> {
    let result = match unit_ref {
        UnitRef::Serialized(_inner) => {
            todo!("Accept serialized unitefs");
        }

        // Dispatch to inner lua evaluator
        UnitRef::Lua(inner) => {
            // let toplevel = crate::lua::evaluate(&inner.root)?;
            let toplevel = crate::lua::evaluate(&inner.main)?;
            let selected_unit = toplevel
                .get(&inner.element)
                .context(format!("Getting element {:?}", inner.element))
                .context("Unit wasn't found")?
                .clone();

            selected_unit.into()
        }
    };

    Ok(result)
}

impl crate::Main for Args {
    fn main(&self) -> Result<()> {
        let result = dispatch(&self.unit_ref)?;
        info!(?result);

        if self.no_dag {
            return Ok(());
        };

        let dag = dag(&result)?;

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

#[tracing::instrument(skip_all, ret, err, level = "debug")]
pub fn dag(input: &MiqResult) -> Result<UnitDag> {
    let mut dag = UnitNodeDag::new();
    let root_n_weight: Unit = input.try_into()?;
    let root_n_weight = UnitNode::new(root_n_weight);
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
    Debug, Clone, Hash, Serialize, Deserialize, PartialEq, JsonSchema, Default, PartialOrd, Eq, Ord,
)]
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
        let raw_text = std::fs::read_to_string(path)?;
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
