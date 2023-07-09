// #![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
#[macro_use]
extern crate educe;

mod build;
mod build_fetch;
mod build_package;
mod db;
mod eval;
mod lua;
mod lua_fetch;
mod lua_package;
mod schema_db;
mod schema_eval;
mod semaphore;
mod mem_app;

use std::path::PathBuf;

use ambassador::{delegatable_trait, Delegate};
use clap::Parser;
use color_eyre::eyre::{bail, ContextCompat};
use color_eyre::Result;
use tracing::info;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

fn setup_logging() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .add_default_filters()
        .display_location_section(true)
        .display_env_section(false)
        .install()?;

    let layer_fmt = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .without_time()
        .with_line_number(true)
        .compact();

    let layer_error = tracing_error::ErrorLayer::default();

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(layer_error)
        .with(layer_fmt)
        .init();

    Ok(())
}

fn main() -> Result<()> {
    setup_logging()?;
    check_dirs()?;
    let parsed = CliParser::parse();
    parsed.command.main()
}

fn check_dirs() -> Result<()> {
    if !PathBuf::from("/miq").try_exists()? {
        info!("Create /miq?");
        if !dialoguer::Confirm::new().default(false).interact()? {
            bail!("No confirmation");
        };

        std::process::Command::new("sudo")
            .args(["mkdir", "/miq"])
            .output()?;

        let nix::unistd::User { name, gid, .. } =
            nix::unistd::User::from_uid(nix::unistd::geteuid())?.unwrap();

        let group = nix::unistd::Group::from_gid(gid)?.unwrap();

        let mut cmd = std::process::Command::new("sudo");
        cmd.args(["chown", "-R", &format!("{}:{}", name, group.name), "/miq"]);
        info!(?cmd);
        cmd.output()?;
    };

    for folder in ["/miq/store", "/miq/eval", "/miq/log"] {
        if !PathBuf::from(folder).try_exists()? {
            info!(?folder, "Creating directory");
            std::fs::create_dir(folder)?;
        };
    }

    Ok(())
}

#[delegatable_trait]
pub trait Main {
    fn main(&self) -> Result<()>;
}

#[derive(clap::Parser, Debug)]
#[clap(
    disable_help_subcommand = true,
    author = clap::crate_authors!("\n"),
    version = clap::crate_version!(),
    help_template = "\
{before-help}{name} {version}
{author-with-newline}{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
"
)]
pub struct CliParser {
    #[command(subcommand)]
    pub command: MiqCommands,
}

#[derive(clap::Subcommand, Debug, Delegate)]
#[delegate(Main)]
pub enum MiqCommands {
    Build(crate::build::Args),
    Eval(crate::eval::Args),
    Lua(crate::lua::Args),
    Store(crate::db::Args),
    Schema(crate::schema_eval::Args),
}
