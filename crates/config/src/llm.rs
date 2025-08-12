//! LLM configuration structures for AI model providers.

use std::borrow::Cow;
use std::collections::BTreeMap;

use secrecy::SecretString;
use serde::{Deserialize, Deserializer};

/// Configuration for an individual model within a provider.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    /// Optional rename - the actual provider model name.
    /// If not specified, the model ID (map key) is used.
    #[serde(default)]
    pub rename: Option<String>,
}

/// LLM configuration for AI model integration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LlmConfig {
    /// Whether the LLM functionality is enabled.
    enabled: bool,

    /// The path where the LLM endpoints will be mounted.
    pub path: Cow<'static, str>,

    /// Map of LLM provider configurations.
    pub providers: BTreeMap<String, LlmProviderConfig>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: Cow::Borrowed("/llm"),
            providers: BTreeMap::new(),
        }
    }
}

impl LlmConfig {
    /// Whether the LLM functionality is enabled.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Whether there are any LLM providers configured.
    pub fn has_providers(&self) -> bool {
        !self.providers.is_empty()
    }
}

/// Provider type enumeration.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderType {
    /// OpenAI provider.
    Openai,
    /// Anthropic provider.
    Anthropic,
    /// Google provider.
    Google,
}

/// Unified LLM provider configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmProviderConfig {
    /// The type of provider.
    #[serde(rename = "type")]
    pub provider_type: ProviderType,

    /// API key for the provider (supports environment variable interpolation).
    /// This key is used as a fallback when token forwarding is enabled and no user key is provided.
    /// When token forwarding is disabled, this is the primary API key.
    #[serde(default)]
    pub api_key: Option<SecretString>,

    /// Custom base URL for the provider.
    /// Each provider has its own default if not specified:
    /// - OpenAI: https://api.openai.com/v1
    /// - Anthropic: https://api.anthropic.com/v1
    /// - Google: https://generativelanguage.googleapis.com/v1beta
    #[serde(default)]
    pub base_url: Option<String>,

    /// Enable token forwarding - allows users to provide their own API keys via headers.
    #[serde(default)]
    pub forward_token: bool,

    /// Explicitly configured models for this provider.
    /// Phase 3: At least one model must be configured.
    #[serde(deserialize_with = "deserialize_non_empty_models_with_default")]
    pub models: BTreeMap<String, ModelConfig>,
}

