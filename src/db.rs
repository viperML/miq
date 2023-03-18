use std::path::Path;
use std::{any, path::PathBuf};

use diesel::{prelude::*, sql_types::Integer};
use log::{debug, info, warn};
use serde::__private::de;

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
        CliSubcommand::Add(args) => add(args),
        CliSubcommand::IsPath(args) => {
            let result = is_db_path(&args.path)?;
            info!("{:?}", result);
            Ok(())
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

pub fn add(args: AddArgs) -> anyhow::Result<()> {
    let conn = &mut connect_db()?;

    let path_normalized = fix_path_trailing_slash(&args.path);

    let path_str: String = path_normalized
        .as_path()
        .to_str()
        .expect("Couldn't convert path to string")
        .to_owned();

    debug!("Adding {:?}", &path_str);

    let input = NewPath {
        store_path: path_str,
    };

    if is_db_path(&path_normalized)? {
        warn!("Path is already on the store");
    } else {
        let result = diesel::insert_into(store::table)
            .values(&input)
            .execute(conn);

        debug!("Result: {:?}", result);
    };

    Ok(())
}

pub fn is_db_path<P: AsRef<Path> + std::fmt::Debug>(path: P) -> anyhow::Result<bool> {
    let conn = &mut connect_db()?;

    let path = fix_path_trailing_slash(path.as_ref());

    let path_str = path.to_str().expect("Couldn't convert path to str");

    debug!("path_str: {:?}", path_str);

    let elements: Vec<StorePath> = store
        .filter(store_path.is(path_str))
        .load::<StorePath>(conn)?;

    for elem in &elements {
        debug!("found elem {:?}", elem);
    }

    Ok(!elements.is_empty())
}

/// Normalize paths coming from user input by adding a trailing slash to folders
fn fix_path_trailing_slash(path: &Path) -> PathBuf {
    path.join("")
}
