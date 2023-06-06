// #![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
#[macro_use]
extern crate educe;

mod build;
mod db;
mod eval;
mod lua;
mod lua_fetch;
mod lua_package;
mod sandbox;
mod schema_db;
mod schema_eval;

use std::path::PathBuf;

use ambassador::{delegatable_trait, Delegate};
use clap::Parser;
use color_eyre::Result;
use tracing_subscriber::prelude::*;

fn setup_logging() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_location_section(true)
        .display_env_section(false)
        .install()?;

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
    let parsed = CliParser::parse();
    parsed.command.main()
}

#[delegatable_trait]
pub trait Main {
    fn main(&self) -> Result<()>;
}

#[derive(clap::Parser, Debug)]
pub struct CliParser {
    #[command(subcommand)]
    pub command: MiqCommands,
}

#[derive(clap::Subcommand, Debug, Delegate)]
#[clap(disable_help_subcommand(true))]
#[delegate(Main)]
pub enum MiqCommands {
    /// Generate the unit schema
    Schema(crate::schema_eval::Args),
    /// Build a unit into the store
    Build(crate::build::Args),
    /// Query and operate on the store database
    Store(crate::db::Args),
    /// Evaluate a unit
    Eval(crate::eval::Args),
    /// -
    Lua(crate::lua::Args),
}
