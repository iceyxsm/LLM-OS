use std::path::PathBuf;

use common_types::{ActionRequest, ModuleDescriptor};
use llmd::{process_action, NoopExecutor, StdoutAuditSink};
use policy_engine::loader::load_policy_document;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .compact()
        .init();

    let module = ModuleDescriptor {
        id: "runtime/model-runtime".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        status: "starting".to_string(),
    };

    info!(target: "llmd", module = ?module, "llmd bootstrap complete");

    let policy_path = std::env::var("LLMOS_POLICY_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/policy.example.yaml"));
    let policy = load_policy_document(&policy_path)?;

    let executor = NoopExecutor;
    let audit_sink = StdoutAuditSink;
    let startup_requests = [
        ActionRequest {
            version: "v1".to_string(),
            subject: "runtime/model-runtime".to_string(),
            action: "network:connect".to_string(),
            resource: "api.openai.com".to_string(),
        },
        ActionRequest {
            version: "v1".to_string(),
            subject: "runtime/mcp-runtime".to_string(),
            action: "fs:write".to_string(),
            resource: "/".to_string(),
        },
    ];

    for request in startup_requests {
        match process_action(&policy, request, &executor, &audit_sink) {
            Ok(result) => info!(target: "llmd", result = ?result, "action executed"),
            Err(err) => warn!(target: "llmd", error = %err, "action denied or failed"),
        }
    }

    tokio::signal::ctrl_c().await?;
    info!(target: "llmd", "shutdown");
    Ok(())
}
