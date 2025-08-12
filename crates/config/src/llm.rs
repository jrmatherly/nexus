//! LLM configuration structures for AI model providers.

use std::borrow::Cow;
use std::collections::BTreeMap;

use secrecy::SecretString;
use serde::Deserialize;

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

    /// Get providers in the old format for compatibility.
    /// TODO: Remove after Phase 2
    pub fn into_providers_compat(self) -> BTreeMap<String, LlmProvider> {
        self.providers
            .into_iter()
            .map(|(name, config)| {
                let provider = match config.provider_type {
                    ProviderType::Openai => LlmProvider::Openai(config),
                    ProviderType::Anthropic => LlmProvider::Anthropic(config),
                    ProviderType::Google => LlmProvider::Google(config),
                };
                (name, provider)
            })
            .collect()
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

// Temporary compatibility layer for Phase 1
// TODO: Remove these after Phase 2 when LLM crate is updated
/// Compatibility alias for OpenAI configuration.
pub type OpenAiConfig = LlmProviderConfig;
/// Compatibility alias for Anthropic configuration.
pub type AnthropicConfig = LlmProviderConfig;
/// Compatibility alias for Google configuration.
pub type GoogleConfig = LlmProviderConfig;

/// Compatibility enum for LLM providers.
#[derive(Debug, Clone)]
pub enum LlmProvider {
    /// OpenAI provider.
    Openai(LlmProviderConfig),
    /// Anthropic provider.
    Anthropic(LlmProviderConfig),
    /// Google provider.
    Google(LlmProviderConfig),
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
    #[serde(default)]
    pub models: BTreeMap<String, ModelConfig>,
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
                    models: {},
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
                    models: {},
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
                    models: {},
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

            [providers.anthropic]
            type = "anthropic"
            api_key = "{{ env.ANTHROPIC_API_KEY }}"

            [providers.google]
            type = "google"
            api_key = "{{ env.GOOGLE_KEY }}"
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
                    models: {},
                },
                "google": LlmProviderConfig {
                    provider_type: Google,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {},
                },
                "openai": LlmProviderConfig {
                    provider_type: Openai,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {},
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
                    models: {},
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

            [providers.anthropic]
            type = "anthropic"
            forward_token = true
            # No api_key provided - relies entirely on token forwarding

            [providers.google]
            type = "google"
            api_key = "{{ env.GOOGLE_KEY }}"
            forward_token = false  # Explicitly disabled
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
                    models: {},
                },
                "google": LlmProviderConfig {
                    provider_type: Google,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: false,
                    models: {},
                },
                "openai": LlmProviderConfig {
                    provider_type: Openai,
                    api_key: Some(
                        SecretBox<str>([REDACTED]),
                    ),
                    base_url: None,
                    forward_token: true,
                    models: {},
                },
            },
        }
        "#);
    }
}
