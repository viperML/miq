#![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]

mod build;
mod cli;
mod db;
mod schema_db;
mod schema_eval;
mod sandbox;

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use tracing::debug;
use tracing_subscriber::prelude::*;
use color_eyre::Result;

fn setup_logging() -> Result<()> {
    color_eyre::install()?;

    let filter_layer =
        tracing_subscriber::EnvFilter::from_default_env().add_directive("debug".parse()?);

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .without_time()
        .with_line_number(true)
        .compact();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    Ok(())
}

fn main() -> Result<()> {
    setup_logging()?;

    let parsed = cli::CliParser::parse();

    match parsed.command {
        cli::MiqCommands::Schema(args) => args.main(),
        cli::MiqCommands::Build(args) => build::build_spec(args),
        cli::MiqCommands::Db(args) => db::cli_dispatch(args),
    }
}
