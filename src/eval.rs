use std::path::Path;
use std::str::FromStr;

use color_eyre::eyre::{bail, ContextCompat, Context};
use color_eyre::{Report, Result};
use daggy::petgraph::algo::toposort;
use daggy::petgraph::dot::{Config, Dot};
use daggy::petgraph::visit::Topo;
use daggy::{petgraph, Dag, NodeIndex, Walker};
use schema_eval::Unit;
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
pub struct Args {
    /// Unitref to evaluate
    #[clap(value_parser = clap::value_parser!(UnitRef))]
    unit_ref: UnitRef,
    /// Write the resulting graph to this file
    #[arg(short, long)]
    output_file: Option<PathBuf>,
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

        let result = match &*components {
            &[path] => {
                if path.starts_with("/miq/eval") {
                    Self::Serialized(path.into())
                } else {
                    bail!("Input is not a valid UnitRef")
                }
            }
            &[path, element] => Self::Lua(LuaRef {
                root: path.into(),
                element: element.into(),
            }),
            _ => bail!("Input is not a valid UnitRef"),
        };
        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub struct LuaRef {
    root: PathBuf,
    element: String,
}

pub fn dispatch(unit_ref: &UnitRef) -> Result<String> {
    let result: String = match unit_ref {
        UnitRef::Serialized(inner) => inner
            .to_str()
            .unwrap()
            .replace("/miq/eval/", "")
            .replace(".toml", ""),

        // Dispatch to inner lua evaluator
        UnitRef::Lua(inner) => {
            let toplevel = crate::lua::evaluate(&inner.root)?;
            let selected_unit = toplevel
                .get(&inner.element)
                .context(format!("Getting element {:?}", inner.element))
                .context("Unit wasn't found")?
                .clone();
            selected_unit.result()
        }
    };

    Ok(result)
}

impl Args {
    pub fn main(&self) -> Result<()> {
        let result = dispatch(&self.unit_ref)?;
        info!(?result);

        let dag = dag(result)?;

        let dot = Dot::with_attr_getters(
            // -
            &dag,
            &[Config::EdgeNoLabel, Config::NodeNoLabel],
            &|_, _| String::new(),
            &|_, (_, weight)| match weight {
                Unit::PackageUnit(inner) => {
                    format!("label = \"{}-{}\" ", inner.name, inner.version)
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

#[tracing::instrument(skip_all, ret, level = "debug")]
pub fn dag<P: AsRef<str> + std::fmt::Debug>(input: P) -> Result<UnitDag> {
    let path = format!("/miq/eval/{}.toml", input.as_ref());

    let mut dag = UnitNodeDag::new();
    let root_n_weight: Unit = toml::from_str(&std::fs::read_to_string(path)?)?;
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

                    let target = Unit::from_result(elem)?;

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
