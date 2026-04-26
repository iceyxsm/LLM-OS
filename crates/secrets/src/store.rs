use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::provider::{SecretError, SecretProvider};

/// Metadata about a secret entry, including which modules may access it.
#[derive(Debug, Clone)]
pub struct SecretEntry {
    pub key: String,
    pub allowed_modules: HashSet<String>,
}

/// A store that maps secret keys to access-control metadata and delegates
/// retrieval to a `SecretProvider`.
pub struct SecretStore {
    provider: Arc<dyn SecretProvider>,
    entries: HashMap<String, SecretEntry>,
}

impl SecretStore {
    pub fn new(provider: Arc<dyn SecretProvider>) -> Self {
        Self {
            provider,
            entries: HashMap::new(),
        }
    }

    /// Register a secret key with the set of modules allowed to read it.
    pub fn register(&mut self, key: impl Into<String>, allowed_modules: HashSet<String>) {
        let key = key.into();
        self.entries.insert(
            key.clone(),
            SecretEntry {
                key,
                allowed_modules,
            },
        );
    }

    /// Retrieve a secret value, enforcing per-module scoping.
    ///
    /// Returns `SecretError::AccessDenied` if the requesting module is not
    /// in the entry's allowed set. Returns `SecretError::NotFound` if the
    /// key has no registered entry.
    pub async fn get(&self, module: &str, key: &str) -> Result<String, SecretError> {
        let entry = self
            .entries
            .get(key)
            .ok_or_else(|| SecretError::NotFound(key.to_string()))?;

        if !entry.allowed_modules.contains(module) {
            return Err(SecretError::AccessDenied {
                module: module.to_string(),
                key: key.to_string(),
            });
        }

        self.provider.get(key).await
    }

    /// List all registered secret keys (without values).
    pub fn keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.entries.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        keys
    }
}

/// A convenience wrapper that binds a `SecretStore` to a specific module,
/// so callers do not need to pass the module id on every request.
pub struct ScopedSecretStore {
    module: String,
    store: Arc<tokio::sync::RwLock<SecretStore>>,
}

impl ScopedSecretStore {
    pub fn new(module: impl Into<String>, store: Arc<tokio::sync::RwLock<SecretStore>>) -> Self {
        Self {
            module: module.into(),
            store,
        }
    }

    pub async fn get(&self, key: &str) -> Result<String, SecretError> {
        let store = self.store.read().await;
        store.get(&self.module, key).await
    }
}
