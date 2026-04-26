use serde::{Deserialize, Serialize};

/// Action to take when a syscall matches a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeccompAction {
    Allow,
    Kill,
    Log,
    Errno,
}

/// A single syscall filtering rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallRule {
    pub names: Vec<String>,
    pub action: SeccompAction,
}

/// A complete seccomp profile with a default action and a list of rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccompProfile {
    pub default_action: SeccompAction,
    pub rules: Vec<SyscallRule>,
}

/// Pre-built seccomp templates for common workload types.
pub struct SeccompTemplate;

impl SeccompTemplate {
    /// A restrictive profile for MCP plugins.
    ///
    /// Allows basic I/O, memory, and process syscalls. Denies network,
    /// mount, and module loading by default.
    pub fn mcp_plugin() -> SeccompProfile {
        SeccompProfile {
            default_action: SeccompAction::Errno,
            rules: vec![
                SyscallRule {
                    names: vec![
                        "read".to_string(),
                        "write".to_string(),
                        "close".to_string(),
                        "fstat".to_string(),
                        "lseek".to_string(),
                        "mmap".to_string(),
                        "mprotect".to_string(),
                        "munmap".to_string(),
                        "brk".to_string(),
                        "exit_group".to_string(),
                        "exit".to_string(),
                        "futex".to_string(),
                        "clock_gettime".to_string(),
                        "getpid".to_string(),
                        "gettid".to_string(),
                    ],
                    action: SeccompAction::Allow,
                },
                SyscallRule {
                    names: vec![
                        "openat".to_string(),
                        "newfstatat".to_string(),
                        "getdents64".to_string(),
                    ],
                    action: SeccompAction::Allow,
                },
            ],
        }
    }

    /// A permissive profile for the model runtime.
    ///
    /// Allows network syscalls in addition to the base set, since the
    /// model runtime needs to reach external APIs.
    pub fn model_runtime() -> SeccompProfile {
        let mut profile = Self::mcp_plugin();
        profile.rules.push(SyscallRule {
            names: vec![
                "socket".to_string(),
                "connect".to_string(),
                "sendto".to_string(),
                "recvfrom".to_string(),
                "bind".to_string(),
                "listen".to_string(),
                "accept".to_string(),
                "setsockopt".to_string(),
                "getsockopt".to_string(),
                "epoll_create1".to_string(),
                "epoll_ctl".to_string(),
                "epoll_wait".to_string(),
            ],
            action: SeccompAction::Allow,
        });
        profile
    }
}
