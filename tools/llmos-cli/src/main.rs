mod modules;
mod output;
mod policy;
mod profile;

use clap::{Parser, Subcommand};
use modules::ModulesArgs;
use policy::PolicyCommand;
use profile::ProfileArgs;

#[derive(Parser, Debug)]
#[command(name = "llmos", version, about = "LLM-OS operator CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Modules(ModulesArgs),
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
    Profile(ProfileArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Modules(args) => modules::run_modules(args).await?,
        Command::Policy { command } => policy::run_policy(command).await?,
        Command::Profile(args) => profile::run_profile(&args)?,
    }

    Ok(())
}
