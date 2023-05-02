use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ptr::hash;

use color_eyre::Result;
use mlua::chunk;
use mlua::prelude::*;
use mlua::serde::de;
use serde::{Deserialize, Serialize};
use sha2::digest::Update;
use sha2::{Digest, Sha256};
use tracing::{debug, info, trace};
use url::Url;

use crate::schema_eval::{Fetch, Package, Unit};

#[derive(Debug, clap::Args)]
pub struct Args {}

impl Args {
    pub fn main(&self) -> Result<()> {
        let lua = Lua::new();
        let globals = lua.globals();

        let lua_print_table = lua.create_function(|ctx, input: LuaTable| {
            for elem in input.pairs::<LuaValue, LuaValue>() {
                if let Ok((key, value)) = elem {
                    ctx.load(chunk! {
                        print("key:\t", $key, "val:\t", $value)
                    })
                    .exec()?;
                }
            }
            Ok(())
        })?;

        globals.set("print_table", lua_print_table)?;

        let lua_mk_fetch = lua.create_function(|ctx, input: LuaValue| {
            // -
            let deser = ctx.from_value::<FetchInput>(input)?;
            let deser = compute_fetch(deser);
            let deser = deser.expect("FATAL");

            trace!(?deser);

            Ok(deser)
        })?;

        let lua_mk_package = lua.create_function(|ctx, input: LuaValue| {
            // -
            let deser = ctx.from_value::<PackageInput>(input)?;
            let deser = compute_package(deser);
            let deser = deser.expect("FATAL");

            trace!(?deser);

            Ok(deser)
        })?;

        lua.load(chunk! {
            mqf = {
                mk_fetch = $lua_mk_fetch,
                mk_package = $lua_mk_package,
            }
        })
        .exec()?;

        lua.load(chunk! {
            // pkgs.lua
            local mk_fetch = mqf.mk_fetch
            local mk_package = mqf.mk_package

            pkg_test = mk_fetch {
                url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz"
            }

            busybox = mk_fetch {
                url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/busybox",
                executable = true,
            }

            bootstrap = mk_package {
                name = "bootstrap",
                version = "0.1.0",
            }

            print(pkg_test)
        })
        .exec()?;

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Hash)]
struct FetchInput {
    url: Url,
    executable: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Hash)]
struct PackageInput {
    name: String,
    version: Option<String>,
    deps: Option<Vec<String>>,
    script: Option<String>,
}

fn hash_string<H: Hash>(input: &H) -> String {
    let mut hasher = fnv::FnvHasher::default();
    input.hash(&mut hasher);
    let result = hasher.finish();
    let s = format!("{:x}", result);
    s
}

fn compute_fetch(input: FetchInput) -> Result<Unit> {
    let name = input
        .url
        .path_segments()
        .expect("URL doesn't have segments")
        .last()
        .unwrap()
        .to_owned();

    let hash = hash_string(&input);
    let result = format!("{}-{}", name, hash);
    let path = format!("/miq/eval/{}.toml", result);
    trace!(?path);

    let result = Fetch {
        result,
        name,
        url: input.url.to_string(),
        integrity: String::from("FIXME"),
        executable: input.executable.unwrap_or_default(),
    };

    let serialized = toml::to_string_pretty(&result)?;
    std::fs::write(path, serialized)?;

    Ok(Unit::FetchUnit(result))
}

fn compute_package(input: PackageInput) -> Result<Unit> {
    let hash = hash_string(&input);
    let result = format!("{}-{}", input.name, hash);
    let path = format!("/miq/eval/{}.toml", result);
    trace!(?path);

    let result = Package {
        result,
        name: input.name,
        version: input.version.unwrap_or_default(),
        deps: input.deps.unwrap_or_default(),
        script: input.script.unwrap_or_default(),
        env: HashMap::new(),
    };

    let serialized = toml::to_string_pretty(&result)?;
    std::fs::write(path, serialized)?;

    Ok(Unit::PackageUnit(result))
}

impl LuaUserData for Unit {}
