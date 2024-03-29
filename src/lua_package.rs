use std::collections::{BTreeMap, BTreeSet};

use mlua::prelude::*;
use mlua::{Lua, Table, Value};
use serde::{Deserialize, Serialize};
use textwrap::dedent;
use tracing::trace;

use crate::eval::MiqResult;
use crate::lua::MetaTextInput;
use crate::schema_eval::{Package, Unit};

/// Input to the lua package function, which will transform it into a proper Package
#[derive(Debug, Serialize, Deserialize, Hash)]
struct PackageInput {
    name: String,
    version: Option<String>,
    script: MetaTextInput,
    deps: Option<Vec<Unit>>,
    env: Option<BTreeMap<String, MetaTextInput>>,
}

fn package<'lua, 'result>(ctx: &'lua Lua, input: Value<'result>) -> Result<Value<'result>, LuaError>
where
    'lua: 'result,
{
    let user_input: PackageInput = ctx.from_value(input)?;
    // trace!(?user_input);
    let result_unit = Unit::try_from(user_input)?;
    let repr = ctx.to_value(&result_unit)?;
    Ok(repr)
}

pub fn add_to_module(ctx: &Lua, module: &Table) -> Result<(), LuaError> {
    module.set("package", ctx.create_function(package)?)?;
    Ok(())
}

impl TryFrom<PackageInput> for Unit {
    type Error = LuaError;

    fn try_from(value: PackageInput) -> std::result::Result<Self, Self::Error> {
        let human_readable = match value.version {
            None => value.name.clone(),
            Some(ref version) => format!("{}-{}", &value.name, version),
        };
        let result = MiqResult::create(&human_readable, &value);

        // Collect into Set to remove dupes
        let mut deps = value
            .deps
            .unwrap_or_default()
            .iter()
            .map(|elem| match elem {
                Unit::PackageUnit(inner) => inner.result.clone(),
                Unit::FetchUnit(inner) => inner.result.clone(),
            })
            .collect::<BTreeSet<_>>();

        trace!(?deps);

        let script = match value.script {
            MetaTextInput::Simple(inner) => inner,
            MetaTextInput::Full(inner) => {
                deps.extend(inner.deps);
                inner.value
            }
        };

        let meta_env = value.env.unwrap_or_default();

        let env = meta_env
            .into_iter()
            .map(|(k, v)| {
                let new_val = match v {
                    MetaTextInput::Simple(inner) => inner,
                    MetaTextInput::Full(inner) => {
                        deps.extend(inner.deps);
                        inner.value
                    }
                };

                (k, new_val)
            })
            .collect();

        let script = dedent(&script);

        let result = Package {
            result,
            name: value.name,
            version: value.version,
            script,
            env,
            deps,
        };

        let unit = Unit::PackageUnit(result);
        unit.write_to_disk().expect("Failed to write to disk");
        Ok(unit)
    }
}
