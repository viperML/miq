use std::path::{Path, self};
use std::{any, path::PathBuf};

use anyhow::bail;
use diesel::{prelude::*, sql_types::Integer};
use log::{debug, info, warn};
use serde::__private::de;

use crate::build;
use crate::db_schema::store;
use crate::db_schema::store::dsl::*;

#[derive(Debug, clap::Args)]
pub struct CliArgs {
    #[command(subcommand)]
    pub action: crate::db::CliSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum CliSubcommand {
    /// List all paths registered
    List,
    /// Manually register a path
    Add(AddArgs),
    /// Check if a path is registered
    IsPath(IsPathArgs),
    /// Remove a path
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

pub fn cli_dispatch(args: CliArgs) -> anyhow::Result<()> {
    match args.action {
        CliSubcommand::List => list(),
        CliSubcommand::Add(args) => {
            let path_normalized = fix_dir_trailing_slash(&args.path);
            add(path_normalized)
        }
        CliSubcommand::IsPath(args) => {
            let path_normalized = fix_dir_trailing_slash(&args.path);
            let result = is_db_path(&path_normalized)?;
            info!("{:?}", result);
            Ok(())
        }
        CliSubcommand::Remove(args) => {
            let path_normalized = fix_dir_trailing_slash(&args.path);
            remove(&path_normalized)
        }
    }
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = store)]
pub struct StorePath {
    pub id: i32,
    pub store_path: String,
}

#[derive(Insertable)]
#[diesel(table_name = store)]
pub struct NewPath {
    pub store_path: String,
}

fn connect_db() -> anyhow::Result<SqliteConnection> {
    let database_url = std::env::var("DATABASE_URL")?;
    debug!("DATABASE_URL: {:?}", database_url);
    Ok(diesel::SqliteConnection::establish(&database_url)?)
}

pub fn list() -> anyhow::Result<()> {
    let conn = &mut connect_db()?;

    let p: Vec<StorePath> = store.limit(5).load::<StorePath>(conn)?;

    for post in p {
        debug!("{:?}", post);
    }

    Ok(())
}

pub fn add<P: AsRef<Path> + std::fmt::Debug>(path: P) -> anyhow::Result<()> {
    let conn = &mut connect_db()?;

    let path_str: String = path
        .as_ref()
        .to_str()
        .expect("Couldn't convert path to string")
        .to_owned();

    debug!("Adding {:?}", &path_str);

    let input = NewPath {
        store_path: path_str,
    };

    if is_db_path(&path)? {
        warn!("Path is already on the store");
    } else {
        let result = diesel::insert_into(store::table)
            .values(&input)
            .execute(conn);

        debug!("Result: {:?}", result);
    };

    Ok(())
}

pub fn remove<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    let conn = &mut connect_db()?;

    let path = path.as_ref();
    build::clean_path(path)?;

    let path_str = path.to_str().unwrap();

    let result = diesel::delete(store)
        .filter(store_path.is(&path_str))
        .execute(conn)?;

    debug!("Deletion result: {:?}", result);

    Ok(())
}

pub fn is_db_path<P: AsRef<Path> + std::fmt::Debug>(path: P) -> anyhow::Result<bool> {
    let conn = &mut connect_db()?;

    let path_str = path.as_ref().to_str().unwrap();

    debug!("path_str: {:?}", path_str);

    let elements: Vec<StorePath> = store
        .filter(store_path.is(path_str))
        .load::<StorePath>(conn)?;

    for elem in &elements {
        debug!("found elem {:?}", elem);
    }

    Ok(!elements.is_empty())
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
