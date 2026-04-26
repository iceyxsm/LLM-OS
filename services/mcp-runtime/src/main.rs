use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use mcp_runtime::{default_manifest_dir, load_manifests, RuntimeManager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(
    name = "mcp-runtime",
    version,
    about = "MCP runtime lifecycle manager for plugin manifests"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Validate(ManifestArgs),
    List(ManifestArgs),
    Run(RunArgs),
}

#[derive(Args, Debug)]
struct ManifestArgs {
    #[arg(long)]
    manifest_dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct RunArgs {
    #[arg(long)]
    manifest_dir: Option<PathBuf>,
    #[arg(long)]
    autostart: bool,
}

#[derive(Debug)]
enum AdminCommand {
    Start(String),
    Stop(String),
    Restart(String),
    List,
    Help,
    Quit,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .compact()
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Validate(args) => validate_manifests(args)?,
        Command::List(args) => list_manifests(args).await?,
        Command::Run(args) => run_runtime(args).await?,
    }

    Ok(())
}

fn resolve_manifest_dir(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(default_manifest_dir)
}

fn validate_manifests(args: ManifestArgs) -> anyhow::Result<()> {
    let dir = resolve_manifest_dir(args.manifest_dir);
    let manifests = load_manifests(&dir)?;
    println!("manifest_dir: {}", dir.display());
    println!("manifest_count: {}", manifests.len());
    println!("validation: ok");
    Ok(())
}

async fn list_manifests(args: ManifestArgs) -> anyhow::Result<()> {
    let dir = resolve_manifest_dir(args.manifest_dir);
    let manifests = load_manifests(&dir)?;
    let mut manager = RuntimeManager::new(manifests);
    for status in manager.list() {
        println!(
            "{}@{} running={} pid={} entrypoint={}",
            status.id,
            status.version,
            status.running,
            status
                .pid
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            status.entrypoint
        );
    }
    Ok(())
}

async fn run_runtime(args: RunArgs) -> anyhow::Result<()> {
    let manifest_dir = resolve_manifest_dir(args.manifest_dir);
    let manifests = load_manifests(&manifest_dir)?;
    let mut manager = RuntimeManager::new(manifests);

    info!(
        target: "mcp-runtime",
        manifest_dir = %manifest_dir.display(),
        manifest_count = manager.manifests_len(),
        "mcp runtime started"
    );
    info!(
        target: "mcp-runtime",
        "admin commands: start <id>, stop <id>, restart <id>, list, help, quit"
    );

    if args.autostart {
        manager.start_all().await?;
    }

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<String>(64);
    tokio::spawn(async move {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }
            if cmd_tx.send(line).await.is_err() {
                break;
            }
        }
    });

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!(target: "mcp-runtime", "received ctrl-c, shutting down");
                break;
            }
            maybe_cmd = cmd_rx.recv() => {
                match maybe_cmd {
                    Some(line) => {
                        match parse_admin_command(&line) {
                            Ok(AdminCommand::Start(id)) => manager.start(&id).await?,
                            Ok(AdminCommand::Stop(id)) => manager.stop(&id).await?,
                            Ok(AdminCommand::Restart(id)) => manager.restart(&id).await?,
                            Ok(AdminCommand::List) => {
                                for status in manager.list() {
                                    println!(
                                        "{}@{} running={} pid={}",
                                        status.id,
                                        status.version,
                                        status.running,
                                        status
                                            .pid
                                            .map(|value| value.to_string())
                                            .unwrap_or_else(|| "-".to_string()),
                                    );
                                }
                            }
                            Ok(AdminCommand::Help) => {
                                println!("commands:");
                                println!("  start <id>");
                                println!("  stop <id>");
                                println!("  restart <id>");
                                println!("  list");
                                println!("  help");
                                println!("  quit");
                            }
                            Ok(AdminCommand::Quit) => break,
                            Err(err) => warn!(target: "mcp-runtime", error = %err, "invalid admin command"),
                        }
                    }
                    None => break,
                }
            }
        }
    }

    manager.stop_all().await?;
    info!(target: "mcp-runtime", "shutdown complete");
    Ok(())
}

fn parse_admin_command(raw: &str) -> Result<AdminCommand, String> {
    let mut parts = raw.split_whitespace();
    let command = parts
        .next()
        .ok_or_else(|| "command cannot be empty".to_string())?
        .to_ascii_lowercase();

    match command.as_str() {
        "start" => parts
            .next()
            .map(|id| AdminCommand::Start(id.to_string()))
            .ok_or_else(|| "start requires a plugin id".to_string()),
        "stop" => parts
            .next()
            .map(|id| AdminCommand::Stop(id.to_string()))
            .ok_or_else(|| "stop requires a plugin id".to_string()),
        "restart" => parts
            .next()
            .map(|id| AdminCommand::Restart(id.to_string()))
            .ok_or_else(|| "restart requires a plugin id".to_string()),
        "list" => Ok(AdminCommand::List),
        "help" => Ok(AdminCommand::Help),
        "quit" | "exit" => Ok(AdminCommand::Quit),
        _ => Err(format!("unknown command '{}'", command)),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_admin_command, AdminCommand};

    #[test]
    fn parse_start_command() {
        let cmd = parse_admin_command("start my.plugin").expect("command should parse");
        match cmd {
            AdminCommand::Start(id) => assert_eq!(id, "my.plugin"),
            _ => panic!("unexpected command variant"),
        }
    }

    #[test]
    fn parse_quit_command() {
        let cmd = parse_admin_command("quit").expect("command should parse");
        assert!(matches!(cmd, AdminCommand::Quit));
    }

    #[test]
    fn parse_unknown_command_errors() {
        let err = parse_admin_command("explode").expect_err("unknown command should fail");
        assert!(err.contains("unknown command"));
    }
}
