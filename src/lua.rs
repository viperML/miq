use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::hash::Hash;
use std::io::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{bail, Context, ContextCompat};
use color_eyre::Result;
use mlua::prelude::*;
use mlua::{chunk, StdLib, Table, Value};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, trace};

use crate::eval::{MiqEvalPath, MiqResult, MiqStorePath};
use crate::schema_eval::Unit;

// impl LuaUserData for Unit {}

#[derive(Debug, clap::Args)]
/// Shorthand to the internal Lua evaluator
pub struct Args {
    /// Toplevel lua file to evaluate
    path: PathBuf,
    /// Name of the table key to evaluate
    #[clap(short, long)]
    unit: Option<String>,
}

impl crate::Main for Args {
    fn main(&self) -> Result<()> {
        evaluate(&self.path)?;
        Ok(())
    }
}

pub fn evaluate<P: AsRef<Path>>(path: P) -> Result<BTreeMap<String, Unit>> {
    let path = path.as_ref();
    let path = path.canonicalize()?;
    info!("Loading {:?}", path);

    let lua = create_lua_env()?;

    let parent = path.parent().wrap_err("Reading input file's parent")?;
    std::env::set_current_dir(parent).wrap_err(format!("Changing directory to {:?}", parent))?;

    let toplevel_export_lua: Table = lua.load(path).eval().wrap_err("Loading input file")?;

    let mut toplevel_export: BTreeMap<String, Unit> = BTreeMap::new();

    for pair in toplevel_export_lua.pairs::<LuaString, Value>() {
        let (k, v) = pair?;
        let key = k.to_str()?.to_owned();

        match lua.from_value::<Unit>(v) {
            Ok(v) => {
                toplevel_export.insert(key, v);
            }
            Err(err @ LuaError::DeserializeError(_)) => {
                trace!(?key, ?err);
            }
            Err(err) => bail!(err),
        };
    }

    debug!(?toplevel_export);

    for (_, elem) in toplevel_export.clone() {
        let result: MiqResult = elem.clone().into();
        let eval_path: MiqEvalPath = (&result).into();

        let prefix = "#:schema /miq/eval-schema.json";
        let serialized = toml::to_string_pretty(&elem)?;

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&eval_path)
            .wrap_err(format!("Opening serialisation file for {:?}", eval_path))?;

        file.write_all(prefix.as_bytes())?;
        file.write_all("\n".as_bytes())?;
        file.write_all(serialized.as_bytes())?;
    }

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

static LUA_INSPECT: &str = std::include_str!("inspect.lua");
static LUA_F: &str = std::include_str!("f.lua");

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

    module.set("interpolate", lua.create_function(interpolate)?)?;

    let f: Value = lua.load(LUA_F).eval()?;
    module.set("f", f)?;

    drop(module);
    Ok(lua)
}

fn interpolate<'lua>(
    ctx: &'lua Lua,
    value: Value<'lua>,
) -> Result<(Value<'lua>, Value<'lua>), LuaError> {
    match value {
        table @ Value::Table(_) => {
            if let Ok(unit) = ctx.from_value::<Unit>(table) {
                let miq_result: MiqResult = unit.into();
                let store_path: MiqStorePath = (&miq_result).into();
                let store_path: &Path = store_path.as_ref();
                let left = store_path.to_str().unwrap().to_owned();
                let right = miq_result.deref().clone();
                Ok((ctx.pack(left)?, ctx.pack(right)?))
            } else {
                Err(LuaError::DeserializeError("Can't interpolate value".into()))
            }
        }
        s @ Value::String(_) => Ok((s, Value::Nil)),
        _ => Err(LuaError::DeserializeError("Can't interpolate value".into())),
    }
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
