use std::path::PathBuf;
use std::process::{Command, Stdio, Output};

use color_eyre::Result;
use tracing::{info, trace};

// Evaluate a package given its name, and return a path to the unit
// #[tracing::instrument(level = "debug", ret, err)]
// pub fn eval<S: AsRef<str> + std::fmt::Debug>(name: S) -> Result<PathBuf> {
//     let name = name.as_ref();

//     let result = crate::lua::evaluate(path)

//     Ok(result)
// }


