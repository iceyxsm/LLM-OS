use std::sync::Arc;

use std::sync::atomic::{AtomicU64, Ordering};

use common_types::{ActionRequest, ModuleDescriptor};
use controlplane_api::{health_service_client::HealthServiceClient, HealthCheckRequest};
use llmd::{
    init_runtime_metrics, process_action, run_metrics_server, AuditSink, GrpcPolicyClientConfig,
    GrpcPolicyDecisionClient, JsonlFileAuditSink, NoopExecutor, RuntimeMetrics, StdoutAuditSink,
};
use tonic::metadata::MetadataValue;
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
    let timeout_per_attempt = std::time::Duration::from_secs(
        std::env::var("LLMOS_POLICY_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(2),
    );
    let max_attempts = std::env::var("LLMOS_POLICY_MAX_ATTEMPTS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);
    let initial_backoff_ms = std::env::var("LLMOS_POLICY_BACKOFF_INITIAL_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(100);
    let max_backoff_ms = std::env::var("LLMOS_POLICY_BACKOFF_MAX_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_000);
    let breaker_threshold = std::env::var("LLMOS_POLICY_BREAKER_THRESHOLD")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);
    let breaker_cooldown_secs = std::env::var("LLMOS_POLICY_BREAKER_COOLDOWN_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(5);
    let health_interval_secs = std::env::var("LLMOS_POLICY_HEALTH_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(30);
    let audit_rotate_max_bytes = std::env::var("LLMOS_AUDIT_ROTATE_MAX_BYTES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10 * 1024 * 1024);
    let audit_rotate_max_files = std::env::var("LLMOS_AUDIT_ROTATE_MAX_FILES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(5);
    let metrics_listen: std::net::SocketAddr = std::env::var("LLMOS_METRICS_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1:9090".to_string())
        .parse()?;

    let metrics = Arc::new(RuntimeMetrics::default());
    init_runtime_metrics(metrics.clone());
    tokio::spawn(async move {
        if let Err(err) = run_metrics_server(metrics_listen, metrics).await {
            warn!(target: "llmd::metrics", error = %err, "metrics server stopped");
        }
    });

    let correlation_id = generate_id("corr");
    let audit_sink: Box<dyn AuditSink> = if let Ok(path) = std::env::var("LLMOS_AUDIT_JSONL_PATH") {
        info!(
            target: "llmd",
            audit_jsonl_path = %path,
            audit_rotate_max_bytes = audit_rotate_max_bytes,
            audit_rotate_max_files = audit_rotate_max_files,
            "using JSONL audit sink"
        );
        Box::new(JsonlFileAuditSink::new_with_rotation(
            path,
            audit_rotate_max_bytes,
            audit_rotate_max_files,
        )?)
    } else {
        Box::new(StdoutAuditSink)
    };

    info!(
        target: "llmd",
        module = ?module,
        policy_endpoint = %endpoint,
        policy_timeout_secs = timeout_per_attempt.as_secs(),
        policy_max_attempts = max_attempts,
        policy_backoff_initial_ms = initial_backoff_ms,
        policy_backoff_max_ms = max_backoff_ms,
        policy_breaker_threshold = breaker_threshold,
        policy_breaker_cooldown_secs = breaker_cooldown_secs,
        metrics_listen = %metrics_listen,
        correlation_id = %correlation_id,
        "llmd bootstrap complete"
    );

    let policy_config = GrpcPolicyClientConfig {
        timeout_per_attempt,
        max_attempts,
        initial_backoff: std::time::Duration::from_millis(initial_backoff_ms),
        max_backoff: std::time::Duration::from_millis(max_backoff_ms),
        circuit_breaker_threshold: breaker_threshold,
        circuit_breaker_cooldown: std::time::Duration::from_secs(breaker_cooldown_secs),
    };
    let mut policy_client =
        GrpcPolicyDecisionClient::connect_with_config(endpoint.clone(), policy_config).await?;

    match check_policy_health(&endpoint, timeout_per_attempt, &correlation_id).await {
        Ok((status, detail)) => info!(
            target: "llmd",
            health_status = %status,
            health_detail = %detail,
            "policy service preflight check passed"
        ),
        Err(err) => {
            warn!(
                target: "llmd",
                error = %err,
                "policy service preflight check failed; continuing in fail-closed mode"
            );
        }
    }

    let health_endpoint = endpoint.clone();
    let health_correlation = correlation_id.clone();
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(health_interval_secs));
        loop {
            interval.tick().await;
            match check_policy_health(&health_endpoint, timeout_per_attempt, &health_correlation)
                .await
            {
                Ok((status, detail)) => info!(
                    target: "llmd::health",
                    health_status = %status,
                    health_detail = %detail,
                    "policy service health check passed"
                ),
                Err(err) => warn!(
                    target: "llmd::health",
                    error = %err,
                    "policy service health check failed"
                ),
            }
        }
    });
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

async fn check_policy_health(
    endpoint: &str,
    timeout: std::time::Duration,
    correlation_id: &str,
) -> anyhow::Result<(String, String)> {
    let mut client = HealthServiceClient::connect(endpoint.to_string()).await?;
    let mut request = tonic::Request::new(HealthCheckRequest {
        service: "policy-engine".to_string(),
    });
    request.metadata_mut().insert(
        "x-request-id",
        MetadataValue::try_from(generate_id("health-req").as_str())?,
    );
    request
        .metadata_mut()
        .insert("x-correlation-id", MetadataValue::try_from(correlation_id)?);

    let response = tokio::time::timeout(timeout, client.check(request))
        .await??
        .into_inner();
    Ok((response.status, response.detail))
}
