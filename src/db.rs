use std::cell::RefCell;
use std::ops::DerefMut;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{bail, eyre};
use color_eyre::Result;
use diesel::migration::MigrationVersion;
use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing::{debug, info, trace, warn};

use crate::build;
use crate::schema_db::store;
use crate::schema_db::store::dsl::*;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[derive(Debug, clap::Args)]
/// Query the file storage
pub struct Args {
    #[command(subcommand)]
    action: crate::db::CliSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum CliSubcommand {
    /// List all paths registered
    #[command(visible_alias("ls"))]
    List,
    /// Manually register a path
    Add(AddArgs),
    /// Check if a path is registered
    IsPath(IsPathArgs),
    /// Manually remove a path
    #[command(visible_alias("rm"))]
    Remove(RemoveArgs),
}

#[derive(Debug, clap::Args)]
struct AddArgs {
    #[arg(value_hint = clap::ValueHint::DirPath)]
    /// Path to add to the store
    path: PathBuf,
}

#[derive(Debug, clap::Args)]
struct RemoveArgs {
    #[arg(value_hint = clap::ValueHint::DirPath)]
    /// Path to remove from the store
    path: Option<PathBuf>,

    /// Remove all known paths (wipe store)
    #[arg(long)]
    all: bool,
}

#[derive(Debug, clap::Args)]
struct IsPathArgs {
    #[arg(value_hint = clap::ValueHint::DirPath)]
    /// Store path to query
    path: PathBuf,
}

impl crate::Main for Args {
    fn main(&self) -> Result<()> {
        let conn = &mut DbConnection::new()?;

        match &self.action {
            CliSubcommand::List => {
                let all = conn.list()?;
                for path in all {
                    info!(?path);
                }
            }
            CliSubcommand::Add(args) => {
                let path_normalized = fix_dir_trailing_slash(&args.path);
                conn.add(path_normalized)?;
            }
            CliSubcommand::IsPath(args) => {
                let path_normalized = fix_dir_trailing_slash(&args.path);
                let result = conn.is_db_path(path_normalized)?;
                info!("{:?}", result);
            }
            CliSubcommand::Remove(args) => {
                match (&args.path, &args.all) {
                    (Some(path), _) => {
                        let path_normalized = fix_dir_trailing_slash(&path);
                        conn.remove(path_normalized)?;
                    }
                    (None, true) => {
                        let all = conn.list()?;
                        debug!(?all);
                        for elem in all {
                            info!(?elem, "Removing");
                            conn.remove(&elem.store_path)?;
                        }
                    }
                    _ => {
                        let err = eyre!(clap::error::ErrorKind::TooFewValues)
                            .wrap_err("Either use a store path or --all");
                        return Err(err);
                    }
                };
            }
        }

        Ok(())
    }
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = store)]
pub struct StorePath {
    pub store_path: String,
}

#[derive(Insertable)]
#[diesel(table_name = store)]
pub struct NewPath {
    pub store_path: String,
}

pub struct DbConnection {
    inner: RefCell<SqliteConnection>,
}

impl DbConnection {
    pub fn new() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")?;
        trace!("DATABASE_URL: {:?}", database_url);
        let mut conn = diesel::SqliteConnection::establish(&database_url)?;

        match conn.run_pending_migrations(MIGRATIONS) {
            Ok::<Vec<MigrationVersion>, _>(migrations) => {
                if !migrations.is_empty() {
                    info!(?migrations, "Ran DB migrations")
                }
            }
            Err(e) => bail!(e),
        };

        Ok(Self {
            inner: RefCell::new(conn),
        })
    }

    pub fn list(&self) -> Result<Vec<StorePath>> {
        let p: Vec<StorePath> = store.load::<StorePath>(self.inner.borrow_mut().deref_mut())?;
        Ok(p)
    }

    pub fn add<P: AsRef<Path> + std::fmt::Debug>(&mut self, path: P) -> Result<()> {
        let path_str: String = path
            .as_ref()
            .to_str()
            .expect("Couldn't convert path to string")
            .to_owned();

        debug!("Adding {:?}", &path_str);

        let input = NewPath {
            store_path: path_str,
        };

        if self.is_db_path(&path)? {
            warn!("Path is already on the store");
        } else {
            let db_response = diesel::insert_into(store::table)
                .values(&input)
                .execute(self.inner.borrow_mut().deref_mut())?;

            trace!(?db_response);
        };

        Ok(())
    }

    pub fn remove<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        build::clean_path(&path)?;

        let path_str = path.to_str().unwrap();

        let db_response = diesel::delete(store)
            .filter(store_path.is(&path_str))
            .execute(self.inner.borrow_mut().deref_mut())?;

        trace!(?db_response);

        Ok(())
    }

    #[tracing::instrument(ret, level = "trace", skip(self))]
    pub fn is_db_path<P: AsRef<Path> + std::fmt::Debug>(&mut self, path: P) -> Result<bool> {
        let path_str = path.as_ref().to_str().unwrap();

        let elements: Vec<StorePath> = store
            .filter(store_path.is(path_str))
            .load::<StorePath>(self.inner.borrow_mut().deref_mut())?;

        Ok(!elements.is_empty())
    }
}

/// Remove trailing slashes from directories (coming from user input)
fn fix_dir_trailing_slash<P: AsRef<Path> + std::fmt::Debug>(path: P) -> PathBuf {
    let base = &mut PathBuf::from("/");

    for comp in path.as_ref().components() {
        trace!("{:?}", comp);
        base.push(comp);
    }

    base.to_owned()
}
