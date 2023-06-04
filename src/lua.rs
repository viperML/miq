use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::ptr::hash;

use color_eyre::eyre::{bail, Context};
use color_eyre::Result;
use mlua::prelude::*;
use mlua::serde::de;
use mlua::{chunk, StdLib, Table, Value};
use serde::{Deserialize, Serialize};
use sha2::digest::Update;
use sha2::{Digest, Sha256};
use textwrap::dedent;
use tracing::{debug, info, trace, warn};
use url::Url;

use crate::eval::MiqResult;
use crate::lua_fetch::FetchInput;
use crate::schema_eval::{Fetch, Package, Unit};

// impl LuaUserData for Unit {}

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Toplevel lua file to evaluate
    #[clap(short, long, default_value = "pkgs.lua")]
    path: PathBuf,
    /// Name of the table key to evaluate
    #[clap(short, long)]
    unit: Option<String>,
}

impl Args {
    pub fn main(&self) -> Result<()> {
        evaluate(&self.path)?;
        Ok(())
    }
}

pub fn evaluate<P: AsRef<Path>>(path: P) -> Result<BTreeMap<String, Unit>> {
    let path = path.as_ref();
    info!("Loading {:?}", path);

    let lua = create_lua_env()?;

    let toplevel_export_lua: Table = lua.load(&std::fs::read_to_string(path)?).eval()?;

    let mut toplevel_export: BTreeMap<String, Unit> = BTreeMap::new();

    for pair in toplevel_export_lua.pairs::<LuaString, Value>() {
        let (k, v) = pair?;
        let k = k.to_str()?.to_owned();
        let v: Unit = lua.from_value(v)?;

        toplevel_export.insert(k, v);
    }

    debug!(?toplevel_export);

    // for (_, elem) in &toplevel_export {
    //     let result = match &elem {
    //         Unit::PackageUnit(inner) => &inner.result,
    //         Unit::FetchUnit(inner) => &inner.result,
    //     };

    //     let path = format!("/miq/eval/{}.toml", result);
    //     trace!(?path);

    //     let serialized = toml::to_string_pretty(&elem)?;
    //     std::fs::write(path, serialized)?;
    // }

    Ok(toplevel_export)
}

pub fn get_or_create_module<'lua, 'module>(lua: &'lua Lua, name: &str) -> Result<Table<'module>>
where
    'lua: 'module,
{
    let globals = lua.globals();
    let package: Table = globals.get("package")?;
    let loaded: Table = package.get("loaded")?;

    let module = loaded.get(name)?;
    match module {
        Value::Nil => {
            let module = lua.create_table()?;
            loaded.set(name, module.clone())?;
            Ok(module)
        }
        Value::Table(table) => Ok(table),
        wat => bail!(
            "cannot register module {} as package.loaded.{} is already set to a value of type {}",
            name,
            name,
            wat.type_name()
        ),
    }
}

// #[tracing::instrument(level = "trace")]
// fn trace_table(t: Table) {
//     for (key, value) in t.pairs::<LuaValue, LuaValue>().flatten() {
//         if let Value::String(key) = key {
//             let key = key.to_str().unwrap();
//             trace!(?key, ?value);
//         } else {
//             trace!(?key, ?value);
//         }
//     }
// }

static LUA_INSPECT: &'static str = std::include_str!("inspect.lua");
static LUA_F: &'static str = std::include_str!("f.lua");

fn create_lua_env() -> Result<Lua> {
    let lua = unsafe {
        Lua::unsafe_new_with(
            // Needed for f-string shenanigans
            StdLib::ALL_SAFE | StdLib::DEBUG,
            LuaOptions::new(), // .catch_rust_panics(false),
        )
    };

    let module = get_or_create_module(&lua, "miq")?;

    let inspect = lua.load(LUA_INSPECT).eval::<Table>()?;
    module.set("inspect", inspect)?;

    module.set(
        "hello",
        lua.create_function(|_, _: Value| {
            eprintln!("🦀 Hello World! 🦀");
            Ok(())
        })?,
    )?;

    crate::lua_fetch::add_to_module(&lua, &module)?;
    crate::lua_package::add_to_module(&lua, &module)?;

    module.set(
        "trace",
        lua.create_function(|ctx, input: Value| {
            let inspect: Table = ctx
                .load(chunk! {
                    return (require("miq")).inspect
                })
                .eval()?;
            let inspected: LuaString = inspect.call(input.clone())?;
            let s = inspected.to_str()?;
            trace!("luatrace>> {}", s);
            Ok(())
        })?,
    )?;

    module.set(
        "get_result",
        lua.create_function(|ctx: &Lua, input: Table| {
            let input: Unit = ctx.from_value(Value::Table(input))?;
            let miqresult: MiqResult = input.into();
            let res = ctx.to_value(&miqresult)?;
            Ok(res)
        })?,
    )?;

    let f: Value = lua.load(LUA_F).eval()?;
    module.set("f", f)?;

    drop(module);
    Ok(lua)
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Educe)]
#[educe(Default)]
#[serde(untagged)]
pub enum MetaTextInput {
    #[educe(Default)]
    Simple(String),
    Full(MetaText),
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Default)]
pub struct MetaText {
    pub deps: Vec<MiqResult>,
    pub value: String,
}
