#![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]

mod cli;
mod store;
mod db_schema;
mod pkgs_schema;
mod build;

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use log::debug;

fn setup_logging() -> anyhow::Result<()> {
    let loglevel = log::LevelFilter::Debug;

    fern::Dispatch::new()
        .format(|out, message, record| out.finish(format_args!("[{}] {}", record.level(), message)))
        .level(loglevel)
        .chain(std::io::stdout())
        .apply()?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    setup_logging()?;

    let parsed = cli::CliParser::parse();

    match parsed.command {
        cli::MiqCommands::Schema => pkgs_schema::build(),
        cli::MiqCommands::Build(args) => build::build_spec(args),
        cli::MiqCommands::Db(args) => store::cli_dispatch(args),
        x => todo!("Command {:?} not yet implemented", x),
    }
}
