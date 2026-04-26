use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("secret not found: {0}")]
    NotFound(String),
    #[error("access denied for module '{module}' to secret '{key}'")]
    AccessDenied { module: String, key: String },
    #[error("provider error: {0}")]
    Provider(String),
}

/// Trait for retrieving secrets from a backend.
///
/// Implementations may read from environment variables, files, vaults,
/// or any other credential store.
#[async_trait]
pub trait SecretProvider: Send + Sync {
    /// Retrieve the raw secret value for the given key.
    async fn get(&self, key: &str) -> Result<String, SecretError>;

    /// Check whether a key exists without retrieving its value.
    async fn exists(&self, key: &str) -> Result<bool, SecretError>;
}

/// A provider that reads secrets from environment variables.
///
/// Keys are uppercased and prefixed with `LLMOS_SECRET_` before lookup.
/// For example, requesting `openai_api_key` resolves to
/// `LLMOS_SECRET_OPENAI_API_KEY`.
#[derive(Debug, Clone, Default)]
pub struct EnvSecretProvider {
    prefix: String,
}

impl EnvSecretProvider {
    pub fn new() -> Self {
        Self {
            prefix: "LLMOS_SECRET_".to_string(),
        }
    }

    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    fn resolve_key(&self, key: &str) -> String {
        format!("{}{}", self.prefix, key.to_uppercase())
    }
}

#[async_trait]
impl SecretProvider for EnvSecretProvider {
    async fn get(&self, key: &str) -> Result<String, SecretError> {
        let env_key = self.resolve_key(key);
        std::env::var(&env_key).map_err(|_| SecretError::NotFound(key.to_string()))
    }

    async fn exists(&self, key: &str) -> Result<bool, SecretError> {
        let env_key = self.resolve_key(key);
        Ok(std::env::var(&env_key).is_ok())
    }
}
