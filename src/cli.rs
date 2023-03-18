#[derive(clap::Parser, Debug)]
pub struct CliParser {
    #[command(subcommand)]
    pub command: MiqCommands,
}

#[derive(clap::Subcommand, Debug)]
pub enum MiqCommands {
    /// Generate the package schema
    Schema,
    /// Build a package into the store
    Build(crate::build::BuildArgs),
    /// Query and operate on the store database
    Db(crate::db::CliArgs),
}
