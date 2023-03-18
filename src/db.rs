use std::{any, path::PathBuf};

use diesel::{prelude::*, sql_types::Integer};
use log::debug;

use crate::db_schema::store;
use crate::db_schema::store::dsl::*;

#[derive(Debug, clap::Args)]
pub struct DbArgs {
    #[command(subcommand)]
    pub action: crate::db::DbSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum DbSubcommand {
    List,
    Add(AddArgs),
}

#[derive(Debug, clap::Args)]
pub struct AddArgs {
    #[arg(value_hint = clap::ValueHint::DirPath)]
    path: PathBuf,
}

pub fn cli_dispatch(args: DbArgs) -> anyhow::Result<()> {
    match args.action {
        DbSubcommand::List => list(),
        DbSubcommand::Add(args) => add(args),
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

    let p = store.limit(5).load::<StorePath>(conn)?;

    for post in p {
        debug!("{:?}", post);
    }

    Ok(())
}

pub fn add(args: AddArgs) -> anyhow::Result<()> {
    let conn = &mut connect_db()?;

    // Prevent inconsistency on trailing slashes
    let path_normalized = args.path.join("");

    let path_str: String = path_normalized
        .as_path()
        .to_str()
        .expect("Couldn't convert path to string")
        .to_owned();

    debug!("Adding {:?}", &path_str);

    let input = NewPath {
        store_path: path_str,
    };

    // TODO: check if store path exists

    let result = diesel::insert_into(store::table)
        .values(&input)
        .execute(conn);

    debug!("Result: {:?}", result);

    Ok(())
}
