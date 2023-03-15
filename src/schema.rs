use std::{fs, io, path::Path};

use anyhow::Context;
use log::debug;
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;

/**
 Vscode Even Better Toml:
 Wipe cache with:
 rm ~/.config/Code/User/globalStorage/tamasfe.even-better-toml/*
*/ */

#[derive(JsonSchema, Debug, Deserialize)]
pub struct Pkg {
    pub name: String,
    pub version: String,
    pub fetch: Vec<Fetchable>,
    pub script: String,
    pub path: String,
}

#[derive(JsonSchema, Debug, Deserialize)]
pub struct Fetchable {
    pub url: String,
    pub hash: String,
}

#[derive(JsonSchema, Debug, Deserialize)]
pub struct PkgSpec {
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
