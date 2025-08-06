pub(crate) mod anthropic;
pub(crate) mod google;
pub(crate) mod openai;

use async_trait::async_trait;

use crate::messages::{ChatCompletionRequest, ChatCompletionResponse, Model};

/// Trait for LLM provider implementations.
///
/// Note for async_trait: We need this trait to be dyn-compatible, so we can't just use the
/// Rust async trait functions without Box/Pin.
#[async_trait]
pub(crate) trait Provider: Send + Sync {
    /// Process a chat completion request.
    async fn chat_completion(&self, request: ChatCompletionRequest) -> crate::Result<ChatCompletionResponse>;

    /// List available models for this provider.
    async fn list_models(&self) -> crate::Result<Vec<Model>>;

    /// Get the provider name.
    fn name(&self) -> &str;
}
