use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fmt::Debug;
use std::path::Path;
use std::process::{Child, Command};
use std::{fs, io};

use bytes::Buf;
use color_eyre::eyre::{bail, Context};
use daggy::petgraph;
use derive_builder::Builder;
use tracing::{debug, info, trace};

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

#[derive(Debug, Builder)]
struct Bubblewrap<'a> {
    // args: Vec<String>,
    builddir: &'a Path,
    resultdir: &'a Path,
    cmd: &'a [&'a str],
    env: &'a BTreeMap<String, String>,
}

impl Bubblewrap<'_> {
    fn run(&self) -> Result<Child> {
        let resultdir = self.resultdir.to_str().unwrap();
        let mut args = vec![
            // Build directory
            "--bind",
            self.builddir.to_str().unwrap(),
            "/build",
            "--chdir",
            "/build",
            "--setenv",
            "HOME",
            "/build",
            // Store
            "--bind",
            "/miq",
            "/miq",
            // Output directory
            "--setenv",
            "miq_out",
            resultdir,
            // Global environent
            "--dev-bind",
            "/dev",
            "/dev",
            "--proc",
            "/proc",
            "--bind",
            "/run",
            "/run",
            "--ro-bind",
            "/etc",
            "/etc",
            "--ro-bind",
            "/nix",
            "/nix",
            "--ro-bind",
            "/bin",
            "/bin",
            // No network
            "--unshare-net",
            // Set user/group
            "--unshare-user",
            "--uid",
            "0",
            "--gid",
            "0",
        ];

        for (name, value) in self.env {
            args.push("--setenv");
            args.push(name);
            args.push(value);
        }

        let mut command = Command::new("bwrap");
        command.args(args);
        command.args(self.cmd);
        trace!(?command);

        Ok(command.spawn()?)
    }
}

impl Build for Package {
    #[tracing::instrument(skip(_args, conn), ret, err, level = "info")]
    fn build(&self, _args: &Args, rebuild: bool, conn: &mut DbConnection) -> Result<MiqStorePath> {
        let store_path: MiqStorePath = (&self.result).into();
        let p: &Path = store_path.as_ref();

        if conn.is_db_path(&store_path)? {
            if rebuild {
                conn.remove(&store_path)?;
            } else {
                return Ok(store_path);
            }
        }

        let tempdir = tempfile::tempdir()?;
        let builddir = tempdir.path();

        let wrapped_cmd = ["/bin/sh", "-c", &self.script];

        let bwrap = BubblewrapBuilder::default()
            .builddir(builddir)
            .resultdir(p)
            .cmd(&wrapped_cmd)
            .env(&self.env)
            .build()?;

        let child_status = bwrap.run()?.wait()?;

        trace!(?child_status);

        if child_status.success() {
            info!(?child_status, "Build successful");
        } else {
            bail!("Bad exit: {:?}", child_status);
        };

        match p.try_exists().wrap_err("Failed to produce an output") {
            Ok(true) => {}
            Ok(false) => bail!("Output path doesn't exist: {:?}", p),
            Err(e) => bail!(e),
        }

        conn.add(&store_path)?;
        Ok(store_path)
    }
}
