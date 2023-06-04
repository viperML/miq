#![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
#[macro_use]
extern crate educe;

mod build;
mod cli;
mod db;
mod eval;
mod ffi;
mod lua_fetch;
mod lua_package;
mod lua;
mod sandbox;
mod schema_db;
mod schema_eval;

use std::path::PathBuf;

use clap::Parser;
use color_eyre::Result;
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

    let layer_error = tracing_error::ErrorLayer::default();

    tracing_subscriber::registry()
        .with(layer_filter)
        .with(layer_error)
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
        cli::MiqCommands::Lua(args) => args.main(),
    }
}
