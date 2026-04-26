use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .compact()
        .init();

    info!(target: "policy-engine", "policy engine online");
    info!(target: "policy-engine", "default mode: deny unless explicit allow");

    tokio::signal::ctrl_c().await?;
    info!(target: "policy-engine", "shutdown");
    Ok(())
}
