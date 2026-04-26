use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use common_types::{ActionRequest, ModuleDescriptor};
use controlplane_api::{health_service_client::HealthServiceClient, HealthCheckRequest};
use llmd::{
    bus::BusAuditSink,
    config::{load_config, LlmdConfig},
    executor::ModelExecutor,
    init_runtime_metrics, process_action, run_metrics_server,
    secrets::build_llmd_secret_store,
    AuditSink, GrpcPolicyClientConfig, GrpcPolicyDecisionClient, JsonlFileAuditSink,
    RuntimeMetrics, StdoutAuditSink,
};
use llmos_model_runtime::{InferenceBackend, MockBackend};
use llmos_service_bus::LocalChannel;
use tokio::sync::RwLock;
use tonic::metadata::MetadataValue;
use tracing::{info, warn};

static REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .compact()
        .init();

    let config_path = std::env::var("LLMOS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/llmos.toml"));

    let config = load_config(&config_path)?;

    info!(
        target: "llmd",
        config_path = %config_path.display(),
        "configuration loaded"
    );

    let module = ModuleDescriptor {
        id: "runtime/model-runtime".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        status: "starting".to_string(),
    };

    // Metrics
    let metrics_listen: std::net::SocketAddr = config.metrics.listen.parse()?;
    let metrics = Arc::new(RuntimeMetrics::default());
    init_runtime_metrics(metrics.clone());
    tokio::spawn(async move {
        if let Err(err) = run_metrics_server(metrics_listen, metrics).await {
            warn!(target: "llmd::metrics", error = %err, "metrics server stopped");
        }
    });

    // Secrets
    let _secret_store = build_llmd_secret_store();
    info!(target: "llmd", "secret store ready");

    // Audit sink (driven by config.audit.sink)
    let correlation_id = generate_id("corr");
    let audit_sink: Box<dyn AuditSink> = build_audit_sink(&config)?;

    // Model backend (driven by config.model.backend)
    let backend = build_model_backend(&config).await?;
    let executor = ModelExecutor::new(backend);

    // Policy client
    let policy_config = GrpcPolicyClientConfig {
        timeout_per_attempt: std::time::Duration::from_secs(config.policy.timeout_secs),
        max_attempts: config.policy.max_attempts,
        initial_backoff: std::time::Duration::from_millis(config.policy.backoff_initial_ms),
        max_backoff: std::time::Duration::from_millis(config.policy.backoff_max_ms),
        circuit_breaker_threshold: config.policy.breaker_threshold,
        circuit_breaker_cooldown: std::time::Duration::from_secs(
            config.policy.breaker_cooldown_secs,
        ),
    };

    info!(
        target: "llmd",
        module = ?module,
        policy_endpoint = %config.policy.endpoint,
        model_backend = %config.model.backend,
        model_name = %config.model.name,
        audit_sink = %config.audit.sink,
        metrics_listen = %config.metrics.listen,
        correlation_id = %correlation_id,
        "llmd bootstrap complete"
    );

    let mut policy_client = GrpcPolicyDecisionClient::connect_with_config(
        config.policy.endpoint.clone(),
        policy_config,
    )
    .await?;

    // Health preflight
    let timeout = std::time::Duration::from_secs(config.policy.timeout_secs);
    match check_policy_health(&config.policy.endpoint, timeout, &correlation_id).await {
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

    // Periodic health checks
    let health_endpoint = config.policy.endpoint.clone();
    let health_correlation = correlation_id.clone();
    let health_interval = config.policy.health_interval_secs;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(health_interval));
        loop {
            interval.tick().await;
            match check_policy_health(&health_endpoint, timeout, &health_correlation).await {
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

    // Startup requests
    let startup_requests = [
        build_request(
            "runtime/model-runtime",
            "network:connect",
            "api.openai.com",
            &correlation_id,
        ),
        build_request("runtime/mcp-runtime", "fs:write", "/", &correlation_id),
        build_request(
            "runtime/model-runtime",
            "model:invoke",
            "Hello, world!",
            &correlation_id,
        ),
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

fn build_audit_sink(config: &LlmdConfig) -> anyhow::Result<Box<dyn AuditSink>> {
    match config.audit.sink.as_str() {
        "jsonl" => {
            if config.audit.jsonl_path.is_empty() {
                anyhow::bail!("audit.sink is 'jsonl' but audit.jsonl_path is empty");
            }
            info!(
                target: "llmd",
                path = %config.audit.jsonl_path,
                rotate_max_bytes = config.audit.rotate_max_bytes,
                rotate_max_files = config.audit.rotate_max_files,
                "using JSONL audit sink"
            );
            Ok(Box::new(JsonlFileAuditSink::new_with_rotation(
                &config.audit.jsonl_path,
                config.audit.rotate_max_bytes,
                config.audit.rotate_max_files,
            )?))
        }
        "bus" => {
            let transport = Arc::new(LocalChannel::new());
            info!(target: "llmd", "using service bus audit sink");
            Ok(Box::new(BusAuditSink::new(transport, "llmd")))
        }
        _ => {
            info!(target: "llmd", "using stdout audit sink");
            Ok(Box::new(StdoutAuditSink))
        }
    }
}

async fn build_model_backend(
    config: &LlmdConfig,
) -> anyhow::Result<Arc<RwLock<dyn InferenceBackend>>> {
    match config.model.backend.as_str() {
        #[cfg(feature = "llama-cpp")]
        "llama-cpp" => {
            use llmos_model_runtime::{LlamaCppBackend, ModelConfig};

            if config.model.model_path.is_empty() {
                anyhow::bail!("model.backend is 'llama-cpp' but model.model_path is empty");
            }

            let model_config = ModelConfig {
                name: config.model.name.clone(),
                model_path: config.model.model_path.clone(),
                architecture: "auto".to_string(),
                quantization: config.model.quantization.clone(),
                context_length: config.model.context_length,
                gpu_layers: config.model.gpu_layers,
                threads: config.model.threads,
            };

            let mut backend = LlamaCppBackend::new();
            info!(
                target: "llmd",
                model_path = %config.model.model_path,
                gpu_layers = config.model.gpu_layers,
                threads = config.model.threads,
                context_length = config.model.context_length,
                "loading llama.cpp model"
            );
            backend
                .load(&model_config)
                .await
                .map_err(|e| anyhow::anyhow!("failed to load model: {e}"))?;
            info!(target: "llmd", name = %config.model.name, "model loaded");

            Ok(Arc::new(RwLock::new(backend)))
        }
        #[cfg(not(feature = "llama-cpp"))]
        "llama-cpp" => {
            anyhow::bail!(
                "model.backend is 'llama-cpp' but llmd was compiled without the 'llama-cpp' feature. \
                 Rebuild with: cargo build -p llmd --features llama-cpp"
            );
        }
        "mock" => {
            info!(target: "llmd", "using mock model backend");
            Ok(Arc::new(RwLock::new(MockBackend::new())))
        }
        other => {
            anyhow::bail!(
                "unknown model.backend '{}'; supported values: mock, llama-cpp",
                other
            );
        }
    }
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
    format!("{prefix}-{ts}-{sequence}")
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
