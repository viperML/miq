use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{self, Path, PathBuf};
use std::str::FromStr;
use std::{fs, io, vec};

use anyhow::{bail, Context};
use bytes::Buf;
use log::{debug, info};
use unshare::Command;

use crate::db;
use crate::pkgs;

#[derive(Debug, clap::Args)]
pub struct BuildArgs {
    /// Path of PkgSpec to build
    #[arg()]
    file: PathBuf,

    /// Don't show build output
    #[arg(long, short)]
    quiet: bool,

    /// Rebuild path even if it already exists
    #[arg(long, short)]
    rebuild: bool,
}

fn clean_path<P: AsRef<Path> + Debug>(path: P) -> io::Result<()> {
    debug!("Requesting clean path on {:?}", path);

    match fs::metadata(&path) {
        Ok(meta) => {
            debug!("Elem exists, removing");
            if meta.is_file() {
                fs::remove_file(&path)?;
            } else if meta.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                panic!("{:?} Wasn't either a dir or a file", path);
            }
            Ok(())
        }
        Err(err) => match err.kind() {
            io::ErrorKind::NotFound => {
                debug!("Doesn't exist, skipping");
                Ok(())
            }
            _ => Err(err),
        },
    }
}

pub fn build_spec(args: BuildArgs) -> anyhow::Result<()> {
    debug!("args: {:?}", args);

    let spec = pkgs::parse(&args.file)?;
    debug!("spec: {:?}", spec);

    for p in spec.pkg {
        debug!("building pkg: {:?}", p);
        build_pkg(p, &args)?;
    }

    Ok(())
}

pub fn build_pkg(pkg: pkgs::Pkg, build_args: &BuildArgs) -> anyhow::Result<()> {
    if db::is_db_path(&pkg.path)? {
        if build_args.rebuild {
            todo!("Unregister")
        } else {
            debug!("Package was already built");
            return Ok(())
        }
    }

    clean_path(&pkg.path)?;

    let fetch_paths: Result<Vec<_>, _> = pkg.fetch.iter().map(fetch).collect();

    let fetch_paths = fetch_paths?;

    let mut env: HashMap<&str, &str> = HashMap::new();

    let env_fetch: Vec<_> = fetch_paths
        .iter()
        .map(|elem| elem.to_str().expect("Couldn't format fetch_path"))
        .collect();

    let env_fetch = &env_fetch.join(":");

    env.insert("miq_fetch", &env_fetch);
    env.insert("miq_out", &pkg.path);

    debug!("env: {:?}", env);

    let cmd_args = ["-c", &pkg.script];

    let mut cmd = Command::new("/bin/sh");
    cmd.args(&cmd_args);
    cmd.env_clear();
    cmd.envs(&pkg.env);
    cmd.envs(&env);

    if build_args.quiet {
        cmd.stdout(unshare::Stdio::Null);
        cmd.stderr(unshare::Stdio::Null);
    }

    debug!("output: {:?}", &cmd);

    let status = cmd.status();
    debug!("{:?}", status);

    db::add(&pkg.path)?;

    Ok(())
}

pub fn fetch(fch: &pkgs::Fetchable) -> anyhow::Result<PathBuf> {
    debug!("Fetching: {:?}", fch);
    let outpath = PathBuf::from_str("/tmp/miq-fetch/fetch1")?;

    if let Ok(meta) = fs::metadata(&outpath) {
        if meta.is_file() {
            debug!("Already exists");
            return Ok(outpath);
        }
    }

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
