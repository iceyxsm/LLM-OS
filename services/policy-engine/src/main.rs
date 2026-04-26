use std::{path::PathBuf, sync::Arc};

use controlplane_api::{
    health_service_server::HealthServiceServer, policy_service_server::PolicyServiceServer,
};
use policy_engine::{
    grpc::{HealthGrpcService, PolicyGrpcService},
    loader::load_policy_document,
    metrics::{init_policy_metrics, run_metrics_server, PolicyEngineMetrics},
    reload::{poll_and_reload, SharedPolicy},
};
use tonic::transport::Server;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .compact()
        .init();

    let policy_path = std::env::var("LLMOS_POLICY_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/policy.example.yaml"));
    let policy = load_policy_document(&policy_path)?;
    let listen_addr: std::net::SocketAddr = std::env::var("LLMOS_POLICY_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1:50051".to_string())
        .parse()?;
    let metrics_listen_addr: std::net::SocketAddr = std::env::var("LLMOS_POLICY_METRICS_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1:9091".to_string())
        .parse()?;
    let shared = SharedPolicy::new(policy.clone());
    let service = PolicyGrpcService::new_shared(shared.clone());
    let health_service = HealthGrpcService;
    let metrics = Arc::new(PolicyEngineMetrics::default());
    metrics.set_rules_loaded(policy.rules.len());
    init_policy_metrics(metrics.clone());

    let reload_interval_secs: u64 = std::env::var("LLMOS_POLICY_RELOAD_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    info!(
        target: "policy-engine",
        path = %policy_path.display(),
        version = %policy.version,
        rules = policy.rules.len(),
        listen = %listen_addr,
        metrics_listen = %metrics_listen_addr,
        reload_interval_secs = reload_interval_secs,
        "policy engine online"
    );
    info!(
        target: "policy-engine",
        "decision mode: deny overrides allow, default deny"
    );
    tokio::spawn(async move {
        if let Err(err) = run_metrics_server(metrics_listen_addr, metrics).await {
            warn!(target: "policy-engine::metrics", error = %err, "metrics server stopped");
        }
    });

    let reload_path = policy_path.clone();
    let reload_shared = shared.clone();
    tokio::spawn(async move {
        poll_and_reload(
            reload_path,
            reload_shared,
            std::time::Duration::from_secs(reload_interval_secs),
        )
        .await;
    });

    Server::builder()
        .add_service(PolicyServiceServer::new(service))
        .add_service(HealthServiceServer::new(health_service))
        .serve_with_shutdown(listen_addr, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    info!(target: "policy-engine", "shutdown complete");
    Ok(())
}
