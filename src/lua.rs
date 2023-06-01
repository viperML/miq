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

use crate::schema_eval::{Fetch, Package, Unit};

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Toplevel lua file to evaluate
    #[clap(short, long, default_value = "pkgs.lua")]
    path: PathBuf,
    /// Name of the table key to evaluate
    #[clap(short, long)]
    unit: Option<String>,
}

pub fn get_or_create_module<'lua>(lua: &'lua Lua, name: &str) -> Result<mlua::Table<'lua>> {
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

#[tracing::instrument(level = "trace")]
fn trace_table(t: Table) {
    for (key, value) in t.pairs::<LuaValue, LuaValue>().flatten() {
        if let Value::String(key) = key {
            let key = key.to_str().unwrap();
            trace!(?key, ?value);
        } else {
            trace!(?key, ?value);
        }
    }
}

static LUA_INSPECT: &'static str = std::include_str!("inspect.lua");

/// Add some utility modules to be require-able
fn preload_modules(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();

    let package: Table = globals.get("package")?;
    let loaded: Table = package.get("loaded")?;

    let inspect: Value = lua.load(LUA_INSPECT).eval()?;
    loaded.set("inspect", inspect)?;

    Ok(())
}

impl Args {
    pub fn main(&self) -> Result<()> {
        evaluate(&self.path);
        Ok(())
    }
}

pub fn evaluate<P: AsRef<Path>>(path: P) -> Result<BTreeMap<String, Unit>> {
    let path = path.as_ref();
    info!("Loading {:?}", path);

    let lua = unsafe {
        Lua::unsafe_new_with(
            // Needed for f-string shenanigans
            StdLib::ALL_SAFE | StdLib::DEBUG,
            LuaOptions::new().catch_rust_panics(false),
        )
    };

    let globals = lua.globals();
    preload_modules(&lua)?;

    let module = get_or_create_module(&lua, "miq")?;

    module.set(
        "hello",
        lua.create_function(|_, _: Value| {
            println!("Hello from rust! 🦀");
            Ok(())
        })?,
    )?;

    module.set(
        "fetch",
        lua.create_function(|ctx, input: Value| {
            let user_input = ctx.from_value::<FetchInput>(input)?;
            let result_unit = Unit::try_from(user_input)?;
            let internal_repr = ctx.create_ser_userdata(result_unit)?;
            Ok(internal_repr)
        })?,
    )?;

    module.set(
        "package",
        lua.create_function(|ctx, input: Value| {
            let user_input: PackageInput = ctx.from_value(input)?;
            let result_unit = Unit::try_from(user_input)?;
            let internal_repr = ctx.create_ser_userdata(result_unit)?;
            Ok(internal_repr)
        })?,
    )?;

    let toplevel_export_lua: Table = lua.load(&std::fs::read_to_string(path)?).eval()?;

    let mut toplevel_export: BTreeMap<String, Unit> = BTreeMap::new();

    for pair in toplevel_export_lua.pairs::<LuaString, Value>() {
        let (k, v) = pair?;
        let k = k.to_str()?.to_owned();
        let v: Unit = lua.from_value(v)?;

        toplevel_export.insert(k, v);
    }

    debug!(?toplevel_export);

    for (name, elem) in &toplevel_export {
        let result = match &elem {
            Unit::PackageUnit(inner) => &inner.result,
            Unit::FetchUnit(inner) => &inner.result,
        };

        let path = format!("/miq/eval/{}.toml", result);
        trace!(?path);

        let serialized = toml::to_string_pretty(&elem)?;
        std::fs::write(path, serialized)?;
    }

    Ok(toplevel_export)
}

/// Input to the lua fetch function, which will transform it into a proper Fetch
#[derive(Educe, Serialize, Deserialize, Hash)]
#[educe(Debug)]
struct FetchInput {
    #[educe(Debug(trait = "std::fmt::Display"))]
    url: Url,
    executable: Option<bool>,
}

/// Input to the lua package function, which will transform it into a proper Package
#[derive(Debug, Serialize, Deserialize, Hash)]
struct PackageInput {
    name: String,
    version: Option<String>,
    script: Option<String>,
    deps: Option<Vec<Unit>>,
    env: Option<BTreeMap<String, String>>,
}

fn hash_string<H: Hash>(input: &H) -> String {
    let mut hasher = fnv::FnvHasher::default();
    input.hash(&mut hasher);
    let result = hasher.finish();
    let s = format!("{:x}", result);
    s
}

impl LuaUserData for Unit {}

impl TryFrom<FetchInput> for Unit {
    type Error = LuaError;

    #[tracing::instrument(level = "trace", ret, err)]
    fn try_from(value: FetchInput) -> std::result::Result<Self, Self::Error> {
        let name = value
            .url
            .path_segments()
            .expect("URL doesn't have segments")
            .last()
            .unwrap()
            .to_owned();

        let hash = hash_string(&value);
        let result = format!("{}-{}", name, hash);

        let result = Fetch {
            result,
            name,
            url: value.url.to_string(),
            integrity: String::from("FIXME"),
            executable: value.executable.unwrap_or_default(),
        };

        Ok(Unit::FetchUnit(result))
    }
}

impl TryFrom<PackageInput> for Unit {
    type Error = LuaError;

    #[tracing::instrument(level = "trace", ret, err)]
    fn try_from(value: PackageInput) -> std::result::Result<Self, Self::Error> {
        let hash = hash_string(&value);
        let result = format!("{}-{}", value.name, hash);

        let deps = value
            .deps
            .unwrap_or_default()
            .iter()
            .map(|elem| match elem {
                Unit::PackageUnit(inner) => inner.result.clone(),
                Unit::FetchUnit(inner) => inner.result.clone(),
            })
            .collect::<Vec<_>>();

        trace!(?deps);

        let result = Package {
            result,
            name: value.name,
            version: value.version.unwrap_or_default(),
            script: dedent(&value.script.unwrap_or_default()),
            env: value.env.unwrap_or_default(),
            deps,
        };

        Ok(Unit::PackageUnit(result))
    }
}

// module.set("f", lua.create_function(lua_f_string)?)?;
// lua.load(chunk! {
//     local inspect = require "inspect"

// function copy(t)
//   if type(t) == "table" then
//     local ans = {}
//     for k,v in next,t do ans[ k ] = v end
//     return ans
//   end
//   return t
// end

// function f(s)
//   local env = copy(_ENV)
//   local i,k,v,fmt = 0
//   repeat
//     i = i + 1
//     k,v = debug.getlocal(2,i)
//     if k ~= nil then env[k] = v end
//   until k == nil
//   print(inspect(env))
// end

//         })
// .exec()?;

// #[tracing::instrument(level = "debug", ret, err)]
// fn lua_f_string(ctx: &Lua, input: LuaString) -> Result<LuaNumber, LuaError> {
//     let globals = ctx.globals();

//     ctx.load(chunk! {
// function copy(t)
//   if type(t) == "table" then
//     local ans = {}
//     for k,v in next,t do ans[ k ] = v end
//     return ans
//   end
//   return t
// end

// local env = copy(_ENV)
//   local i,k,v,fmt = 0
//   repeat
//     i = i + 1
//     k,v = debug.getlocal(2,i)
//     if k ~= nil then env[k] = v end
//   until k == nil

//   print(inspect(env))
//     }).exec()?;

//     Ok(0.0)
// }
