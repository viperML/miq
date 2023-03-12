use log::debug;
use schemars::{schema_for, JsonSchema};

#[derive(JsonSchema)]
pub struct Pkg {
    pub pname: String,
}

pub fn build() -> anyhow::Result<()> {
    let schema = schema_for!(Pkg);
    let schema_str = serde_json::to_string_pretty(&schema)?;

    println!("{}", &schema_str);

    Ok(())
}
