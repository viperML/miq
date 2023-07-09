use std::fs::OpenOptions;
use std::hash::Hash;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;

use color_eyre::eyre::{bail, ensure, Context};
use color_eyre::{Report, Result};
use daggy::petgraph::dot::{Config, Dot};
use daggy::{Dag, NodeIndex};
use schema_eval::Unit;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{instrument, span, trace};

use crate::*;

#[derive(Debug, Clone)]
pub struct Weight<T> {
    inner: T,
    visited: bool,
}

impl<T> Weight<T> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            visited: false,
        }
    }
}

type UnitNodeDag = Dag<Weight<Unit>, ()>;
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
    /// Print eval paths instead of names
    #[arg(long)]
    eval_paths: bool,
}

#[delegatable_trait]
pub trait RefToUnit {
    fn ref_to_unit(&self) -> Result<Unit>;
}

#[derive(Debug, Clone, Delegate)]
#[delegate(RefToUnit)]
/// A reference to derive a Unit from
pub enum UnitRef {
    /// Already evaluated unit.toml
    Serialized(PathBuf),
    /// Dispatch to the internal evaluator
    Lua(lua::LuaRef),
}

impl RefToUnit for PathBuf {
    fn ref_to_unit(&self) -> Result<Unit> {
        let file_contents = std::fs::read_to_string(self)?;
        let deserialized = toml::from_str(&file_contents)?;
        Ok(deserialized)
    }
}

impl FromStr for UnitRef {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("/miq/eval") {
            return Ok(Self::Serialized(PathBuf::from(s)));
        };

        if s.contains(".lua") {
            return Ok(Self::Lua(
                lua::LuaRef::from_str(s).context("Evaluating LuaRef")?,
            ));
        }

        bail!(format!("Couldn't match {} as a known UnitRef", s));
    }
}

impl crate::Main for Args {
    fn main(&self) -> Result<()> {
        let root_unit = self.unit_ref.ref_to_unit()?;

        if self.no_dag {
            return Ok(());
        };

        let (dag, _) = dag(root_unit)?;

        let node_formatter = |_, (_, weight)| graphviz_unit_format(weight, self.eval_paths);
        let dot = Dot::with_attr_getters(
            // -
            &dag,
            &[Config::EdgeNoLabel, Config::NodeNoLabel],
            &|_, _| String::new(),
            &node_formatter,
        );
        println!("{:?}", dot);

        if let Some(path) = &self.output_file {
            std::fs::write(path, format!("{:?}", dot))?;
        }

        Ok(())
    }
}

fn graphviz_unit_format(unit: &Unit, use_paths: bool) -> String {
    let pretty_name = if use_paths {
        let p = unit.result().store_path();
        format!("{}", p.to_string_lossy())
    } else {
        match unit {
            Unit::PackageUnit(inner) => {
                format!(
                    "{}",
                    inner.name,
                    // FIXME
                    // inner.version.unwrap_or(String::new())
                )
            }
            Unit::FetchUnit(inner) => {
                format!("{}", inner.name)
            }
        }
    };

    match unit {
        Unit::PackageUnit(_) => {
            format!("label = \"{}\" ", pretty_name)
        }
        Unit::FetchUnit(_) => {
            format!("label = \"{}\", shape=box, color=gray70 ", pretty_name)
        }
    }
}

const MAX_DAG_CYCLES: u32 = 20;

#[tracing::instrument(skip_all, ret, err, level = "trace")]
pub fn dag(input: Unit) -> Result<(UnitDag, NodeIndex)> {
    let mut dag = UnitNodeDag::new();
    let root_index = dag.add_node(Weight::new(input));

    let mut cycle = 0;

    while dag
        .raw_nodes()
        .iter()
        .any(|node| node.weight.visited == false)
    {
        ensure!(cycle <= MAX_DAG_CYCLES, "Maximum dag eval cycles reached");
        cycle += 1;

        add_children_recursive(&mut dag, root_index)?;
    }

    let result = dag.map(
        // format guard
        |_, node| node.inner.clone(),
        |_, _| (),
    );

    Ok((result, root_index))
}

