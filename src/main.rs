#![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
#[macro_use] extern crate educe;


mod build;
mod cli;
mod dag;
mod db;
mod sandbox;
mod schema_db;
mod schema_eval;

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use color_eyre::Result;
use tracing::debug;
use tracing_subscriber::prelude::*;

fn setup_logging() -> Result<()> {
    color_eyre::install()?;

    let layer_filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("info".parse()?)
        .add_directive("miq=trace".parse()?);

    let layer_fmt = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .without_time()
        .with_line_number(true)
        .compact();

    tracing_subscriber::registry()
        .with(layer_filter)
        .with(layer_fmt)
        .init();

    Ok(())
}

fn main() -> Result<()> {
    setup_logging()?;

    let parsed = cli::CliParser::parse();

    match parsed.command {
        cli::MiqCommands::Schema(args) => args.main(),
        cli::MiqCommands::Build(args) => args.main(),
        cli::MiqCommands::Store(args) => db::cli_dispatch(args),
        cli::MiqCommands::Eval(args) => args.main(),
    }
}
