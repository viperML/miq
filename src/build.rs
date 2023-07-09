use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::{fs, io};

use color_eyre::eyre::ensure;
use color_eyre::Help;
use daggy::Walker;
use futures::stream::futures_unordered;
use futures::TryStreamExt;
use indicatif::{MultiProgress, ProgressBar};
use owo_colors::OwoColorize;
use tracing::{debug, instrument, span, trace, Level};

use crate::eval::{RefToUnit, UnitRef};
use crate::schema_eval::{Build, Unit};
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

        let bars = MultiProgress::new();
        // bars.set_alignment(MultiProgressAlignment::Top);
        // bars.set_move_cursor(false);

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
                    let bars = bars.clone();
                    let fut = tokio::spawn(async move {
                        trace!("Starting build task");
                        let build_result = match unit {
                            Unit::PackageUnit(_) => unit.build(rebuild, &_db_conn, None),
                            Unit::FetchUnit(_) => {
                                let pb = ProgressBar::hidden();
                                unit.build(rebuild, &_db_conn, Some(bars.add(pb)))
                            }
                        }
                        .await;
                        // let res = unit.build(rebuild, &_db_conn).await;
                        (unit, build_result)
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
                let msg = format!(
                    "{} <- {}",
                    unit.result().store_path().to_string_lossy().bright_blue(),
                    &u.bright_black()
                );
                bars.println(msg)?;
            }

            trace!(?build_tasks);
        }
        Ok(())
    }
}

#[instrument(ret, err, level = "trace")]
pub fn clean_path<P: AsRef<Path> + Debug>(path: P) -> Result<()> {
    match fs::metadata(&path) {
        Ok(meta) => {
            trace!("Path exists, removing");
            if meta.is_file() {
                fs::remove_file(&path)?;
            } else if meta.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                bail!("{:?} Wasn't either a dir or a file", path);
            }
            Ok(())
        }
        Err(err) => match err.kind() {
            io::ErrorKind::NotFound => {
                trace!("Doesn't exist, skipping");
                Ok(())
            }
            _ => bail!(err),
        },
    }
}

#[instrument(ret, err, level = "trace")]
pub fn check_path<P: AsRef<Path> + Debug>(path: P) -> Result<()> {
    let path = path.as_ref();

    match path.try_exists() {
        Ok(true) => Ok(()),
        Ok(false) => bail!("Path doesn't exist: {:?}", path),
        Err(e) => bail!(e),
    }
}
