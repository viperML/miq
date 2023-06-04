use std::collections::BTreeMap;

use mlua::{prelude::*, chunk};
use mlua::{Lua, Table, Value};
use serde::{Deserialize, Serialize};
use textwrap::dedent;
use tracing::trace;
use url::Url;

use crate::eval::MiqResult;
use crate::lua::MetaTextInput;
use crate::schema_eval::{Fetch, Unit, Package};

/// Input to the lua package function, which will transform it into a proper Package
#[derive(Debug, Serialize, Deserialize, Hash)]
struct PackageInput {
    name: String,
    version: Option<String>,
    script: Option<MetaTextInput>,
    deps: Option<Vec<Unit>>,
    env: Option<BTreeMap<String, String>>,
}

fn package<'lua, 'result>(
    ctx: &'lua Lua,
    input: Value<'result>,
) -> Result<LuaAnyUserData<'result>, LuaError>
where
    'lua: 'result,
{
    let input_repr = input.clone();
    ctx.load(chunk! {
        local miq = require("miq")
        miq.trace($input_repr)
    })
    .exec()?;

    let user_input: PackageInput = ctx.from_value(input)?;
    let result_unit = Unit::try_from(user_input)?;
    let internal_repr = ctx.create_ser_userdata(result_unit)?;
    Ok(internal_repr)
}


impl TryFrom<PackageInput> for Unit {
    type Error = LuaError;

    fn try_from(value: PackageInput) -> std::result::Result<Self, Self::Error> {
        let result = MiqResult::create(&value.name, &value);

        let mut deps = value
            .deps
            .unwrap_or_default()
            .iter()
            .map(|elem| match elem {
                Unit::PackageUnit(inner) => inner.result.clone(),
                Unit::FetchUnit(inner) => inner.result.clone(),
            })
            .collect::<Vec<_>>();

        trace!(?deps);

        let script = match value.script.unwrap_or_default() {
            MetaTextInput::Simple(inner) => inner,
            MetaTextInput::Full(inner) => {
                deps.extend(inner.deps);
                inner.value
            }
        };
        // let meta_text = value.script.unwrap_or_default();
        // let script = meta_text.value;
        // deps.extend(meta_text.deps);

        let script = dedent(&script);

        let result = Package {
            result,
            name: value.name,
            version: value.version.unwrap_or_default(),
            script,
            env: value.env.unwrap_or_default(),
            deps,
        };

        Ok(Unit::PackageUnit(result))
    }
}
