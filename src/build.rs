use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fmt::Debug;
use std::io::Write;
use std::ops::Deref;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::{fs, io};

use async_trait::async_trait;
use bytes::Buf;
use color_eyre::eyre::{bail, ensure, eyre, Context};
use color_eyre::Help;

use daggy::Walker;
use derive_builder::Builder;
use futures::stream::futures_unordered;
use futures::{StreamExt, TryStreamExt};
use owo_colors::OwoColorize;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio_process_stream::{Item, ProcessLineStream};
use tracing::{debug, span, trace, Level};

use crate::db::DbConnection;
use crate::eval::{RefToUnit, UnitRef};
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

    /// Rebuild the selected element, but don't rebuild its dependency tree
    #[arg(long, short)]
    rebuild: bool,

    /// Rebuild all packages in the dependency tree
    #[arg(long, short = 'R')]
    rebuild_all: bool,

    /// Maximum number of concurrent build jobs. Fetch jobs are parallelized automatically.
    #[arg(long = "jobs", short = 'j', default_value = "1")]
    max_jobs: usize,
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
    Waiting,
    Building,
    Finished,
}

const MAX_BUILD_ITERATIONS: u32 = 1000;

impl Args {
    async fn _main(&self) -> Result<()> {
        let root_node = self.unit_ref.ref_to_unit()?;
        let (dag, _) = eval::dag(root_node.clone())?;
        let dag: &'static mut _ = Box::leak(Box::new(dag));

        let db_conn = Arc::new(Mutex::new(crate::db::DbConnection::new()?));

        let mut build_tasks: HashMap<&Unit, BuildTask> = HashMap::new();
        let mut futs = futures_unordered::FuturesUnordered::new();

        let mut sentry = 0;

        build_tasks.insert(&root_node, BuildTask::Waiting);

        while !build_tasks.iter().all(|(_, task)| match task {
            BuildTask::Finished => true,
            _ => false,
        }) {
            for index in dag.graph().node_indices() {
                // Avoid blowing up
                ensure!(
                    sentry <= MAX_BUILD_ITERATIONS,
                    "Build sentry reached, something might have gone wrong!"
                );
                sentry = sentry + 1;

                let unit = &dag[index];

                let span = span!(Level::TRACE, "Graph walk", ?unit, ?index);
                let _enter = span.enter();

                let existing_task = build_tasks.get(&unit);
                trace!(?existing_task);
                match existing_task {
                    None | Some(BuildTask::Waiting) => {}
                    _ => continue,
                };

                let all_deps_built = dag.children(index).iter(&dag).all(|(_, dep_index)| {
                    match &build_tasks.get(&dag[dep_index]) {
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
                    Unit::PackageUnit(_) => number_packages_building < self.max_jobs,
                    _ => true,
                };

                let task_status = if all_deps_built && can_add_to_tasks {
                    let _db_conn = db_conn.clone();
                    // let unit = unit.clone();
                    let rebuild = match (self, &unit) {
                        (Args { rebuild: true, .. }, _) => unit == &root_node,
                        (
                            Args {
                                rebuild_all: true, ..
                            },
                            Unit::PackageUnit(_),
                        ) => true,
                        _ => false,
                    };
                    let fut = tokio::spawn(async move {
                        trace!("Starting build task");
                        let res = unit.build(rebuild, &_db_conn).await;
                        (unit, res)
                    });
                    futs.push(fut);
                    BuildTask::Building
                } else {
                    BuildTask::Waiting
                };

                trace!(
                    ?number_packages_building,
                    ?can_add_to_tasks,
                    ?all_deps_built,
                    ?task_status,
                    ?sentry
                );

                build_tasks.insert(unit, task_status);
            }

            while let Some((unit, output)) = futs.try_next().await? {
                debug!(?unit, ?output, "Task finished");

                output
                    .suggestion(format!(
                        "Check the unit definition at {}",
                        unit.result().eval_path().to_string_lossy()
                    ))
                    .suggestion(format!(
                        "Build logs available at /miq/log/{}.log",
                        unit.result().as_str()
                    ))
                    .suggestion(format!(
                        "Intermetidate results at {}",
                        unit.result().store_path().to_string_lossy()
                    ))?;

                let t = build_tasks.get_mut(&unit).unwrap();
                *t = BuildTask::Finished;

                let u = format!("{unit:?}");
                eprintln!(
                    "{} <- {}",
                    unit.result().store_path().to_string_lossy().bright_blue(),
                    &u.bright_black()
                );
            }

            trace!(?build_tasks);
        }
        Ok(())
    }
}

#[async_trait]
impl Build for Fetch {
    #[tracing::instrument(skip(conn), ret, err, level = "debug")]
    async fn build(&self, rebuild: bool, conn: &Mutex<DbConnection>) -> Result<()> {
        let path = self.result.store_path();
        let path = path.as_path();

        if conn.lock().unwrap().is_db_path(&path)? {
            if rebuild {
                conn.lock().unwrap().remove(&path)?;
            } else {
                return Ok(());
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
        Ok(())
    }
}

#[async_trait]
impl Build for Package {
    #[tracing::instrument(skip(conn), ret, err, level = "debug")]
    async fn build(&self, rebuild: bool, conn: &Mutex<DbConnection>) -> Result<()> {
        let path = self.result.store_path();
        let path = path.as_path();
        let path_str = path.to_str().unwrap();

        if conn.lock().unwrap().is_db_path(&path)? {
            if rebuild {
                conn.lock().unwrap().remove(&path)?;
            } else {
                return Ok(());
            }
        }

        clean_path(&path)?;
        std::fs::create_dir(&path)?;
        let _builddir = tempfile::tempdir()?;
        let builddir = _builddir.path();

        let _tmpdir = tempfile::tempdir()?;
        let tmpdir = _tmpdir.path().to_str().unwrap();

        let mut cmd = Command::new("bwrap");
        cmd.env_clear();
        cmd.args([
            "--bind", tmpdir, tmpdir, "--setenv", "TMP", tmpdir, "--setenv", "TMPDIR", tmpdir,
            "--setenv", "TEMP", tmpdir, "--setenv", "TEMPDIR", tmpdir,
        ]);
        cmd.args([
            "--clearenv",
            // Store and result
            "--ro-bind",
            "/miq",
            "/miq",
            "--bind",
            path_str,
            path_str,
            "--setenv",
            "miq_out",
            path_str,
            // Build directory
            "--bind",
            builddir.to_str().unwrap(),
            "/build",
            "--chdir",
            "/build",
            "--setenv",
            "HOME",
            "/build",
            // Global environent
            "--dev-bind",
            "/dev",
            "/dev",
            "--proc",
            "/proc",
            "--bind",
            "/run",
            "/run",
            "--bind",
            "/tmp",
            "/tmp",
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
        ]);
        cmd.args(["/bin/sh", "-c", &self.script]);
        cmd.envs(&self.env);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let child = cmd.spawn()?;

        let log_file_path = format!("/miq/log/{}.log", self.result.deref());
        let err_msg = format!("Creating logfile at {}", log_file_path);
        let mut log_file = std::fs::File::create(log_file_path).wrap_err(err_msg)?;

        let mut procstream = ProcessLineStream::try_from(child)?;
        while let Some(item) = procstream.next().await {
            use owo_colors::OwoColorize;
            match item {
                Item::Stdout(line) => {
                    let msg = format!("{}>>{}", self.name.blue(), line.bright_black());
                    println!("{}", msg);
                    log_file.write_all(line.as_bytes())?;
                    log_file.write_all(b"\n")?;
                }
                Item::Stderr(line) => {
                    let msg = format!("{}>>{}", self.name.blue(), line.bright_black());
                    println!("{}", msg);
                    log_file.write_all(line.as_bytes())?;
                    log_file.write_all(b"\n")?;
                }
                Item::Done(Ok(exit)) => {
                    if exit.success() {
                        debug!("Build OK");
                    } else {
                        bail!(eyre!("Exit not successful").wrap_err(exit));
                    }
                }
                Item::Done(Err(exit)) => bail!(exit),
            }
        }

        match path.try_exists().wrap_err("Failed to produce an output") {
            Ok(true) => {}
            Ok(false) => bail!("Output path doesn't exist: {:?}", path),
            Err(e) => bail!(e),
        }

        conn.lock().unwrap().add(&path)?;
        Ok(())
    }
}
