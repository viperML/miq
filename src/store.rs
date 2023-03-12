use std::any::request_ref;
use std::fmt::Debug;
use std::path::Path;
use std::{fs, io};

use anyhow::{bail, Context};
use log::debug;

use crate::expr;

fn mkdir<P: AsRef<Path> + Debug>(p: P) -> Result<(), io::Error> {
    debug!("Creating directory: {:?}", p);

    if let Err(err) = fs::create_dir(p) {
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

pub fn build(pkg: expr::FOP) -> anyhow::Result<()> {
    let builddir = Path::new("/tmp/miq-build");
    debug!("builddir: {:?}", builddir);

    mkdir(builddir)?;

    let outdir = expr::pkg_path(&pkg);
    debug!("outdir: {:?}", outdir);
    mkdir(outdir)?;


    // let client = reqwest::
    let client = reqwest::blocking::Client::new();
    // let response =


    Ok(())
}
