use clap::{Parser, Subcommand};
use common_types::ModuleDescriptor;

#[derive(Parser, Debug)]
#[command(name = "llmos", version, about = "LLM-OS operator CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Modules,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Modules => {
            let modules = vec![
                ModuleDescriptor {
                    id: "services/llmd".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    status: "scaffolded".to_string(),
                },
                ModuleDescriptor {
                    id: "services/mcp-runtime".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    status: "scaffolded".to_string(),
                },
                ModuleDescriptor {
                    id: "services/policy-engine".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    status: "scaffolded".to_string(),
                },
            ];

            for m in modules {
                println!("{}@{} [{}]", m.id, m.version, m.status);
            }
        }
    }

    Ok(())
}
