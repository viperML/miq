use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use color_eyre::eyre::bail;
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
    pub action: crate::db::CliSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum CliSubcommand {
    /// List all paths registered
    #[command(visible_alias("ls"))]
    List,
    /// Manually register a path
    Add(AddArgs),
    /// Check if a path is registered
    IsPath(IsPathArgs),
    /// Manually remove a path
    #[command(visible_alias("rm"))]
    Remove(AddArgs),
}

#[derive(Debug, clap::Args)]
pub struct AddArgs {
    #[arg(value_hint = clap::ValueHint::DirPath)]
    path: PathBuf,
}

#[derive(Debug, clap::Args)]
pub struct IsPathArgs {
    #[arg(value_hint = clap::ValueHint::DirPath)]
    path: PathBuf,
}

impl crate::Main for Args {
    fn main(&self) -> Result<()> {
        let conn = &mut DbConnection::new()?;

        match &self.action {
            CliSubcommand::List => {
                conn.list()?;
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
                let path_normalized = fix_dir_trailing_slash(&args.path);
                conn.remove(path_normalized)?;
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

    pub fn list(&self) -> Result<()> {
        let p: Vec<StorePath> = store
            .limit(5)
            .load::<StorePath>(self.inner.borrow_mut().deref_mut())?;

        for elem in p {
            info!(?elem);
        }

        Ok(())
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
        debug!("{:?}", comp);
        base.push(comp);
    }

    base.to_owned()
}
