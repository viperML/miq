use std::fmt::Debug;
use std::io::{Read, Write};
use std::path::{self, Path, PathBuf};
use std::{fs, io};

use anyhow::{bail, Context};
use bytes::Buf;
use log::debug;

use crate::schema;

#[derive(Debug, clap::Args)]
pub struct BuildArgs {
    /// Path of PkgSpec to build
    #[arg()]
    file: PathBuf,
}

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
    let builddir = tempfile::Builder::new().prefix("miq-build-").tempfile()?;
    let builddir_path = builddir.path();

    for fetchable in pkg.fetch {
        debug!("Fetching: {:?}", fetchable);
    }

    debug!("{:?}", builddir_path);
    Ok(())
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
//     let response = client.get(pkg.url).send()?;

//     let mut content = response.bytes()?.reader();
//     debug!("Copying file");

//     std::io::copy(&mut content, &mut f)?;

//     Ok(())
// }
