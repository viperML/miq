use mlua::prelude::*;
use mlua::{Lua, Table, Value};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::eval::MiqResult;
use crate::schema_eval::{Fetch, Unit};

/// Input to the lua fetch function, which will transform it into a proper Fetch
#[derive(Educe, Serialize, Deserialize, Hash)]
#[educe(Debug)]
pub struct FetchInput {
    #[educe(Debug(trait = "std::fmt::Display"))]
    pub url: Url,
    pub executable: Option<bool>,
}

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

        let result = MiqResult::create(&name, &value);

        let inner = Fetch {
            result,
            name,
            url: value.url.to_string(),
            integrity: String::from("FIXME"),
            executable: value.executable.unwrap_or_default(),
        };

        let unit = Unit::FetchUnit(inner);
        unit.write_to_disk().expect("Failed to write to disk");
        Ok(unit)
    }
}

fn fetch<'lua, 'result>(ctx: &'lua Lua, input: Value<'result>) -> Result<Value<'result>, LuaError>
where
    'lua: 'result,
{
    let user_input = ctx.from_value::<FetchInput>(input)?;
    let result_unit = Unit::try_from(user_input)?;
    let repr = ctx.to_value(&result_unit)?;
    // let internal_repr = ctx.create_ser_userdata(result_unit)?;
    Ok(repr)
}

pub fn add_to_module(ctx: &Lua, module: &Table) -> Result<(), LuaError> {
    module.set("fetch", ctx.create_function(fetch)?)?;
    Ok(())
}
