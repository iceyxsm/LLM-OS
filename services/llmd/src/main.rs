use common_types::{ActionRequest, ModuleDescriptor};
use llmd::{process_action, GrpcPolicyDecisionClient, NoopExecutor, StdoutAuditSink};
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

    let endpoint = std::env::var("LLMOS_POLICY_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:50051".to_string());
    let timeout = std::time::Duration::from_secs(
        std::env::var("LLMOS_POLICY_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(2),
    );

    info!(
        target: "llmd",
        module = ?module,
        policy_endpoint = %endpoint,
        policy_timeout_secs = timeout.as_secs(),
        "llmd bootstrap complete"
    );

    let mut policy_client = GrpcPolicyDecisionClient::connect(endpoint, timeout).await?;
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
        match process_action(&mut policy_client, request, &executor, &audit_sink).await {
            Ok(result) => info!(target: "llmd", result = ?result, "action executed"),
            Err(err) => warn!(target: "llmd", error = %err, "action denied or failed"),
        }
    }

    tokio::signal::ctrl_c().await?;
    info!(target: "llmd", "shutdown");
    Ok(())
}
