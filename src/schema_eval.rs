/**
 Vscode Even Better Toml:
 Wipe cache with:
 rm ~/.config/Code/User/globalStorage/tamasfe.even-better-toml/*
*/ */
use std::collections::{HashMap, BTreeMap};
use std::ffi::OsString;
use std::path::PathBuf;

use color_eyre::Result;
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::eval::MiqResult;

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
            let dummy = Unit::PackageUnit(Package::default());
            let s = toml::to_string_pretty(&dummy)?;
            println!("{}", s);
        }

        Ok(())
    }
}

#[derive(Educe, PartialEq, Clone, Serialize, Deserialize, JsonSchema, Hash)]
#[educe(Debug)]
#[serde(untagged)]
pub enum Unit {
    #[educe(Debug(name = false))]
    PackageUnit(Package),
    #[educe(Debug(name = false))]
    FetchUnit(Fetch),
}


#[derive(Educe, PartialEq, Clone, Serialize, Deserialize, JsonSchema, Default, Hash)]
#[educe(Debug)]
pub struct Package {
    #[educe(Debug(ignore))]
    pub result: MiqResult,
    pub name: String,
    #[educe(Debug(ignore))]
    pub version: String,
    #[educe(Debug(ignore))]
    pub deps: Vec<MiqResult>,
    #[educe(Debug(ignore))]
    pub script: OsString,
    #[educe(Debug(ignore))]
    pub env: BTreeMap<OsString, OsString>,
}

#[derive(Educe, PartialEq, Clone, Deserialize, Serialize, JsonSchema, Default, Hash)]
#[educe(Debug)]
pub struct Fetch {
    #[educe(Debug(ignore))]
    pub result: MiqResult,
    pub name: String,
    #[educe(Debug(ignore))]
    pub url: String,
    #[educe(Debug(ignore))]
    pub integrity: String,
    #[educe(Debug(ignore))]
    pub executable: bool,
}
