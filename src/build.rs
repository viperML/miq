use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{self, Path, PathBuf};
use std::str::FromStr;
use std::{fs, io, vec};

use bytes::Buf;
use tracing::{debug, info};
use tempfile::tempfile;

use std::process::Command;

use crate::schema_eval::{self, Fetchable};
use crate::*;

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

pub fn clean_path<P: AsRef<Path> + Debug>(path: P) -> io::Result<()> {
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

pub fn build_spec(args: BuildArgs) -> Result<()> {
    debug!("args: {:?}", args);

    let spec = schema_eval::parse(&args.file)?;
    debug!("spec: {:?}", spec);

    // Sequentially process fetches and then buildables
    // Eventually use a solver to apply any correct ordering between fetches and pkgs

    for fetchable in spec.fetch {
        fetchable.fetch()?;
    }

    for p in spec.pkg {
        debug!("processing: {:?}", p);
        build_pkg(p, &args)?;
    }

    Ok(())
}

pub fn build_pkg(pkg: schema_eval::Pkg, build_args: &BuildArgs) -> Result<()> {
    if db::is_db_path(&pkg.path)? {
        if build_args.rebuild {
            debug!("Rebuilding pkg, unregistering from the store");
            db::remove(&pkg.path)?;
        } else {
            debug!("Package was already built");
            return Ok(());
        }
    }

    let mut miq_env: HashMap<&str, &str> = HashMap::new();
    miq_env.insert("miq_out", pkg.path.to_str().unwrap());


    // FIXME
    miq_env.insert("HOME", "/home/ayats");
    debug!("env: {:?}", miq_env);

    let mut cmd = Command::new("/bin/sh");
    cmd.args(["-c", &pkg.script]);
    cmd.env_clear();
    cmd.envs(&pkg.env);
    cmd.envs(&miq_env);

    let sandbox = sandbox::SandBox {};
    sandbox.run(&mut cmd)?;

    db::add(&pkg.path)?;

    Ok(())
}

impl Fetchable {
    /// Main function for a fetchable
    fn fetch(&self) -> Result<()> {
        debug!("Fetching: {:?}", self);

        let already_fetched = db::is_db_path(&self.path)?;

        if already_fetched {
            debug!("Already fetched!");
            return Ok(());
        }

        let tempfile = &mut tempfile::NamedTempFile::new()?;
        // FIXME
        // let tempfile = RefCell::new(tempfile::NamedTempFile::new());
        debug!("tempfile: {:?}", &tempfile);

        let client = reqwest::blocking::Client::new();
        let response = client.get(&self.url).send()?;
        let content = &mut response.bytes()?.reader();
        std::io::copy(content, tempfile)?;

        debug!("Fetch Ok");

        std::fs::copy(tempfile.path(), &self.path)?;
        debug!("Move OK");

        // Make sure we don't drop before
        // drop(tempfile);

        db::add(&self.path)?;

        Ok(())
    }
}
