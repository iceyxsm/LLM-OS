use std::sync::Arc;

use common_types::{ActionRequest, ActionResult, ActionStatus, LlmOsError};
use llmos_model_runtime::InferenceBackend;
use tokio::sync::RwLock;
use tracing::info;

use crate::ActionExecutor;

/// An executor that delegates "model:invoke" actions to an InferenceBackend.
///
/// For actions that are not model invocations, it falls back to a simple
/// pass-through that marks the action as executed.
pub struct ModelExecutor {
    backend: Arc<RwLock<dyn InferenceBackend>>,
}

impl ModelExecutor {
    pub fn new(backend: Arc<RwLock<dyn InferenceBackend>>) -> Self {
        Self { backend }
    }
}

impl ActionExecutor for ModelExecutor {
    fn execute(&self, request: &ActionRequest) -> Result<ActionResult, LlmOsError> {
        // Only handle model:invoke actions through the backend.
        // All other actions pass through as executed.
        if request.action != "model:invoke" {
            return Ok(ActionResult {
                version: request.version.clone(),
                status: ActionStatus::Executed,
                message: format!(
                    "executed {} for subject {} on {}",
                    request.action, request.subject, request.resource
                ),
            });
        }

        // The ActionExecutor trait is synchronous, but InferenceBackend is async.
        // Use try_read to check if the backend is loaded without blocking.
        let backend = match self.backend.try_read() {
            Ok(guard) => guard,
            Err(_) => {
                return Err(LlmOsError::ActionDenied(
                    "model backend is busy; try again later".to_string(),
                ));
            }
        };

        if !backend.is_loaded() {
            return Ok(ActionResult {
                version: request.version.clone(),
                status: ActionStatus::Executed,
                message: "model:invoke accepted (no model loaded; mock pass-through)".to_string(),
            });
        }

        // For now, return a placeholder result. Real inference would need
        // an async executor path, which requires refactoring ActionExecutor
        // to be async. This is the integration point for that future change.
        info!(
            target: "llmd::executor",
            subject = %request.subject,
            resource = %request.resource,
            "model:invoke action accepted by model executor"
        );

        Ok(ActionResult {
            version: request.version.clone(),
            status: ActionStatus::Executed,
            message: format!(
                "model:invoke dispatched for {} on {}",
                request.subject, request.resource
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llmos_model_runtime::MockBackend;

    fn test_request(action: &str) -> ActionRequest {
        ActionRequest {
            version: "v1".to_string(),
            request_id: "req-1".to_string(),
            correlation_id: "corr-1".to_string(),
            subject: "runtime/model-runtime".to_string(),
            action: action.to_string(),
            resource: "test-model".to_string(),
        }
    }

    #[test]
    fn non_model_action_passes_through() {
        let backend = Arc::new(RwLock::new(MockBackend::new()));
        let executor = ModelExecutor::new(backend);
        let result = executor
            .execute(&test_request("network:connect"))
            .unwrap();
        assert_eq!(result.status, ActionStatus::Executed);
        assert!(result.message.contains("network:connect"));
    }

    #[test]
    fn model_invoke_without_loaded_model() {
        let backend = Arc::new(RwLock::new(MockBackend::new()));
        let executor = ModelExecutor::new(backend);
        let result = executor.execute(&test_request("model:invoke")).unwrap();
        assert_eq!(result.status, ActionStatus::Executed);
        assert!(result.message.contains("mock pass-through"));
    }

    #[tokio::test]
    async fn model_invoke_with_loaded_model() {
        let mut mock = MockBackend::new();
        mock.load(&llmos_model_runtime::ModelConfig::cpu_only("test", "/tmp/test.gguf"))
            .await
            .unwrap();
        let backend = Arc::new(RwLock::new(mock));
        let executor = ModelExecutor::new(backend);
        let result = executor.execute(&test_request("model:invoke")).unwrap();
        assert_eq!(result.status, ActionStatus::Executed);
        assert!(result.message.contains("model:invoke dispatched"));
    }
}
