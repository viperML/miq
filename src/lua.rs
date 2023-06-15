use std::hash::Hash;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use color_eyre::eyre::{bail, Context, ContextCompat};
use color_eyre::{Help, Report, Result};
use mlua::prelude::*;
use mlua::{chunk, StdLib, Table, Value};
use serde::{Deserialize, Serialize};
use tracing::{instrument, span, trace, Level};

use crate::eval::{MiqResult, MiqStorePath, RefToUnit};
use crate::schema_eval::Unit;

// impl LuaUserData for Unit {}

#[derive(Debug, clap::Args)]
/// Reference implementation of the evaluator, in Lua
pub struct Args {
    /// LuaRef to evaluate, for example ./pkgs/init.lua#bootstrap.busybox
    luaref: LuaRef,
}

impl crate::Main for Args {
    fn main(&self) -> Result<()> {
        let lua = create_lua_env()?;
        self.luaref.get_toplevel(&lua)?;
        Ok(())
    }
}

#[derive(Debug, Clone, clap::Args)]
/// How the user refers to something that uses the Lua evaluators and returns a Unit
pub struct LuaRef {
    /// Luafile to evaluate
    root: PathBuf,
    /// element to evaluate
    element: Option<Vec<String>>,
}

impl FromStr for LuaRef {
    type Err = Report;

    #[instrument(ret, err, level = "trace")]
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let mut result = match *s.split('#').collect::<Vec<_>>() {
            [root] => Self {
                root: PathBuf::from(root),
                element: None,
            },
            [root, element] => Self {
                root: PathBuf::from(root),
                element: Some(element.split(".").map(str::to_owned).collect()),
            },
            _ => bail!(format!("Couldn't match a Luaref from: {}", s)),
        };

        result.root = result.root.canonicalize()?;

        Ok(result)
    }
}

impl LuaRef {
    pub fn get_toplevel<'lua, 'result>(&self, lua: &'lua Lua) -> Result<Table<'result>>
    where
        'lua: 'result,
    {
        let parent_path = self.root.parent().wrap_err("Reading the parent folder")?;
        std::env::set_current_dir(parent_path)
            .wrap_err(format!("Changing directory to {:?}", parent_path))?;

        let export: Table = lua
            .load(self.root.as_path())
            .eval()
            .wrap_err("Loading root file")?;

        luatrace(&lua, export.clone())?;

        Ok(export)
    }
}

impl RefToUnit for LuaRef {
    fn ref_to_unit(&self) -> Result<Unit> {
        let lua = create_lua_env()?;
        let mut export: Table = self.get_toplevel(&lua)?;

        let mut elements = match &self.element {
            None => bail!("Didn't specify which element to evaluate"),
            Some(e) => e,
        }
        .to_owned();

        let err_msg = format!("Deserializing element {:?}", elements.join("."));

        let final_elem = elements.pop().wrap_err("Elements was empty")?;

        for elem in elements {
            let err_msg = format!("Trying to read element {}", elem);
            export = export.get(elem.to_owned()).wrap_err(err_msg)?;
        }

        let result: Value = export.get(final_elem.as_str())?;
        let result: Unit = lua
            .from_value(result)
            .wrap_err(err_msg)
            .suggestion("Did you use a incorrent LuaRef?")?;

        Ok(result)
    }
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

// static LUA_INSPECT: &str = std::include_str!("inspect.lua");
// static LUA_F: &str = std::include_str!("f.lua");

fn create_lua_env() -> Result<Lua> {
    let lua = unsafe {
        Lua::unsafe_new_with(
            // Needed for f-string shenanigans
            StdLib::ALL_SAFE | StdLib::DEBUG,
            LuaOptions::new(), // .catch_rust_panics(false),
        )
    };

    let module = get_or_create_module(&lua, "miq")?;

    load_from_bundle(&lua, &module, "inspect")?;

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
        lua.create_function(|ctx, val: Value| luatrace(ctx, val))?,
    )?;
    module.set("interpolate", lua.create_function(interpolate)?)?;
    module.set("dedent", lua.create_function(dedent)?)?;

    load_from_bundle(&lua, &module, "f")?;

    drop(module);

    Ok(lua)
}

static LUA_INSPECT: &str = std::include_str!("lua/inspect.lua");
static LUA_F: &str = std::include_str!("lua/f.lua");

fn load_from_bundle(ctx: &Lua, module: &Table, name: &str) -> Result<()> {
    let string = match name {
        "f" => LUA_F,
        "inspect" => LUA_INSPECT,
        _ => todo!("Read any file"),
    };

    let export: Value = ctx.load(string).eval()?;
    module.set(name, export)?;

    Ok(())
}

#[instrument(skip(ctx, input), level = "trace")]
fn luatrace<'value, 'lua, V: mlua::prelude::IntoLua<'value>>(
    ctx: &'lua Lua,
    input: V,
) -> Result<(), LuaError>
where
    'lua: 'value,
{
    let inspect: Table = ctx
        .load(chunk! {
            return (require("miq")).inspect
        })
        .eval()?;

    let inspected: LuaString = inspect.call(input)?;
    let s = inspected.to_str()?;
    // let span = span!(Level::TRACE, "luatrace");
    // let _enter = span.enter();
    trace!("luatrace>> {}", s);
    Ok(())
}

#[instrument(ret, err, level = "trace")]
fn interpolate<'lua>(
    ctx: &'lua Lua,
    value: Value<'lua>,
    //       Text         Deps
) -> Result<(Value<'lua>, Value<'lua>), LuaError> {
    match value {
        table @ Value::Table(_) => {
            if let Ok(unit) = ctx.from_value::<Unit>(table.clone()) {
                let miq_result: MiqResult = unit.into();
                let store_path: MiqStorePath = (&miq_result).into();
                let store_path: &Path = store_path.as_ref();
                let left = store_path.to_str().unwrap().to_owned();
                let right = miq_result.deref().clone();
                Ok((ctx.pack(left)?, ctx.pack(right)?))
            } else if let Ok(mt) = ctx.from_value::<MetaText>(table) {
                let right = mt
                    .deps
                    .into_iter()
                    .map(|r| {
                        let r: &str = r.as_ref();
                        r.to_owned()
                    })
                    .collect::<Vec<String>>();
                let left = mt.value;
                Ok((ctx.pack(left).unwrap(), ctx.pack(right).unwrap()))
            } else {
                Err(LuaError::DeserializeError("Can't interpolate value".into()))
            }
        }
        s @ Value::String(_) => Ok((s, Value::Nil)),
        _ => Err(LuaError::DeserializeError("Can't interpolate value".into())),
    }
}

#[instrument(ret, err, level = "trace")]
fn dedent<'lua>(ctx: &'lua Lua, s: LuaString<'lua>) -> Result<Value<'lua>, LuaError> {
    let s = s.to_str()?;
    let s_dedent = textwrap::dedent(s);
    let result = ctx.pack(s_dedent)?;
    Ok(result)
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
