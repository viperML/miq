use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

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

#[derive(serde::Deserialize, Debug)]
struct FOP {
    url: String,
}

fn main() -> anyhow::Result<()> {
    setup_logging()?;

    let file = PathBuf::from_str("/home/ayats/Documents/miq/pkgs/main.dhall")?;

    let test = serde_dhall::from_file(&file).parse::<BTreeMap<String, FOP>>()?;

    debug!("{:?}", &test);

    // let main_config = fs::read_to_string(&file)?;

    // debug!("{:?}", main_config);

    Ok(())
}
