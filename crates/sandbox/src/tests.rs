use crate::apparmor::AppArmorTemplate;
use crate::capability::CapabilityPreset;
use crate::namespace::NamespacePreset;
use crate::seccomp::{SeccompAction, SeccompTemplate};

#[test]
fn mcp_plugin_seccomp_denies_by_default() {
    let profile = SeccompTemplate::mcp_plugin();
    assert_eq!(profile.default_action, SeccompAction::Errno);
}

#[test]
fn mcp_plugin_seccomp_allows_basic_io() {
    let profile = SeccompTemplate::mcp_plugin();
    let allowed: Vec<&str> = profile
        .rules
        .iter()
        .filter(|r| r.action == SeccompAction::Allow)
        .flat_map(|r| r.names.iter().map(|n| n.as_str()))
        .collect();
    assert!(allowed.contains(&"read"));
    assert!(allowed.contains(&"write"));
    assert!(allowed.contains(&"close"));
}

#[test]
fn model_runtime_seccomp_allows_network() {
    let profile = SeccompTemplate::model_runtime();
    let allowed: Vec<&str> = profile
        .rules
        .iter()
        .filter(|r| r.action == SeccompAction::Allow)
        .flat_map(|r| r.names.iter().map(|n| n.as_str()))
        .collect();
    assert!(allowed.contains(&"socket"));
    assert!(allowed.contains(&"connect"));
}

#[test]
fn mcp_plugin_apparmor_denies_network() {
    let profile = AppArmorTemplate::mcp_plugin("/opt/plugins");
    assert!(!profile.network_access);
    assert!(profile.write_paths.is_empty());
}

#[test]
fn model_runtime_apparmor_allows_network() {
    let profile = AppArmorTemplate::model_runtime("/etc/llmos", "/var/log/llmos");
    assert!(profile.network_access);
    assert!(!profile.write_paths.is_empty());
}

#[test]
fn full_namespace_isolates_everything() {
    let ns = NamespacePreset::full();
    assert!(ns.pid);
    assert!(ns.net);
    assert!(ns.mount);
    assert!(ns.ipc);
    assert!(ns.user);
}

#[test]
fn none_namespace_shares_everything() {
    let ns = NamespacePreset::none();
    assert!(!ns.pid);
    assert!(!ns.net);
    assert!(!ns.mount);
    assert!(!ns.ipc);
    assert!(!ns.user);
}

#[test]
fn minimal_capabilities_deny_dangerous_caps() {
    let caps = CapabilityPreset::minimal();
    assert!(caps.allowed.is_empty());
    assert!(caps.denied.contains(&"CAP_SYS_ADMIN".to_string()));
    assert!(caps.denied.contains(&"CAP_NET_RAW".to_string()));
}

#[test]
fn model_runtime_capabilities_allow_net_bind() {
    let caps = CapabilityPreset::model_runtime();
    assert!(caps.allowed.contains(&"CAP_NET_BIND_SERVICE".to_string()));
}