#[instrument(skip(dag), ret, err, level = "trace")]
fn add_children_recursive(dag: &mut UnitNodeDag, index: NodeIndex) -> Result<()> {
    let Weight {
        inner: unit,
        visited,
    } = dag[index].clone();

    let _span = span!(tracing::Level::TRACE, "adding children", ?unit);
    let _enter = _span.enter();

    match unit {
        Unit::FetchUnit(_) => {
            trace!("Fetch unit, no children to add");
        }
        Unit::PackageUnit(package) => {
            for dep in &package.deps {
                let dep_unit = Unit::from_result(dep)?;

                trace!(?dep, "=> adding dep");

                let maybe_child_index: Option<NodeIndex> = dag
                    .graph()
                    .node_indices()
                    .find(|&index| &dag[index].inner == &dep_unit);

                let child_index: NodeIndex = match maybe_child_index {
                    None => {
                        let dep_unit_node = Weight::new(dep_unit);
                        let (_, child_index) = dag.add_child(index, (), dep_unit_node);
                        child_index
                    }
                    Some(child_index) => {
                        if !visited {
                            dag.add_edge(index, child_index, ())?;
                        }
                        child_index
                    }
                };

                add_children_recursive(dag, child_index)?;
            }
        }
    }

    dag[index].visited = true;

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

impl Unit {
    pub fn result<'s>(&'s self) -> &'s MiqResult {
        match self {
            Unit::PackageUnit(inner) => &inner.result,
            Unit::FetchUnit(inner) => &inner.result,
        }
    }
}

impl Unit {
    pub fn from_result(result: &MiqResult) -> Result<Self> {
        let path = result.eval_path();
        let raw_text = std::fs::read_to_string(&path.as_path())
            .wrap_err(format!("Reading eval path {:?}", path))?;
        let result = toml::from_str(&raw_text)?;
        Ok(result)
    }
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq, JsonSchema, Default, Educe)]
#[educe(Deref)]
pub struct MiqEvalPath(PathBuf);

impl MiqResult {
    pub fn eval_path<'s>(&'s self) -> MiqEvalPath {
        let path_str = ["/miq/eval/", &self.0, ".toml"].join("");
        MiqEvalPath(PathBuf::from(path_str))
    }
}

#[test]
fn test_get_evalpath() {
    let input = MiqResult("hello-world-AAAA".into());
    let output = input.eval_path();
    let output_expected = MiqEvalPath("/miq/eval/hello-world-AAAA.toml".into());
    assert_eq!(output, output_expected);
}

impl AsRef<Path> for MiqEvalPath {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq, JsonSchema, Default, Educe)]
#[educe(Deref)]
pub struct MiqStorePath(PathBuf);

impl MiqResult {
    pub fn store_path<'s>(&'s self) -> MiqStorePath {
        let path_str = ["/miq/store/", &self.0].join("");
        MiqStorePath(PathBuf::from(path_str))
    }
}

#[test]
fn test_get_storepath() {
    let input = MiqResult("hello-world-AAAA".into());
    let output = input.store_path();
    let output_expected = MiqStorePath("/miq/store/hello-world-AAAA".into());
    assert_eq!(output, output_expected);
}

impl AsRef<Path> for MiqStorePath {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl Unit {
    pub fn write_to_disk(&self) -> Result<()> {
        let header = "#:schema /miq/eval-schema.json";
        let serialized = toml::to_string_pretty(self)?;
        let eval_path = self.result().eval_path();

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&eval_path.as_path())
            .wrap_err(format!("Opening serialisation file for {:?}", eval_path))?;

        file.write_all(header.as_bytes())?;
        file.write_all("\n".as_bytes())?;
        file.write_all(serialized.as_bytes())?;

        Ok(())
    }
}
