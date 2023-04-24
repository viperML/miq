use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use color_eyre::Result;

use color_eyre::eyre::Context;
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
/**
 Vscode Even Better Toml:
 Wipe cache with:
 rm ~/.config/Code/User/globalStorage/tamasfe.even-better-toml/*
*/ */
use tracing::{debug, info};

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

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Generate dummy data to test the schema
    #[clap(short, long)]
    dummy_data: bool,
    /// Parse a unit and print its internal representation
    #[clap(short, long)]
    parse: Option<PathBuf>,
}

impl Args {
    pub fn main(&self) -> Result<()> {
        if let Some(p) = &self.parse {
            let s = std::fs::read_to_string(p)?;
            let result: Unit = toml::from_str(&s)?;
            println!("{:?}", result);
        } else if !self.dummy_data {
            let schema = schema_for!(Unit);
            let schema_str = serde_json::to_string_pretty(&schema)?;
            println!("{}", &schema_str);
            let p = "/miq/eval-schema.json";
            info!("Writing schema to {}", p);
            info!("Reset VS code with: rm ~/.config/Code/User/globalStorage/tamasfe.even-better-toml/*");
            std::fs::write(p, schema_str)?;
        } else {
            let dummy = Unit::Package(Package::default());
            let s = toml::to_string_pretty(&dummy)?;
            println!("{}", s);
        }

        Ok(())
    }
}

pub fn parse<P: AsRef<Path>>(path: P) -> Result<MiqSpec> {
    let contents = fs::read_to_string(&path).context("While reading the PkgSpec")?;
    let parsed = toml::from_str(&contents).context("While parsing the PkgSpec")?;

    Ok(parsed)
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Unit {
    Package(Package),
    Fetch(Fetch),
}

impl Unit {
    pub fn from_result<P: AsRef<str>>(r: P) -> Result<Unit> {
        let r = r.as_ref();
        let filename = format!("/miq/eval/{}.toml", r);
        let contents = std::fs::read_to_string(filename)?;
        Ok(toml::from_str(contents.as_str())?)
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct Package {
    pub result: String,
    pub name: String,
    pub version: String,
    pub deps: Vec<String>,
    pub script: String,
    pub env: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize, JsonSchema, Default)]
pub struct Fetch {
    pub result: String,
    pub name: String,
    pub url: String,
    pub integrity: String,
}
