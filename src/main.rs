mod expr;
mod store;

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::str::FromStr;

use log::debug;

fn setup_logging() -> anyhow::Result<()> {
    let loglevel = log::LevelFilter::Debug;

    fern::Dispatch::new()
        .format(|out, message, record| out.finish(format_args!("[{}] {}", record.level(), message)))
        .level(loglevel)
        .chain(std::io::stdout())
        .apply()?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    setup_logging()?;

    let file = PathBuf::from_str("/home/ayats/Documents/miq/pkgs/main.dhall")?;

    let pkgs = serde_dhall::from_file(&file).parse::<BTreeMap<String, expr::FOP>>()?;

    debug!("{:?}", &pkgs);

    for (k,v) in pkgs {
        debug!("{:?}", v);
        let path = expr::pkg_path(&v);
        debug!("{:?}", path);
    }

    Ok(())
}
