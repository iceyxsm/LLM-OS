use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| {
            anyhow!(
                "failed to resolve workspace root from {}",
                manifest_dir.display()
            )
        })
}

fn sample_manifest_dir() -> Result<PathBuf> {
    Ok(workspace_root()?
        .join("sdk")
        .join("plugin-api")
        .join("manifests"))
}

fn mcp_runtime_bin() -> String {
    env!("CARGO_BIN_EXE_mcp-runtime").to_string()
}

#[tokio::test]
async fn list_reads_sample_manifest() -> Result<()> {
    let manifest_dir = sample_manifest_dir()?;
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        Command::new(mcp_runtime_bin())
            .arg("list")
            .arg("--manifest-dir")
            .arg(&manifest_dir)
            .output(),
    )
    .await
    .context("timed out waiting for mcp-runtime list output")??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "mcp-runtime list failed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("mock.mcp.echo@0.1.0"),
        "expected sample plugin to appear in list output\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    Ok(())
}

#[tokio::test]
async fn run_autostart_supports_stop_start_list_cycle() -> Result<()> {
    let manifest_dir = sample_manifest_dir()?;
    let mut child = Command::new(mcp_runtime_bin())
        .arg("run")
        .arg("--manifest-dir")
        .arg(&manifest_dir)
        .arg("--autostart")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn mcp-runtime run process")?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("failed to open child stdin"))?;
    stdin
        .write_all(b"list\nstop mock.mcp.echo\nlist\nstart mock.mcp.echo\nlist\nquit\n")
        .await
        .context("failed to write admin commands")?;
    stdin
        .flush()
        .await
        .context("failed to flush admin commands")?;
    drop(stdin);

    let output = tokio::time::timeout(Duration::from_secs(90), child.wait_with_output())
        .await
        .context("timed out waiting for mcp-runtime run output")??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "mcp-runtime run failed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("mock.mcp.echo@0.1.0 running=true"),
        "expected running=true in admin list output\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("mock.mcp.echo@0.1.0 running=false"),
        "expected running=false after stop command\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    Ok(())
}
