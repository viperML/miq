use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{self, Path, PathBuf};
use std::str::FromStr;
use std::{fs, io, vec};

use bytes::Buf;
use color_eyre::eyre::bail;
use daggy::petgraph;
use tempfile::tempfile;
use tracing::{debug, info, warn};
use tracing_subscriber::fmt::format;

use std::process::Command;

use crate::schema_eval::{self, Fetch, Package, Unit};
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
    debug!("Requesting clean path on {:?}", path);

    match fs::metadata(&path) {
        Ok(meta) => {
            debug!("Elem exists, removing");
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
                debug!("Doesn't exist, skipping");
                Ok(())
            }
            _ => Err(err),
        },
    }
}

impl Args {
    pub fn main(&self) -> Result<()> {
        let result_dag = dag::evaluate_dag(&self.file)?;

        info!(?result_dag);

        let result = petgraph::algo::toposort(&result_dag, None)
            .expect("DAG was not acyclic!")
            .iter()
            .map(|&node| result_dag.node_weight(node).expect("Couldn't get node"))
            .collect::<Vec<_>>();

        info!(?result);

        for unit in result {
            build_unit(unit, self)?;
        }

        Ok(())
    }
}

fn build_unit(unit: &Unit, args: &Args) -> Result<()> {
    match unit {
        Unit::Package(inner) => build_package(inner, args),
        Unit::Fetch(inner) => build_fetch(inner),
    }?;

    Ok(())
}

fn build_fetch(input: &Fetch) -> Result<()> {
    // debug!("Fetching: {:?}", input);
    let path = format!("/miq/store/{}", input.result);

    let already_fetched = db::is_db_path(&path)?;

    if already_fetched {
        debug!("Already fetched!");
        return Ok(());
    }

    let tempfile = &mut tempfile::NamedTempFile::new()?;
    // FIXME
    // let tempfile = RefCell::new(tempfile::NamedTempFile::new());
    debug!("tempfile: {:?}", &tempfile);

    let client = reqwest::blocking::Client::new();
    let response = client.get(&input.url).send()?;
    let content = &mut response.bytes()?.reader();
    std::io::copy(content, tempfile)?;

    debug!("Fetch Ok");

    std::fs::copy(tempfile.path(), &path)?;
    debug!("Move OK");

    // Make sure we don't drop before
    // drop(tempfile);

    db::add(&path)?;

    Ok(())
}

fn build_package(input: &Package, build_args: &Args) -> Result<()> {
    let path = format!("/miq/store/{}", input.result);

    if db::is_db_path(&path)? {
        if build_args.rebuild {
            debug!("Rebuilding pkg, unregistering from the store");
            db::remove(&path)?;
        } else {
            debug!("Package was already built");
            return Ok(());
        }
    }

    let mut miq_env: HashMap<&str, &str> = HashMap::new();
    miq_env.insert("miq_out", &path);

    // FIXME
    miq_env.insert("HOME", "/home/ayats");
    debug!("env: {:?}", miq_env);

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
