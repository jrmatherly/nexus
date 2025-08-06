use indoc::formatdoc;
use std::future::Future;
use std::net::SocketAddr;

#[derive(Clone, Debug, Copy)]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Google,
}

/// Configuration for a test LLM provider
pub struct LlmProviderConfig {
    pub name: String,
    pub address: SocketAddr,
    pub provider_type: ProviderType,
}

/// Trait for test LLM providers
pub trait TestLlmProvider: Send + Sync + 'static {
    /// Get the provider type for config (e.g., "openai", "anthropic")
    fn provider_type(&self) -> &str;

    /// Get the provider name (used as the config key)
    fn name(&self) -> &str;

    /// Start the mock server and return its configuration
    fn spawn(self: Box<Self>) -> impl Future<Output = anyhow::Result<LlmProviderConfig>> + Send;

    /// Generate the configuration snippet for this provider
    fn generate_config(&self, config: &LlmProviderConfig) -> String {
        generate_config_for_type(config.provider_type, config)
    }
}

/// Generate configuration for a given provider type
pub fn generate_config_for_type(provider_type: ProviderType, config: &LlmProviderConfig) -> String {
    match provider_type {
        ProviderType::OpenAI => formatdoc! {r#"

            [llm.providers.{}]
            type = "openai"
            api_key = "test-key"
            api_url = "http://{}/v1"
        "#, config.name, config.address},

        ProviderType::Anthropic => formatdoc! {r#"

            [llm.providers.{}]
            type = "anthropic"
            api_key = "test-key"
            api_url = "http://{}/v1"
        "#, config.name, config.address},

        ProviderType::Google => formatdoc! {r#"

            [llm.providers.{}]
            type = "google"
            api_key = "test-key"
            api_url = "http://{}/v1beta"
        "#, config.name, config.address},
    }
}
