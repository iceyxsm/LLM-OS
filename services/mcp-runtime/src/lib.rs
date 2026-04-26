use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use tokio::process::{Child, Command};
use tokio::time::{timeout, Duration};
use tracing::{info, warn};

const STOP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
pub enum Capability {
    #[serde(rename = "model:invoke")]
    ModelInvoke,
    #[serde(rename = "mcp:spawn")]
    McpSpawn,
    #[serde(rename = "network:egress")]
    NetworkEgress,
    #[serde(rename = "fs:read")]
    FsRead,
    #[serde(rename = "fs:write")]
    FsWrite,
    #[serde(rename = "audit:emit")]
    AuditEmit,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub id: String,
    pub version: String,
    pub entrypoint: String,
    pub description: Option<String>,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone)]
pub struct PluginRuntimeStatus {
    pub id: String,
    pub version: String,
    pub entrypoint: String,
    pub running: bool,
    pub pid: Option<u32>,
}

struct ManagedProcess {
    child: Child,
}

pub struct RuntimeManager {
    manifests: HashMap<String, PluginManifest>,
    running: HashMap<String, ManagedProcess>,
}

impl RuntimeManager {
    pub fn new(manifests: HashMap<String, PluginManifest>) -> Self {
        Self {
            manifests,
            running: HashMap::new(),
        }
    }

    pub fn manifests_len(&self) -> usize {
        self.manifests.len()
    }

    pub async fn start_all(&mut self) -> Result<()> {
        let mut ids: Vec<String> = self.manifests.keys().cloned().collect();
        ids.sort_unstable();
        for id in ids {
            self.start(&id).await?;
        }
        Ok(())
    }

    pub async fn start(&mut self, id: &str) -> Result<()> {
        self.reconcile_exited();
        if self.running.contains_key(id) {
            return Ok(());
        }

        let manifest = self
            .manifests
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow!("plugin '{}' was not found in loaded manifests", id))?;
        let child = spawn_process(&manifest)?;
        let pid = child.id();
        self.running
            .insert(id.to_string(), ManagedProcess { child });
        info!(
            target: "mcp-runtime",
            plugin_id = %id,
            pid = ?pid,
            "plugin started"
        );
        Ok(())
    }

    pub async fn stop(&mut self, id: &str) -> Result<()> {
        self.reconcile_exited();
        match self.running.remove(id) {
            Some(mut proc) => {
                stop_process(&mut proc.child).await?;
                info!(target: "mcp-runtime", plugin_id = %id, "plugin stopped");
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub async fn restart(&mut self, id: &str) -> Result<()> {
        self.stop(id).await?;
        self.start(id).await
    }

    pub async fn stop_all(&mut self) -> Result<()> {
        let mut ids: Vec<String> = self.running.keys().cloned().collect();
        ids.sort_unstable();
        for id in ids {
            self.stop(&id).await?;
        }
        Ok(())
    }

    pub fn list(&mut self) -> Vec<PluginRuntimeStatus> {
        self.reconcile_exited();
        let mut ids: Vec<String> = self.manifests.keys().cloned().collect();
        ids.sort_unstable();
        ids.into_iter()
            .filter_map(|id| {
                self.manifests.get(&id).map(|manifest| {
                    let pid = self.running.get(&id).and_then(|proc| proc.child.id());
                    PluginRuntimeStatus {
                        id,
                        version: manifest.version.clone(),
                        entrypoint: manifest.entrypoint.clone(),
                        running: pid.is_some(),
                        pid,
                    }
                })
            })
            .collect()
    }

    fn reconcile_exited(&mut self) {
        let mut exited = Vec::new();
        for (id, proc) in &mut self.running {
            match proc.child.try_wait() {
                Ok(Some(status)) => {
                    info!(
                        target: "mcp-runtime",
                        plugin_id = %id,
                        exit_status = %status,
                        "plugin exited"
                    );
                    exited.push(id.clone());
                }
                Ok(None) => {}
                Err(err) => {
                    warn!(
                        target: "mcp-runtime",
                        plugin_id = %id,
                        error = %err,
                        "failed to probe plugin process status"
                    );
                }
            }
        }
        for id in exited {
            self.running.remove(&id);
        }
    }
}

pub fn default_manifest_dir() -> PathBuf {
    std::env::var("LLMOS_MCP_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("sdk/plugin-api/manifests"))
}

pub fn load_manifests(dir: &Path) -> Result<HashMap<String, PluginManifest>> {
    if !dir.exists() {
        return Ok(HashMap::new());
    }

    let mut manifests = HashMap::new();
    for entry in
        std::fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let manifest = load_manifest_file(&path)?;
        if manifests.insert(manifest.id.clone(), manifest).is_some() {
            bail!(
                "duplicate plugin id in manifests directory: {}",
                path.display()
            );
        }
    }

    Ok(manifests)
}

pub fn load_manifest_file(path: &Path) -> Result<PluginManifest> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read manifest file {}", path.display()))?;
    parse_manifest_json(&raw, path)
}

fn parse_manifest_json(raw: &str, source: &Path) -> Result<PluginManifest> {
    let manifest: PluginManifest = serde_json::from_str(raw)
        .with_context(|| format!("failed to parse JSON manifest at {}", source.display()))?;

    validate_plugin_id(&manifest.id).with_context(|| {
        format!(
            "manifest {} has invalid plugin id '{}'",
            source.display(),
            manifest.id
        )
    })?;
    if manifest.capabilities.is_empty() {
        bail!(
            "manifest {} has empty capabilities; at least one capability is required",
            source.display()
        );
    }
    if manifest.entrypoint.trim().is_empty() {
        bail!(
            "manifest {} has an empty entrypoint; a command is required",
            source.display()
        );
    }

    let mut unique = HashSet::new();
    for cap in &manifest.capabilities {
        if !unique.insert(cap) {
            bail!(
                "manifest {} contains duplicate capability entries",
                source.display()
            );
        }
    }

    Ok(manifest)
}

fn validate_plugin_id(id: &str) -> Result<()> {
    if id.is_empty() {
        bail!("plugin id cannot be empty");
    }

    if id.chars().all(|ch| {
        ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '.' || ch == '_' || ch == '-'
    }) {
        Ok(())
    } else {
        bail!("plugin id must match ^[a-z0-9._-]+$")
    }
}

fn spawn_process(manifest: &PluginManifest) -> Result<Child> {
    let mut cmd = shell_command(&manifest.entrypoint);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    cmd.spawn().with_context(|| {
        format!(
            "failed to spawn plugin '{}' with entrypoint '{}'",
            manifest.id, manifest.entrypoint
        )
    })
}

fn shell_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    }
    #[cfg(not(windows))]
    {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    }
}

