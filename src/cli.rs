#[derive(clap::Parser, Debug)]
pub struct CliParser {
    #[command(subcommand)]
    pub command: MiqCommands,
}

#[derive(clap::Subcommand, Debug)]
pub enum MiqCommands {
    /// Generate the package schema
    Schema(crate::schema_eval::Args),
    /// Build a package into the store
    Build(crate::build::Args),
    /// Query and operate on the store database
    Store(crate::db::CliArgs),
    /// Evaluate a unit graph
    Eval(crate::dag::Args),
}
