use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

use bytes::Buf;
use daggy::petgraph;
use tracing::{debug, trace};

use crate::schema_eval::{Fetch, Package, Unit};
use crate::*;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Path of the buildable
    #[arg()]
    file: PathBuf,

    /// Don't show build output
    #[arg(long, short)]
    quiet: bool,

    /// Rebuild path even if it already exists
    #[arg(long, short)]
    rebuild: bool,
}

pub fn clean_path<P: AsRef<Path> + Debug>(path: P) -> io::Result<()> {
    trace!("Requesting clean path on {:?}", path);

    match fs::metadata(&path) {
        Ok(meta) => {
            trace!("Elem exists, removing");
            if meta.is_file() {
                fs::remove_file(&path)?;
            } else if meta.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                panic!("{:?} Wasn't either a dir or a file", path);
            }
            Ok(())
        }
        Err(err) => match err.kind() {
            io::ErrorKind::NotFound => {
                trace!("Doesn't exist, skipping");
                Ok(())
            }
            _ => Err(err),
        },
    }
}

impl Args {
    pub fn main(&self) -> Result<()> {
        let result_dag = dag::evaluate_dag(&self.file)?;

        let sorted_dag = petgraph::algo::toposort(&result_dag, None)
            .expect("DAG was not acyclic!")
            .iter()
            .map(|&node| result_dag.node_weight(node).expect("Couldn't get node"))
            .collect::<Vec<_>>();

        trace!(?sorted_dag);

        // Only build last package in the chain
        let n_units = sorted_dag.len();
        for (i, unit) in sorted_dag.iter().enumerate() {
            let rebuild = self.rebuild && i == n_units - 1;
            match unit {
                Unit::PackageUnit(inner) => build_package(inner, self, rebuild),
                Unit::FetchUnit(inner) => build_fetch(inner, self, rebuild),
            }?;
        }

        Ok(())
    }
}

#[tracing::instrument(skip(build_args), ret, level = "info")]
fn build_fetch(input: &Fetch, build_args: &Args, rebuild: bool) -> Result<()> {
    let path = format!("/miq/store/{}", input.result);

    if db::is_db_path(&path)? {
        if rebuild {
            db::remove(&path)?;
        } else {
            return Ok(());
        }
    }

    let tempfile = &mut tempfile::NamedTempFile::new()?;
    debug!(?tempfile);

    let client = reqwest::blocking::Client::new();
    let response = client.get(&input.url).send()?;
    let content = &mut response.bytes()?.reader();
    std::io::copy(content, tempfile)?;

    std::fs::copy(tempfile.path(), &path)?;

    db::add(&path)?;

    Ok(())
}

#[tracing::instrument(skip(build_args), ret, level = "info")]
fn build_package(input: &Package, build_args: &Args, rebuild: bool) -> Result<()> {
    let path = format!("/miq/store/{}", input.result);

    if db::is_db_path(&path)? {
        if rebuild {
            db::remove(&path)?;
        } else {
            return Ok(());
        }
    }

    let mut miq_env: HashMap<&str, &str> = HashMap::new();
    miq_env.insert("miq_out", &path);

    // FIXME
    miq_env.insert("HOME", "/home/ayats");
    debug!(?miq_env);

    let mut cmd = Command::new("/bin/sh");
    cmd.args(["-c", &input.script]);
    cmd.env_clear();
    cmd.envs(&input.env);
    cmd.envs(&miq_env);

    let sandbox = sandbox::SandBox {};
    sandbox.run(&mut cmd)?;

    db::add(&path)?;

    Ok(())
}