async fn stop_process(child: &mut Child) -> Result<()> {
    if let Some(pid) = child.id() {
        info!(target: "mcp-runtime", pid = pid, "stopping plugin process");
    }

    child
        .start_kill()
        .context("failed to signal plugin process")?;
    match timeout(STOP_TIMEOUT, child.wait()).await {
        Ok(wait_result) => {
            let status = wait_result.context("failed while waiting for process to exit")?;
            info!(
                target: "mcp-runtime",
                exit_status = %status,
                "plugin process terminated"
            );
            Ok(())
        }
        Err(_) => bail!("timed out waiting for plugin process to exit"),
    }
}

#[cfg(test)]
mod tests {
    use super::{load_manifests, parse_manifest_json};
    use std::path::Path;

    #[test]
    fn parse_manifest_accepts_valid_payload() {
        let raw = r#"{
          "id":"example.plugin",
          "version":"1.0.0",
          "entrypoint":"node server.js",
          "capabilities":["mcp:spawn","fs:read"]
        }"#;
        let manifest =
            parse_manifest_json(raw, Path::new("manifest.json")).expect("valid manifest");
        assert_eq!(manifest.id, "example.plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.entrypoint, "node server.js");
    }

    #[test]
    fn parse_manifest_rejects_invalid_id() {
        let raw = r#"{
          "id":"Example.Plugin",
          "version":"1.0.0",
          "entrypoint":"node server.js",
          "capabilities":["mcp:spawn"]
        }"#;
        let err =
            parse_manifest_json(raw, Path::new("manifest.json")).expect_err("manifest should fail");
        assert!(err.to_string().contains("invalid plugin id"));
    }

    #[test]
    fn load_manifests_rejects_duplicate_ids() {
        let root = std::env::temp_dir().join(format!(
            "llm_os_mcp_runtime_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("temp dir");

        let one = root.join("one.json");
        let two = root.join("two.json");
        let json = r#"{
          "id":"dup.plugin",
          "version":"1.0.0",
          "entrypoint":"echo test",
          "capabilities":["fs:read"]
        }"#;
        std::fs::write(&one, json).expect("write one");
        std::fs::write(&two, json).expect("write two");

        let err = load_manifests(&root).expect_err("duplicate ids should fail");
        assert!(err.to_string().contains("duplicate plugin id"));

        let _ = std::fs::remove_dir_all(root);
    }
}
