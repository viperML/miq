use std::path::Path;

use crate::*;
use color_eyre::Result;
use daggy::petgraph::dot::{Config, Dot};
use daggy::petgraph::visit::Topo;
use daggy::{petgraph, Walker};
use daggy::{Dag, NodeIndex};
use schema_eval::Unit;
use tracing::{info, warn};
use tracing_subscriber::fmt::format;

#[derive(Debug, Clone)]
struct UnitNode {
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

type UnitDag = Dag<UnitNode, ()>;

#[derive(Debug, clap::Args)]
pub struct Args {
    path: PathBuf,
}

impl Args {
    pub fn main(&self) -> Result<()> {
        evaluate_dag(&self.path)?;
        Ok(())
    }
}

pub fn evaluate_dag<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();

    let mut dag = UnitDag::new();
    let root_n_weight: Unit = toml::from_str(&std::fs::read_to_string(path)?)?;
    let root_n_weight = UnitNode::new(root_n_weight);
    let root_n = dag.add_node(root_n_weight);

    info!(?dag);

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

        info!(?size);

        cycle = cycle + 1;
    }

    // println!("{:?}", Dot::with_config(&dag, &[Config::EdgeNoLabel],));
    let result = Dot::with_attr_getters(
        // -
        &dag,
        &[Config::EdgeNoLabel, Config::NodeNoLabel],
        &|_, _| String::new(),
        &|_, (_, weight)| match &weight.inner {
            Unit::Package(inner) => format!("label = \"{}\"", inner.result),
            Unit::Fetch(inner) => format!("label = \"{}\"", inner.result),
        },
    );
    println!("{:?}", result);

    Ok(())
}

fn cycle_dag(dag: &mut UnitDag, node: NodeIndex) -> Result<()> {
    let old_dag = dag.clone();
    info!("Cycling at node {:?}", old_dag[node]);
    let node_weight = old_dag.node_weight(node).unwrap();

    if !dag[node].visited {
        dag[node].visit();

        match &node_weight.inner {
            Unit::Package(inner) => {
                for elem in &inner.deps {
                    info!("I want to create {:?}", elem);

                    let target = Unit::from_result(elem)?;

                    for parent in Topo::new(&old_dag).iter(&old_dag) {
                        let p = &old_dag[parent].inner;
                        warn!("Examining G component {:?}", p);

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
            Unit::Fetch(_inner) => {}
        }
    }

    for (_, n) in old_dag.parents(node).iter(&old_dag) {
        cycle_dag(dag, n)?;
    }

    Ok(())
}
