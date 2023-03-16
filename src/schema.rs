use std::{fs, io, path::Path};

/**
 Vscode Even Better Toml:
 Wipe cache with:
 rm ~/.config/Code/User/globalStorage/tamasfe.even-better-toml/*
*/ */
use anyhow::Context;
use log::debug;
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;

/// Definition of a package. A package the minimum buildable unit
#[derive(JsonSchema, Debug, Deserialize)]
pub struct Pkg {
    /// Name of the package, normalized
    pub name: String,
    /// Version of the package, normalized
    pub version: String,
    /// List of fecthables available during build-time
    pub fetch: Vec<Fetchable>,
    /// POSIX script excuted at build-time
    pub script: String,
    /// Path that this package produces
    pub path: String,
}

/// A fetchable is fetched from the internet and hash-checked
#[derive(JsonSchema, Debug, Deserialize)]
pub struct Fetchable {
    /// URL to fetch
    pub url: String,
    /// SRI hash to check for integrity
    pub hash: String,
}

/// miq consumes pkg-spec files
#[derive(JsonSchema, Debug, Deserialize)]
pub struct PkgSpec {
    /// List of packages to build
    pub pkg: Vec<Pkg>,
}

pub fn build() -> anyhow::Result<()> {
    let schema = schema_for!(PkgSpec);
    let schema_str = serde_json::to_string_pretty(&schema)?;

    println!("{}", &schema_str);

    Ok(())
}

pub fn parse<P: AsRef<Path>>(path: P) -> anyhow::Result<PkgSpec> {
    let contents = fs::read_to_string(&path).context("While reading the PkgSpec")?;
    let parsed = toml::from_str(&contents).context("While parsing the PkgSpec")?;

    Ok(parsed)
}
