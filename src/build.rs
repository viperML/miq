use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fmt::Debug;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::{fs, io};

use async_trait::async_trait;
use bytes::Buf;
use color_eyre::eyre::{bail, ensure, Context};
use daggy::petgraph::visit::Dfs;
use daggy::Walker;
use derive_builder::Builder;
use futures::stream::futures_unordered;
use futures::TryStreamExt;
use tokio::process::Command;
use tracing::{debug, info, span, trace, Level};

use crate::db::DbConnection;
use crate::eval::{MiqStorePath, RefToUnit, UnitRef};
use crate::schema_eval::{Build, Fetch, Package, Unit};
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

    /// Maximum number of concurrent build jobs. Fetch jobs are parallelized automatically.
    #[arg(long, short, default_value = "1")]
    jobs: usize,
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

#[derive(Debug, PartialEq, Eq)]
enum BuildTask {
    Pending,
    Building,
    Finished,
}

impl Args {
    async fn _main(&self) -> Result<()> {
        trace!("Starting async");
        let root_node = self.unit_ref.ref_to_unit()?;
        let (dag, root_index) = eval::dag(root_node.clone())?;

        // There is no reverse search algo, so use regular search on reversed graph
        let graph_reversed = {
            let mut g = dag.graph().clone();
            g.reverse();
            g
        };

        let db_conn = Arc::new(Mutex::new(crate::db::DbConnection::new()?));

        let mut build_tasks: HashMap<&Unit, BuildTask> = HashMap::new();
        let mut futs = futures_unordered::FuturesUnordered::new();

        let mut sentry = 0;

        build_tasks.insert(&root_node, BuildTask::Pending);

        while !build_tasks.iter().all(|(_, task)| match task {
            BuildTask::Finished => true,
            _ => false,
        }) {
            // Avoid blowing up
            sentry = sentry + 1;
            ensure!(
                sentry <= 10,
                "Sentry reached, something might have gone wrong!"
            );

            let mut graph_search = Dfs::new(&graph_reversed, root_index);

            while let Some(index) = graph_search.next(&graph_reversed) {
                let unit = &dag[index];
                let span = span!(Level::TRACE, "Graph walk", ?unit, ?index);
                let _enter = span.enter();

                let existing_task = build_tasks.get(&unit);
                trace!(?existing_task);
                match existing_task {
                    None | Some(BuildTask::Pending) => {}
                    _ => continue,
                };

                let mut task = BuildTask::Pending;

                let all_parents_built = dag.parents(index).iter(&dag).all(|(_, parent_index)| {
                    match &build_tasks.get(&dag[parent_index]) {
                        Some(BuildTask::Finished) => true,
                        _ => false,
                    }
                });

                let number_packages_building = build_tasks
                    .iter()
                    .filter(|(unit, _)| match unit {
                        Unit::PackageUnit(_) => true,
                        _ => false,
                    })
                    .filter(|(_, task)| match task {
                        BuildTask::Building => true,
                        _ => false,
                    })
                    .count();

                let can_add_to_tasks = match unit {
                    Unit::PackageUnit(_) => number_packages_building < self.jobs,
                    _ => true,
                };

                trace!(
                    ?number_packages_building,
                    ?can_add_to_tasks,
                    ?all_parents_built
                );

                if all_parents_built && can_add_to_tasks {
                    let _db_conn = db_conn.clone();
                    let unit = unit.clone();
                    let fut = tokio::spawn(async move {
                        trace!("Starting build task");
                        let res = unit.build(false, &_db_conn).await;
                        (unit, res)
                    });

                    futs.push(fut);
                    task = BuildTask::Building;
                }

                build_tasks.insert(unit, task);
            }

            while let Some((unit, result)) = futs.try_next().await? {
                let result = result?;
                info!(?unit, ?result, "Task finished");
                let t = build_tasks.get_mut(&unit).unwrap();
                *t = BuildTask::Finished;
            }

            trace!(?build_tasks);
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
