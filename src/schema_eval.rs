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
