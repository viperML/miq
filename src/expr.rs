use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    str::FromStr,
};

const STORE_PATH: &str = "/myq/store";

#[derive(serde::Deserialize, Debug, Hash)]
pub struct FOP {
    url: String,
    pname: String,
    version: String,
}

pub fn pkg_path(pkg: &FOP) -> PathBuf {
    let base_path = Path::new(STORE_PATH);

    // base_path.join()

    let mut hasher = DefaultHasher::new();
    pkg.hash(&mut hasher);
    let result = hasher.finish();
    let folder_name = format!("{}-{}-{}", result, pkg.pname, pkg.version);

    base_path.join(folder_name)
}
