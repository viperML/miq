use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::path::Path;
use std::process::Command;
use std::{fs, io};

use bytes::Buf;
use color_eyre::eyre::{bail, Context};
use daggy::petgraph;
use tracing::{debug, trace};

use crate::db::DbConnection;
use crate::eval::{MiqStorePath, RefToUnit, UnitRef};
use crate::schema_eval::{Build, Fetch, Package};
use crate::*;

#[derive(Debug, clap::Args)]
/// Build a package
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

impl crate::Main for Args {
    fn main(&self) -> Result<()> {
        let unit = self.unit_ref.ref_to_unit()?;
        let dag = eval::dag(unit)?;

        let sorted_dag = petgraph::algo::toposort(&dag, None)
            .expect("DAG was not acyclic!")
            .iter()
            .map(|&node| dag.node_weight(node).expect("Couldn't get node"))
            .collect::<Vec<_>>();

        trace!(?sorted_dag);

        // Only build last package in the chain
        let n_units = sorted_dag.len();

        let conn = &mut crate::db::DbConnection::new()?;

        for (i, unit) in sorted_dag.iter().enumerate() {
            let rebuild = self.rebuild && i == n_units - 1;
            unit.build(self, rebuild, conn)?;
        }

        Ok(())
    }
}

impl Build for Fetch {
    #[tracing::instrument(skip(_args, conn), ret, err, level = "info")]
    fn build(&self, _args: &Args, rebuild: bool, conn: &mut DbConnection) -> Result<MiqStorePath> {
        let path: MiqStorePath = (&self.result).into();

        if conn.is_db_path(&path)? {
            if rebuild {
                conn.remove(&path)?;
            } else {
                return Ok(path);
            }
        }

        let tempfile = &mut tempfile::NamedTempFile::new()?;
        debug!(?tempfile);

        let client = reqwest::blocking::Client::new();
        trace!("Fetching file, please wait");
        let response = client.get(&self.url).send()?;
        let content = &mut response.bytes()?.reader();
        std::io::copy(content, tempfile)?;

        std::fs::copy(tempfile.path(), &path)?;

        if self.executable {
            // FIXME
            debug!("Setting exec bit");
            std::process::Command::new("chmod")
                .args([OsStr::new("+x"), path.as_ref()])
                .output()?;
        }

        conn.add(&path)?;

        Ok(path)
    }
}
impl Build for Package {
    #[tracing::instrument(skip(_args, conn), ret, err, level = "info")]
    fn build(&self, _args: &Args, rebuild: bool, conn: &mut DbConnection) -> Result<MiqStorePath> {
        let path: MiqStorePath = (&self.result).into();

        if conn.is_db_path(&path)? {
            if rebuild {
                conn.remove(&path)?;
            } else {
                return Ok(path);
            }
        }

        let mut miq_env: HashMap<&OsStr, &OsStr> = HashMap::new();
        miq_env.insert(OsStr::new("miq_out"), path.as_ref());

        // FIXME
        // miq_env.insert("HOME", "/home/ayats");
        // miq_env.insert("PATH", "/var/empty");
        debug!(?miq_env);

        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", &self.script]);
        cmd.env_clear();
        cmd.envs(&self.env);
        cmd.envs(&miq_env);

        let sandbox = sandbox::SandBox {};
        sandbox.run(&mut cmd)?;

        match path.try_exists().wrap_err("Failed to produce an output") {
            Ok(true) => {}
            Ok(false) => bail!("Output path doesn't exist: {:?}", path),
            Err(e) => bail!(e),
        }

        conn.add(&path)?;

        Ok(path)
    }
}
