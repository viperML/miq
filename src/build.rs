use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fmt::{format, Debug};
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
use daggy::petgraph::graph::Node;
use daggy::petgraph::visit::{Dfs, IntoNodeReferences};
use daggy::Walker;
use derive_builder::Builder;
use futures::stream::futures_unordered;
use futures::{StreamExt, TryStreamExt};
use owo_colors::OwoColorize;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio_process_stream::{Item, ProcessLineStream};
use tracing::{debug, info, span, trace, Level};

use crate::db::DbConnection;
use crate::eval::{MiqEvalPath, MiqResult, MiqStorePath, RefToUnit, UnitRef};
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

const MAX_BUILD_ITERATIONS: u32 = 100;

impl Args {
    async fn _main(&self) -> Result<()> {
        let root_node = self.unit_ref.ref_to_unit()?;
        let (dag, _) = eval::dag(root_node.clone())?;

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
                    let unit = unit.clone();
                    let rebuild = match (self, &unit) {
                        (Args { rebuild: true, .. }, _) => unit == root_node,
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
                    ?task_status
                );

                build_tasks.insert(unit, task_status);
            }

            while let Some((unit, result)) = futs.try_next().await? {
                let res: MiqResult = unit.clone().into();
                let eval: MiqEvalPath = (&res).into();
                let sugg = format!("Check the unit at {}", eval.as_ref().to_str().unwrap());
                let result = result.suggestion(sugg)?;
                debug!(?unit, ?result, "Task finished");
                let t = build_tasks.get_mut(&unit).unwrap();
                *t = BuildTask::Finished;

                // Pretty log
                let p: &Path = result.as_ref();
                let u = format!("{unit:?}");
                eprintln!(
                    "{} <- {}",
                    p.to_str().unwrap().bright_blue(),
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
    #[tracing::instrument(skip(conn), ret, err, level = "debug")]
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

        clean_path(&store_path)?;
        let tempdir = tempfile::tempdir()?;
        let builddir = tempdir.path();

        let wrapped_cmd = ["/bin/sh", "-c", &self.script];

        let mut cmd = BubblewrapBuilder::default()
            .builddir(builddir)
            .resultdir(p)
            .cmd(&wrapped_cmd)
            .env(&self.env)
            .build()?
            .build_command()?;

        let child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

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
            "--clearenv",
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
