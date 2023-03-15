use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{self, Path, PathBuf};
use std::str::FromStr;
use std::{fs, io, vec};

use anyhow::{bail, Context};
use bytes::Buf;
use log::debug;
use unshare::Command;

use crate::schema;

#[derive(Debug, clap::Args)]
pub struct BuildArgs {
    /// Path of PkgSpec to build
    #[arg()]
    file: PathBuf,
}

fn mkdir<P: AsRef<Path> + Debug>(p: P) -> Result<(), io::Error> {
    debug!("Creating directory: {:?}", p);

    if let Err(err) = fs::create_dir_all(p) {
        match err.kind() {
            io::ErrorKind::AlreadyExists => {
                debug!("Build dir already exists");
                Ok(())
            }
            _ => Err(err),
        }
    } else {
        debug!("Create dir: operation successful");
        Ok(())
    }
}

pub fn build_spec(args: BuildArgs) -> anyhow::Result<()> {
    debug!("args: {:?}", args);

    let spec = schema::parse(args.file)?;
    debug!("spec: {:?}", spec);

    for p in spec.pkg {
        debug!("building pkg: {:?}", p);
        build_pkg(p)?;
    }

    Ok(())
}

pub fn build_pkg(pkg: schema::Pkg) -> anyhow::Result<()> {
    let fetch_paths: Result<Vec<_>, _> =
        pkg.fetch.iter().map(|fetchable| fetch(fetchable)).collect();

    let fetch_paths = fetch_paths?;

    let mut env: HashMap<&str, &str> = HashMap::new();

    let env_fetch: Vec<_> = fetch_paths
        .iter()
        .map(|elem| elem.to_str().expect("Couldn't format fetch_path"))
        .collect();

    let env_fetch = &env_fetch.join(":");

    env.insert("MIQ_FETCH", env_fetch);

    env.insert("PATH", "/home/ayats/Documents/miq/devel/nix-bootstrap/bin");

    debug!("env: {:?}", env);


    // let args: Vec<_> = pkg.script.split(" ").collect();
    // let sh_args = vec!("-c");
    // let cmd_args = [ &sh_args[..], &[vec] ].concat();
    let cmd_args = [
        "-c",
        &pkg.script
    ];

    let mut cmd = Command::new("/bin/sh");
    cmd.args(&cmd_args);
    cmd.env_clear();
    cmd.envs(&env);

    debug!("output: {:?}", &cmd);

    let status = cmd.status();
    debug!("{:?}", status);

    Ok(())
}

pub fn fetch(fch: &schema::Fetchable) -> anyhow::Result<PathBuf> {
    let outpath = PathBuf::from_str("/tmp/miq-fetch/fetch1")?;

    let meta = fs::metadata(&outpath)?;
    if meta.is_file() {
        return Ok(outpath);
    }

    todo!();

    debug!("Fetching: {:?}", fch);
    fs::create_dir_all("/tmp/miq-fetch")?;
    let mut outfile = File::create(&outpath)?;
    debug!("outfile {:?}", outfile);

    let client = reqwest::blocking::Client::new();
    let response = client.get(&fch.url).send()?;
    let mut content = response.bytes()?.reader();
    std::io::copy(&mut content, &mut outfile)?;

    debug!("Fetch Ok");

    Ok(outpath)
}

// pub fn build(pkg: expr::FOP) -> anyhow::Result<()> {
//     // TODO use tempfile
//     let builddir = Path::new("/tmp/miq-build");
//     debug!("builddir: {:?}", builddir);

//     mkdir(builddir)?;

//     let tmpf = Path::new("/tmp/miq-download");

//     let mut f = std::fs::File::create(tmpf)?;
//     debug!("f: {:?}", f);

//     let client = reqwest::blocking::Client::new();

//     debug!("Copying file");

//     Ok(())
// }
