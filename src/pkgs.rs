use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

/**
 Vscode Even Better Toml:
 Wipe cache with:
 rm ~/.config/Code/User/globalStorage/tamasfe.even-better-toml/*
*/ */
use anyhow::Context;
use tracing::debug;
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;

/// Definition of a package. A package the minimum buildable unit
#[derive(JsonSchema, Debug, Deserialize)]
pub struct Pkg {
    /// Name of the package, normalized
    pub name: String,
    /// Version of the package, normalized
    pub version: String,
    /// POSIX script executed at build-time
    pub script: String,
    /// Path that this package produces
    pub path: PathBuf,
    /// Environment variables at build-time
    pub env: HashMap<String, String>,

    /// Build-time deps, for build machine
    pub bdeps_buildm: Vec<PathBuf>,
    /// Build-time deps, for target machine
    pub bdeps_hostm: Vec<PathBuf>,
    /// Run-time deps, for target machine
    pub rdeps_hostm: Vec<PathBuf>,
}

/// A fetchable is fetched from the internet and hash-checked
#[derive(JsonSchema, Debug, Deserialize)]
pub struct Fetchable {
    /// URL to fetch
    pub url: String,
    /// SRI hash to check for integrity
    pub hash: String,
    /// Produced path in the store
    pub path: PathBuf,
}

/// miq consumes pkg-spec files
#[derive(JsonSchema, Debug, Deserialize)]
pub struct MiqSpec {
    /// List of packages
    pub pkg: Vec<Pkg>,
    /// List of fetchables
    pub fetch: Vec<Fetchable>,
}

pub fn build_schema() -> anyhow::Result<()> {
    let schema = schema_for!(MiqSpec);
    let schema_str = serde_json::to_string_pretty(&schema)?;

    println!("{}", &schema_str);

    Ok(())
}

pub fn parse<P: AsRef<Path>>(path: P) -> anyhow::Result<MiqSpec> {
    let contents = fs::read_to_string(&path).context("While reading the PkgSpec")?;
    let parsed = toml::from_str(&contents).context("While parsing the PkgSpec")?;

    Ok(parsed)
}
