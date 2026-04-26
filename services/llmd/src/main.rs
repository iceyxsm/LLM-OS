use common_types::ModuleDescriptor;
use tracing::info;

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
    tokio::signal::ctrl_c().await?;
    info!(target: "llmd", "shutdown");
    Ok(())
}
