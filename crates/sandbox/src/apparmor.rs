use serde::{Deserialize, Serialize};

/// An AppArmor profile definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppArmorProfile {
    pub name: String,
    pub read_paths: Vec<String>,
    pub write_paths: Vec<String>,
    pub network_access: bool,
}

/// Pre-built AppArmor templates for common workload types.
pub struct AppArmorTemplate;

impl AppArmorTemplate {
    /// A restrictive profile for MCP plugins.
    ///
    /// Read access to the plugin directory only. No write access.
    /// No network access.
    pub fn mcp_plugin(plugin_dir: &str) -> AppArmorProfile {
        AppArmorProfile {
            name: "llmos-mcp-plugin".to_string(),
            read_paths: vec![
                format!("{plugin_dir}/**"),
                "/usr/lib/**".to_string(),
                "/lib/**".to_string(),
            ],
            write_paths: vec![],
            network_access: false,
        }
    }

    /// A profile for the model runtime.
    ///
    /// Read access to config and model directories. Write access to
    /// the audit log directory. Network access enabled for API calls.
    pub fn model_runtime(config_dir: &str, audit_dir: &str) -> AppArmorProfile {
        AppArmorProfile {
            name: "llmos-model-runtime".to_string(),
            read_paths: vec![
                format!("{config_dir}/**"),
                "/usr/lib/**".to_string(),
                "/lib/**".to_string(),
            ],
            write_paths: vec![format!("{audit_dir}/**")],
            network_access: true,
        }
    }
}
