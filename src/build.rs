use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt::Debug;
use std::fs::File;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

use bytes::Buf;
use color_eyre::eyre::{bail, Context};
use daggy::petgraph;
use tracing::{debug, trace};

use crate::eval::{MiqStorePath, UnitRef};
use crate::schema_eval::{Fetch, Package, Unit};
use crate::*;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Unitref to build
    unit_ref: UnitRef,

    /// Don't show build output
    #[arg(long, short)]
    quiet: bool,

    /// Rebuild even if it already exists
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
        let result = eval::dispatch(&self.unit_ref)?;
        let dag = eval::dag(&result)?;

        let sorted_dag = petgraph::algo::toposort(&dag, None)
            .expect("DAG was not acyclic!")
            .iter()
            .map(|&node| dag.node_weight(node).expect("Couldn't get node"))
            .collect::<Vec<_>>();

        trace!(?sorted_dag);

        // Only build last package in the chain
        let n_units = sorted_dag.len();
        for (i, unit) in sorted_dag.iter().enumerate() {
            let rebuild = self.rebuild && i == n_units - 1;
            match unit {
                Unit::PackageUnit(inner) => {
                    build_package(inner, self, rebuild)?;
                }
                Unit::FetchUnit(inner) => {
                    build_fetch(inner, self, rebuild)?;
                }
            };
        }

        Ok(())
    }
}

#[tracing::instrument(skip(_build_args), ret, level = "info")]
fn build_fetch(input: &Fetch, _build_args: &Args, rebuild: bool) -> Result<()> {
    let path: MiqStorePath = (&input.result).into();

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
    trace!("Fetching file, please wait");
    let response = client.get(&input.url).send()?;
    let content = &mut response.bytes()?.reader();
    std::io::copy(content, tempfile)?;

    std::fs::copy(tempfile.path(), &path)?;

    if input.executable {
        // FIXME
        debug!("Setting exec bit");
        std::process::Command::new("chmod")
            .args([OsStr::new("+x"), path.as_ref()])
            .output()?;
    }

    db::add(&path)?;

    Ok(())
}

#[tracing::instrument(skip(_build_args), ret, level = "info")]
fn build_package(input: &Package, _build_args: &Args, rebuild: bool) -> Result<()> {
    let path: MiqStorePath = (&input.result).into();

    if db::is_db_path(&path)? {
        if rebuild {
            db::remove(&path)?;
        } else {
            return Ok(());
        }
    }

    let mut miq_env: HashMap<&OsStr, &OsStr> = HashMap::new();
    miq_env.insert(OsStr::new("miq_out"), path.as_ref());

    // FIXME
    // miq_env.insert("HOME", "/home/ayats");
    // miq_env.insert("PATH", "/var/empty");
    debug!(?miq_env);

    let mut cmd = Command::new("/bin/sh");
    cmd.args([OsStr::new("-c"), &input.script]);
    cmd.env_clear();
    cmd.envs(&input.env);
    cmd.envs(&miq_env);

    let sandbox = sandbox::SandBox {};
    sandbox.run(&mut cmd)?;

    match path.try_exists().wrap_err("Failed to produce an output") {
        Ok(true) => {}
        Ok(false) => bail!("Output path doesn't exist: {:?}", path),
        Err(e) => bail!(e),
    }

    db::add(&path)?;

    Ok(())
}
