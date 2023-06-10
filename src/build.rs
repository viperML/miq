use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fmt::Debug;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fs, io};

use async_trait::async_trait;
use bytes::Buf;
use color_eyre::eyre::{bail, Context};
use daggy::petgraph::visit::Dfs;
use daggy::{petgraph, Walker};
use derive_builder::Builder;
use futures::stream::futures_unordered;
use futures::TryStreamExt;
use tokio::process::Command;
use tracing::{debug, info, span, trace, Level};

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
        tokio::runtime::Runtime::new()?.block_on(self._main())
    }
}

#[derive(Debug, Default)]
struct BuildTask {
    buildable: bool,
    building: bool,
    built: bool,
}

impl Args {
    async fn _main(&self) -> Result<()> {
        let root_unit = self.unit_ref.ref_to_unit()?;
        let (dag, root_node) = eval::dag(root_unit)?;

        let mut graph_reversed = dag.graph().clone();
        graph_reversed.reverse();

        let db_conn = Arc::new(Mutex::new(crate::db::DbConnection::new()?));

        let mut build_tasks: HashMap<daggy::NodeIndex, BuildTask> = HashMap::new();
        let mut futs = futures_unordered::FuturesUnordered::new();

        let mut sentry = 0;

        loop {
            sentry = sentry + 1;

            let mut search = Dfs::new(&graph_reversed, root_node);
            let mut all_built = true;

            while let Some(index) = search.next(&graph_reversed) {
                let node = dag.node_weight(index).unwrap();
                let t = build_tasks.get(&index);
                let span = span!(Level::TRACE, "Graph walk", ?node, ?index);
                let _enter = span.enter();
                trace!(?t);

                if let Some(t) = t {
                    if t.building || t.built {
                        continue;
                    }
                }

                let mut task = BuildTask::default();
                task.buildable = true;

                let mut all_parents_built = true;
                for (_, parent_index) in dag.parents(index).iter(&dag) {
                    let parent = &dag[parent_index];
                    trace!(?parent);
                    let parent_task = build_tasks.get(&parent_index);
                    trace!(?parent_task, ?parent_index);
                    match parent_task {
                        None => {
                            all_parents_built = false;
                        }
                        Some(parent_task) => {
                            // trace!(?index, "Setting as buildable");
                            // task.buildable = !parent_task.built;
                            if !parent_task.built {
                                all_parents_built = false;
                            }
                        }
                    }
                }

                task.buildable = all_parents_built;

                trace!(?task, ?all_parents_built, "Inserting task to map");
                build_tasks.insert(index, task);
            }

            for (k, task) in build_tasks.iter_mut() {
                let unit = &dag[*k];
                // trace!(?unit, ?task, "Checking state of task");
                let conn = db_conn.clone();
                let unit = unit.clone();
                let k = k.clone();

                if !task.built {
                    all_built = false;
                }

                if task.buildable && !task.built && !task.building {
                    all_built = false;
                    let fut = tokio::spawn(async move {
                        trace!(?unit, "Starting build task");
                        (unit.build(false, &conn).await, k)
                    });
                    futs.push(fut);
                    task.building = true;
                }
            }

            while let Ok(Some((task_result, n))) = futs.try_next().await {
                let task_result = task_result?;
                debug!(?task_result, ?n, "Task finished");
                let t = build_tasks.get_mut(&n).unwrap();
                t.built = true;
                // tokio::time::sleep(Duration::from_secs(1)).await;
            }

            if all_built {
                break;
            }

            if sentry > 3 {
                bail!("Sentry reached!");
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Build for Fetch {
    #[tracing::instrument(skip(conn), ret, err, level = "info")]
    async fn build(&self, rebuild: bool, conn: &Mutex<DbConnection>) -> Result<MiqStorePath> {
        let path: MiqStorePath = (&self.result).into();

        if conn.lock().unwrap().is_db_path(&path)? {
            if rebuild {
                conn.lock().unwrap().remove(&path)?;
            } else {
                return Ok(path);
            }
        }

        let tempfile = &mut tempfile::NamedTempFile::new()?;
        debug!(?tempfile);

        let client = reqwest::Client::new();
        trace!("Fetching file, please wait");
        let response = client.get(&self.url).send().await?;
        let content = &mut response.bytes().await?.reader();
        std::io::copy(content, tempfile)?;

        std::fs::copy(tempfile.path(), &path)?;

        if self.executable {
            // FIXME
            debug!("Setting exec bit");
            std::process::Command::new("chmod")
                .args([OsStr::new("+x"), path.as_ref()])
                .output()?;
        }

        // FIXME
        conn.lock().unwrap().add(&path)?;

        Ok(path)
    }
}

#[async_trait]
impl Build for Package {
    #[tracing::instrument(skip(conn), ret, err, level = "info")]
    async fn build(&self, rebuild: bool, conn: &Mutex<DbConnection>) -> Result<MiqStorePath> {
        let store_path: MiqStorePath = (&self.result).into();
        let p: &Path = store_path.as_ref();

        if conn.lock().unwrap().is_db_path(&store_path)? {
            if rebuild {
                conn.lock().unwrap().remove(&store_path)?;
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

        let child_status = bwrap
            .build_command()?
            .stdout(Stdio::null())
            .spawn()?
            .wait()
            .await?;

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

        // Fixme
        conn.lock().unwrap().add(&store_path)?;
        Ok(store_path)
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
    fn build_command(&self) -> Result<Command> {
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
        Ok(command)
    }
}
