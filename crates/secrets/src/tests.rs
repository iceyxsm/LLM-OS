use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;

use crate::provider::{SecretError, SecretProvider};
use crate::store::SecretStore;

struct InMemoryProvider {
    secrets: HashMap<String, String>,
}

impl InMemoryProvider {
    fn new(secrets: HashMap<String, String>) -> Self {
        Self { secrets }
    }
}

#[async_trait]
impl SecretProvider for InMemoryProvider {
    async fn get(&self, key: &str) -> Result<String, SecretError> {
        self.secrets
            .get(key)
            .cloned()
            .ok_or_else(|| SecretError::NotFound(key.to_string()))
    }

    async fn exists(&self, key: &str) -> Result<bool, SecretError> {
        Ok(self.secrets.contains_key(key))
    }
}

fn test_provider() -> Arc<InMemoryProvider> {
    let mut secrets = HashMap::new();
    secrets.insert("openai_api_key".to_string(), "sk-test-123".to_string());
    secrets.insert("anthropic_api_key".to_string(), "ak-test-456".to_string());
    Arc::new(InMemoryProvider::new(secrets))
}

#[tokio::test]
async fn allowed_module_can_read_secret() {
    let provider = test_provider();
    let mut store = SecretStore::new(provider);
    store.register(
        "openai_api_key",
        HashSet::from(["runtime/model-runtime".to_string()]),
    );

    let value = store
        .get("runtime/model-runtime", "openai_api_key")
        .await
        .expect("should succeed");
    assert_eq!(value, "sk-test-123");
}

#[tokio::test]
async fn denied_module_cannot_read_secret() {
    let provider = test_provider();
    let mut store = SecretStore::new(provider);
    store.register(
        "openai_api_key",
        HashSet::from(["runtime/model-runtime".to_string()]),
    );

    let err = store
        .get("runtime/mcp-runtime", "openai_api_key")
        .await
        .expect_err("should be denied");
    assert!(matches!(err, SecretError::AccessDenied { .. }));
}

#[tokio::test]
async fn unregistered_key_returns_not_found() {
    let provider = test_provider();
    let store = SecretStore::new(provider);

    let err = store
        .get("runtime/model-runtime", "nonexistent")
        .await
        .expect_err("should not be found");
    assert!(matches!(err, SecretError::NotFound(_)));
}

#[tokio::test]
async fn keys_returns_sorted_registered_keys() {
    let provider = test_provider();
    let mut store = SecretStore::new(provider);
    store.register("zebra_key", HashSet::from(["mod-a".to_string()]));
    store.register("alpha_key", HashSet::from(["mod-b".to_string()]));

    let keys = store.keys();
    assert_eq!(keys, vec!["alpha_key", "zebra_key"]);
}

#[tokio::test]
async fn provider_missing_key_returns_not_found() {
    let provider = test_provider();
    let mut store = SecretStore::new(provider);
    store.register(
        "missing_key",
        HashSet::from(["runtime/model-runtime".to_string()]),
    );

    let err = store
        .get("runtime/model-runtime", "missing_key")
        .await
        .expect_err("provider should return not found");
    assert!(matches!(err, SecretError::NotFound(_)));
}
