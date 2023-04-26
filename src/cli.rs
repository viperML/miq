#[derive(clap::Parser, Debug)]
pub struct CliParser {
    #[command(subcommand)]
    pub command: MiqCommands,
}

#[derive(clap::Subcommand, Debug)]
#[clap(disable_help_subcommand(true))]
pub enum MiqCommands {
    /// Generate the unit schema
    Schema(crate::schema_eval::Args),
    /// Build a unit into the store
    Build(crate::build::Args),
    /// Query and operate on the store database
    Store(crate::db::CliArgs),
    /// Evaluate a unit
    Eval(crate::eval::Args),
}
