use std::collections::BTreeMap;

use config::{LlmProviderConfig, ModelConfig};

use crate::messages::{Model, ObjectType};

/// Manages model configuration and resolution for LLM providers.
///
/// This struct handles the common logic for:
/// - Resolving model names (with rename support)
/// - Listing configured models
/// - Backward compatibility (empty config = allow all models)
pub(crate) struct ModelManager {
    models: BTreeMap<String, ModelConfig>,
    owner: String,
}

impl ModelManager {
    /// Create a new ModelManager from provider configuration.
    pub fn new(config: &LlmProviderConfig, owner: impl Into<String>) -> Self {
        Self {
            models: config.models.clone(),
            owner: owner.into(),
        }
    }

    /// Check if a model is configured and resolve its actual name.
    ///
    /// Returns the renamed model if configured, or the original name if rename is not specified.
    /// If no models are configured (backward compatibility), returns the requested model.
    pub fn resolve_model(&self, requested_model: &str) -> Option<String> {
        // If no models are configured, allow any model (backward compatibility)
        if self.models.is_empty() {
            return Some(requested_model.to_string());
        }

        // Check if the requested model is explicitly configured
        self.models
            .get(requested_model)
            .map(|model_config| model_config.rename.as_deref().unwrap_or(requested_model).to_string())
    }

    /// Get list of configured models for the /models endpoint.
    ///
    /// Returns an empty list when no models are configured (Phase 2 behavior).
    /// In Phase 3, this would return an error instead.
    pub fn get_configured_models(&self) -> Vec<Model> {
        if self.models.is_empty() {
            // Return empty list when no models configured
            // In Phase 3, this would return an error instead
            return Vec::new();
        }

        self.models
            .keys()
            .map(|model_id| Model {
                id: model_id.clone(),
                object: ObjectType::Model,
                created: 1719475200, // Fixed timestamp for Phase 2
                owned_by: self.owner.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::ModelConfig;

    #[test]
    fn empty_config_allows_any_model() {
        let config = LlmProviderConfig {
            provider_type: config::ProviderType::Openai,
            api_key: None,
            base_url: None,
            forward_token: false,
            models: BTreeMap::new(),
        };

        let manager = ModelManager::new(&config, "test");

        assert_eq!(manager.resolve_model("gpt-4"), Some("gpt-4".to_string()));
        assert_eq!(manager.resolve_model("any-model"), Some("any-model".to_string()));
    }

    #[test]
    fn configured_model_without_rename() {
        let mut models = BTreeMap::new();
        models.insert("gpt-4".to_string(), ModelConfig { rename: None });

        let config = LlmProviderConfig {
            provider_type: config::ProviderType::Openai,
            api_key: None,
            base_url: None,
            forward_token: false,
            models,
        };

        let manager = ModelManager::new(&config, "test");

        assert_eq!(manager.resolve_model("gpt-4"), Some("gpt-4".to_string()));
        assert_eq!(manager.resolve_model("gpt-3.5"), None);
    }

    #[test]
    fn configured_model_with_rename() {
        let mut models = BTreeMap::new();
        models.insert(
            "claude".to_string(),
            ModelConfig {
                rename: Some("claude-3-opus-20240229".to_string()),
            },
        );

        let config = LlmProviderConfig {
            provider_type: config::ProviderType::Anthropic,
            api_key: None,
            base_url: None,
            forward_token: false,
            models,
        };

        let manager = ModelManager::new(&config, "anthropic");

        assert_eq!(
            manager.resolve_model("claude"),
            Some("claude-3-opus-20240229".to_string())
        );
        assert_eq!(manager.resolve_model("claude-3-opus-20240229"), None);
    }

    #[test]
    fn get_configured_models_returns_list() {
        let mut models = BTreeMap::new();
        models.insert("gpt-4".to_string(), ModelConfig { rename: None });
        models.insert("gpt-3.5-turbo".to_string(), ModelConfig { rename: None });

        let config = LlmProviderConfig {
            provider_type: config::ProviderType::Openai,
            api_key: None,
            base_url: None,
            forward_token: false,
            models,
        };

        let manager = ModelManager::new(&config, "openai");
        let model_list = manager.get_configured_models();

        assert_eq!(model_list.len(), 2);
        assert!(model_list.iter().any(|m| m.id == "gpt-4"));
        assert!(model_list.iter().any(|m| m.id == "gpt-3.5-turbo"));
        assert!(model_list.iter().all(|m| m.owned_by == "openai"));
    }

    #[test]
    fn empty_models_returns_empty_list() {
        let config = LlmProviderConfig {
            provider_type: config::ProviderType::Google,
            api_key: None,
            base_url: None,
            forward_token: false,
            models: BTreeMap::new(),
        };

        let manager = ModelManager::new(&config, "google");
        let model_list = manager.get_configured_models();

        assert!(model_list.is_empty());
    }
}
