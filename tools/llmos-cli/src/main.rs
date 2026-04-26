use clap::{Args, Parser, Subcommand};
use common_types::ModuleDescriptor;
use controlplane_api::{policy_service_client::PolicyServiceClient, EvaluatePolicyRequest};
use tonic::metadata::MetadataValue;

#[derive(Parser, Debug)]
#[command(name = "llmos", version, about = "LLM-OS operator CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Modules,
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
}

#[derive(Subcommand, Debug)]
enum PolicyCommand {
    Check(PolicyCheckArgs),
}

#[derive(Args, Debug)]
struct PolicyCheckArgs {
    #[arg(long)]
    subject: String,
    #[arg(long)]
    action: String,
    #[arg(long)]
    resource: String,
    #[arg(long, default_value = "http://127.0.0.1:50051")]
    endpoint: String,
    #[arg(long, default_value_t = 2)]
    timeout_secs: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
        Command::Policy { command } => match command {
            PolicyCommand::Check(args) => run_policy_check(args).await?,
        },
    }

    Ok(())
}

async fn run_policy_check(args: PolicyCheckArgs) -> anyhow::Result<()> {
    let request_id = generate_id("cli-req");
    let correlation_id = generate_id("cli-corr");
    let mut client = PolicyServiceClient::connect(args.endpoint.clone()).await?;

    let mut request = tonic::Request::new(EvaluatePolicyRequest {
        subject: args.subject,
        action: args.action,
        resource: args.resource,
    });

    request.metadata_mut().insert(
        "x-request-id",
        MetadataValue::try_from(request_id.as_str())?,
    );
    request.metadata_mut().insert(
        "x-correlation-id",
        MetadataValue::try_from(correlation_id.as_str())?,
    );

    let response = tokio::time::timeout(
        std::time::Duration::from_secs(args.timeout_secs),
        client.evaluate(request),
    )
    .await??
    .into_inner();

    println!("decision: {}", response.effect);
    println!("reason: {}", response.reason);
    if response.rule_id.is_empty() {
        println!("rule_id: <none>");
    } else {
        println!("rule_id: {}", response.rule_id);
    }
    println!("request_id: {}", request_id);
    println!("correlation_id: {}", correlation_id);

    Ok(())
}

fn generate_id(prefix: &str) -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let sequence = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}-{}-{}", prefix, ts, sequence)
}