/// Custom deserializer that ensures at least one model is configured.
/// This handles both missing field (uses default) and empty map cases.
fn deserialize_non_empty_models_with_default<'de, D>(deserializer: D) -> Result<BTreeMap<String, ModelConfig>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    // First deserialize as Option to handle missing field
    let models_opt = Option::<BTreeMap<String, ModelConfig>>::deserialize(deserializer)?;

    // Get the models map, using empty map if field was missing
    let models = models_opt.unwrap_or_default();

    // Now validate that we have at least one model
    if models.is_empty() {
        Err(Error::custom("At least one model must be configured for each provider"))
    } else {
        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use insta::assert_debug_snapshot;

    #[test]
    fn llm_config_defaults() {
        let config: LlmConfig = toml::from_str("").unwrap();

        assert_debug_snapshot!(&config, @r#"
        LlmConfig {
            enabled: true,
            path: "/llm",
            providers: {},
        }
        "#);
    }

    #[test]
    fn llm_config_with_openai() {
        let config = indoc! {r#"
            enabled = true
            path = "/llm"

            [providers.openai]
            type = "openai"
            api_key = "${OPENAI_API_KEY}"
            
            [providers.openai.models.gpt-4]
            
            [providers.openai.models.gpt-3-5-turbo]
        "#};

        let config: LlmConfig = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config, @r#"
        LlmConfig {
            enabled: true,
            path: "/llm",
            providers: {
                "openai": LlmProviderConfig {
                    provider_type: Openai,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {
                        "gpt-3-5-turbo": ModelConfig {
                            rename: None,
                        },
                        "gpt-4": ModelConfig {
                            rename: None,
                        },
                    },
                },
            },
        }
        "#);
    }

    #[test]
    fn llm_config_with_anthropic() {
        let config = indoc! {r#"
            enabled = true
            path = "/llm"

            [providers.anthropic]
            type = "anthropic"
            api_key = "{{ env.ANTHROPIC_API_KEY }}"
            
            [providers.anthropic.models.claude-3-opus]
            
            [providers.anthropic.models.claude-3-sonnet]
        "#};

        let config: LlmConfig = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config, @r#"
        LlmConfig {
            enabled: true,
            path: "/llm",
            providers: {
                "anthropic": LlmProviderConfig {
                    provider_type: Anthropic,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {
                        "claude-3-opus": ModelConfig {
                            rename: None,
                        },
                        "claude-3-sonnet": ModelConfig {
                            rename: None,
                        },
                    },
                },
            },
        }
        "#);
    }

    #[test]
    fn llm_config_with_google() {
        let config = indoc! {r#"
            [providers.google]
            type = "google"
            api_key = "{{ env.GOOGLE_KEY }}"
            
            [providers.google.models.gemini-pro]
            
            [providers.google.models.gemini-pro-vision]
        "#};

        let config: LlmConfig = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config, @r#"
        LlmConfig {
            enabled: true,
            path: "/llm",
            providers: {
                "google": LlmProviderConfig {
                    provider_type: Google,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {
                        "gemini-pro": ModelConfig {
                            rename: None,
                        },
                        "gemini-pro-vision": ModelConfig {
                            rename: None,
                        },
                    },
                },
            },
        }
        "#);
    }

    #[test]
    fn llm_config_with_multiple_providers() {
        let config = indoc! {r#"
            enabled = true
            path = "/ai"

            [providers.openai]
            type = "openai"
            api_key = "${OPENAI_API_KEY}"
            
            [providers.openai.models.gpt-4]

            [providers.anthropic]
            type = "anthropic"
            api_key = "{{ env.ANTHROPIC_API_KEY }}"
            
            [providers.anthropic.models.claude-3-opus]

            [providers.google]
            type = "google"
            api_key = "{{ env.GOOGLE_KEY }}"
            
            [providers.google.models.gemini-pro]
        "#};

        let config: LlmConfig = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config, @r#"
        LlmConfig {
            enabled: true,
            path: "/ai",
            providers: {
                "anthropic": LlmProviderConfig {
                    provider_type: Anthropic,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {
                        "claude-3-opus": ModelConfig {
                            rename: None,
                        },
                    },
                },
                "google": LlmProviderConfig {
                    provider_type: Google,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {
                        "gemini-pro": ModelConfig {
                            rename: None,
                        },
                    },
                },
                "openai": LlmProviderConfig {
                    provider_type: Openai,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {
                        "gpt-4": ModelConfig {
                            rename: None,
                        },
                    },
                },
            },
        }
        "#);
    }

    #[test]
    fn llm_config_disabled() {
        let config = indoc! {r#"
            enabled = false
        "#};

        let config: LlmConfig = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config, @r#"
        LlmConfig {
            enabled: false,
            path: "/llm",
            providers: {},
        }
        "#);
    }

    #[test]
    fn llm_config_custom_path() {
        let config = indoc! {r#"
            path = "/models"
        "#};

        let config: LlmConfig = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config, @r#"
        LlmConfig {
            enabled: true,
            path: "/models",
            providers: {},
        }
        "#);
    }

    #[test]
    fn llm_config_invalid_provider_type() {
        let config = indoc! {r#"
            [providers.invalid]
            type = "unknown-provider"
            api_key = "key"
        "#};

        let result: Result<LlmConfig, _> = toml::from_str(config);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("unknown variant"));
    }

    #[test]
    fn llm_config_with_static_api_key() {
        let config = indoc! {r#"
            [providers.openai]
            type = "openai"
            api_key = "sk-1234567890abcdef"
            
            [providers.openai.models.gpt-4]
        "#};

        let config: LlmConfig = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config, @r#"
        LlmConfig {
            enabled: true,
            path: "/llm",
            providers: {
                "openai": LlmProviderConfig {
                    provider_type: Openai,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {
                        "gpt-4": ModelConfig {
                            rename: None,
                        },
                    },
                },
            },
        }
        "#);
    }

    #[test]
    fn llm_config_with_explicit_models() {
        let config = indoc! {r#"
            [providers.openai]
            type = "openai"
            api_key = "key"
            
            [providers.openai.models.gpt-4]
            rename = "gpt-4-turbo-preview"
            
            [providers.openai.models.gpt-3-5]
            rename = "gpt-3.5-turbo"
        "#};

        let config: LlmConfig = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config, @r#"
        LlmConfig {
            enabled: true,
            path: "/llm",
            providers: {
                "openai": LlmProviderConfig {
                    provider_type: Openai,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {
                        "gpt-3-5": ModelConfig {
                            rename: Some(
                                "gpt-3.5-turbo",
                            ),
                        },
                        "gpt-4": ModelConfig {
                            rename: Some(
                                "gpt-4-turbo-preview",
                            ),
                        },
                    },
                },
            },
        }
        "#);
    }

    #[test]
    fn llm_config_models_without_rename() {
        let config = indoc! {r#"
            [providers.openai]
            type = "openai"
            api_key = "key"
            
            [providers.openai.models.gpt-4]
            # No rename - will use "gpt-4" as-is
            
            [providers.openai.models.custom-model]
            # No fields at all
        "#};

        let config: LlmConfig = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config, @r#"
        LlmConfig {
            enabled: true,
            path: "/llm",
            providers: {
                "openai": LlmProviderConfig {
                    provider_type: Openai,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {
                        "custom-model": ModelConfig {
                            rename: None,
                        },
                        "gpt-4": ModelConfig {
                            rename: None,
                        },
                    },
                },
            },
        }
        "#);
    }

    #[test]
    fn llm_config_mixed_providers_with_models() {
        let config = indoc! {r#"
            [providers.openai]
            type = "openai"
            api_key = "key1"
            
            [providers.openai.models.gpt-4]
            rename = "gpt-4-turbo"
            
            [providers.anthropic]
            type = "anthropic"
            api_key = "key2"
            
            [providers.anthropic.models.claude-3]
            rename = "claude-3-opus-20240229"
            
            [providers.anthropic.models.claude-instant]
            # No rename
        "#};

        let config: LlmConfig = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config, @r#"
        LlmConfig {
            enabled: true,
            path: "/llm",
            providers: {
                "anthropic": LlmProviderConfig {
                    provider_type: Anthropic,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {
                        "claude-3": ModelConfig {
                            rename: Some(
                                "claude-3-opus-20240229",
                            ),
                        },
                        "claude-instant": ModelConfig {
                            rename: None,
                        },
                    },
                },
                "openai": LlmProviderConfig {
                    provider_type: Openai,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {
                        "gpt-4": ModelConfig {
                            rename: Some(
                                "gpt-4-turbo",
                            ),
                        },
                    },
                },
            },
        }
        "#);
    }

    #[test]
    fn llm_config_with_forward_token_enabled() {
        let config = indoc! {r#"
            [providers.openai]
            type = "openai"
            api_key = "sk-fallback-key"
            forward_token = true
            
            [providers.openai.models.gpt-4]

            [providers.anthropic]
            type = "anthropic"
            forward_token = true
            # No api_key provided - relies entirely on token forwarding
            
            [providers.anthropic.models.claude-3-opus]

            [providers.google]
            type = "google"
            api_key = "{{ env.GOOGLE_KEY }}"
            forward_token = false  # Explicitly disabled
            
            [providers.google.models.gemini-pro]
        "#};

        let config: LlmConfig = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config, @r#"
        LlmConfig {
            enabled: true,
            path: "/llm",
            providers: {
                "anthropic": LlmProviderConfig {
                    provider_type: Anthropic,
                    api_key: None,
                    base_url: None,
                    forward_token: true,
                    models: {
                        "claude-3-opus": ModelConfig {
                            rename: None,
                        },
                    },
                },
                "google": LlmProviderConfig {
                    provider_type: Google,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {
                        "gemini-pro": ModelConfig {
                            rename: None,
                        },
                    },
                },
                "openai": LlmProviderConfig {
                    provider_type: Openai,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: true,
                    models: {
                        "gpt-4": ModelConfig {
                            rename: None,
                        },
                    },
                },
            },
        }
        "#);
    }
}
