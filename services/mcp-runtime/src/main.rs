use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .compact()
        .init();

    info!(target: "mcp-runtime", "mcp runtime started");
    info!(target: "mcp-runtime", "expecting plugin manifests from sdk/plugin-api");

    tokio::signal::ctrl_c().await?;
    info!(target: "mcp-runtime", "shutdown");
    Ok(())
}
