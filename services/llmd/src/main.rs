use std::sync::atomic::{AtomicU64, Ordering};

use common_types::{ActionRequest, ModuleDescriptor};
use llmd::{
    process_action, AuditSink, GrpcPolicyDecisionClient, JsonlFileAuditSink, NoopExecutor,
    StdoutAuditSink,
};
use tracing::{info, warn};

static REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);

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

    let correlation_id = generate_id("corr");
    let audit_sink: Box<dyn AuditSink> = if let Ok(path) = std::env::var("LLMOS_AUDIT_JSONL_PATH") {
        info!(target: "llmd", audit_jsonl_path = %path, "using JSONL audit sink");
        Box::new(JsonlFileAuditSink::new(path)?)
    } else {
        Box::new(StdoutAuditSink)
    };

    info!(
        target: "llmd",
        module = ?module,
        policy_endpoint = %endpoint,
        policy_timeout_secs = timeout.as_secs(),
        correlation_id = %correlation_id,
        "llmd bootstrap complete"
    );

    let mut policy_client = GrpcPolicyDecisionClient::connect(endpoint, timeout).await?;
    let executor = NoopExecutor;
    let startup_requests = [
        build_request(
            "runtime/model-runtime",
            "network:connect",
            "api.openai.com",
            &correlation_id,
        ),
        build_request("runtime/mcp-runtime", "fs:write", "/", &correlation_id),
    ];

    for request in startup_requests {
        match process_action(&mut policy_client, request, &executor, audit_sink.as_ref()).await {
            Ok(result) => info!(target: "llmd", result = ?result, "action executed"),
            Err(err) => warn!(target: "llmd", error = %err, "action denied or failed"),
        }
    }

    tokio::signal::ctrl_c().await?;
    info!(target: "llmd", "shutdown");
    Ok(())
}

fn build_request(
    subject: &str,
    action: &str,
    resource: &str,
    correlation_id: &str,
) -> ActionRequest {
    ActionRequest {
        version: "v1".to_string(),
        request_id: generate_id("req"),
        correlation_id: correlation_id.to_string(),
        subject: subject.to_string(),
        action: action.to_string(),
        resource: resource.to_string(),
    }
}

fn generate_id(prefix: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let sequence = REQUEST_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{}", prefix, ts, sequence)
}
