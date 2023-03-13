#[derive(clap::Parser, Debug)]
pub struct CliParser {
    #[command(subcommand)]
    pub command: MiqCommands,
}

#[derive(clap::Subcommand, Debug)]
pub enum MiqCommands {
    Schema,
    Build(crate::store::BuildArgs)
}
