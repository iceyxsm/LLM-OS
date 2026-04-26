use llmos_sandbox::{
    AppArmorProfile, AppArmorTemplate, CapabilityPreset, CapabilitySet, NamespaceConfig,
    NamespacePreset, SeccompProfile, SeccompTemplate,
};

use crate::Capability;

/// Sandbox configuration resolved for a specific plugin based on its capabilities.
#[derive(Debug, Clone)]
pub struct PluginSandboxConfig {
    pub seccomp: SeccompProfile,
    pub apparmor: AppArmorProfile,
    pub namespaces: NamespaceConfig,
    pub capabilities: CapabilitySet,
}

/// Resolve sandbox configuration for a plugin based on its declared capabilities.
///
/// Plugins requesting network egress get the more permissive model-runtime
/// profiles. All others get the restrictive MCP plugin defaults.
pub fn resolve_sandbox(
    plugin_id: &str,
    plugin_capabilities: &[Capability],
    plugin_dir: &str,
) -> PluginSandboxConfig {
    let needs_network = plugin_capabilities
        .iter()
        .any(|c| matches!(c, Capability::NetworkEgress));

    if needs_network {
        PluginSandboxConfig {
            seccomp: SeccompTemplate::model_runtime(),
            apparmor: AppArmorTemplate::model_runtime(
                plugin_dir,
                &format!("/var/log/llmos/{plugin_id}"),
            ),
            namespaces: NamespacePreset::network_only(),
            capabilities: CapabilityPreset::model_runtime(),
        }
    } else {
        PluginSandboxConfig {
            seccomp: SeccompTemplate::mcp_plugin(),
            apparmor: AppArmorTemplate::mcp_plugin(plugin_dir),
            namespaces: NamespacePreset::full(),
            capabilities: CapabilityPreset::minimal(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_without_network_gets_restrictive_profile() {
        let config = resolve_sandbox(
            "test.plugin",
            &[Capability::McpSpawn, Capability::AuditEmit],
            "/opt/plugins",
        );
        assert_eq!(config.namespaces, NamespacePreset::full());
        assert_eq!(config.capabilities, CapabilityPreset::minimal());
    }

    #[test]
    fn plugin_with_network_gets_permissive_profile() {
        let config = resolve_sandbox(
            "test.plugin",
            &[Capability::NetworkEgress, Capability::McpSpawn],
            "/opt/plugins",
        );
        assert_eq!(config.namespaces, NamespacePreset::network_only());
        assert_eq!(config.capabilities, CapabilityPreset::model_runtime());
    }
}
