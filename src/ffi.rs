use std::path::PathBuf;
use std::process::{Command, Stdio, Output};

use color_eyre::Result;
use tracing::{info, trace};

/// Evaluate a package given its name, and return a path to the unit
#[tracing::instrument(level = "debug", ret, err)]
pub fn eval<S: AsRef<str> + std::fmt::Debug>(name: S) -> Result<PathBuf> {
    let name = name.as_ref();

    let output = Command::new("python3")
        .args(["-m", "miq_eval", name])
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .stdout(Stdio::piped())
        .output()?;

    trace!(?output);

    // match output {
    //     Output { status: ExitStatus}
    // }

    let s = std::str::from_utf8(&output.stdout)?.trim();

    let path_string = format!("/miq/eval/{}.toml", s);
    let path = PathBuf::from(path_string);

    Ok(path)
}
