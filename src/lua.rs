use std::hash::{Hash, Hasher};
use std::ptr::hash;

use color_eyre::Result;
use mlua::chunk;
use mlua::prelude::*;
use mlua::serde::de;
use serde::{Deserialize, Serialize};
use sha2::digest::Update;
use sha2::{Digest, Sha256};
use tracing::{debug, info};
use url::Url;

use crate::schema_eval::{Fetch, Unit};

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
            let deser = compute_unit(deser);
            let deser = deser.expect("FATAL");

            info!(?deser);

            Ok(deser)
        })?;

        lua.load(chunk! {
            mqf = {
                mk_fetch = $lua_mk_fetch
            }
        })
        .exec()?;

        lua.load(chunk! {
            // pkgs.lua
            pkg_test = mqf.mk_fetch({
                url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz"
            })

            busybox = mqf.mk_fetch({
                url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/busybox",
                executable = true,
            })

            print(pkg_test)
        })
        .exec()?;

        let x: Unit = globals.get("pkg_test")?;
        info!(?x);

        let test_result: LuaValue = globals.get("pkg_test")?;
        info!(?test_result);

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Hash)]
struct FetchInput {
    url: Url,
    executable: Option<bool>,
}

fn hash_string<H: Hash>(input: &H) -> String {
    let mut hasher = fnv::FnvHasher::default();
    input.hash(&mut hasher);
    let result = hasher.finish();
    let s = format!("{:x}", result);
    s
}

fn compute_fetch(input: FetchInput) -> Result<Fetch> {
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
    debug!(?path);

    let result = Fetch {
        result,
        name,
        url: input.url.to_string(),
        integrity: String::from("FIXME"),
        executable: input.executable.unwrap_or_default(),
    };

    let serialized = toml::to_string_pretty(&result)?;

    std::fs::write(path, serialized)?;

    Ok(result)
}

fn compute_unit(input: FetchInput) -> Result<Unit> {
    Ok(Unit::FetchUnit(compute_fetch(input)?))
}

impl LuaUserData for Unit {}
