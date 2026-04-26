use std::collections::HashSet;
use std::sync::Arc;

use llmos_secrets::{EnvSecretProvider, ScopedSecretStore, SecretStore};
use tokio::sync::RwLock;
use tracing::info;

/// Build a scoped secret store for the llmd module.
///
/// Registers known provider credential keys and returns a store scoped
/// to the "runtime/model-runtime" module identity.
pub fn build_llmd_secret_store() -> ScopedSecretStore {
    let provider = Arc::new(EnvSecretProvider::new());
    let mut store = SecretStore::new(provider);

    let llmd_module = "runtime/model-runtime".to_string();

    // Register known provider credential keys.
    store.register("openai_api_key", HashSet::from([llmd_module.clone()]));
    store.register("anthropic_api_key", HashSet::from([llmd_module.clone()]));
    store.register("local_model_path", HashSet::from([llmd_module.clone()]));

    let shared = Arc::new(RwLock::new(store));
    info!(target: "llmd::secrets", "secret store initialized with provider credential keys");
    ScopedSecretStore::new(llmd_module, shared)
}
