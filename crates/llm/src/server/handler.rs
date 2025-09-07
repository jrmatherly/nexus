//! LLM handler that conditionally applies metrics

use crate::{
    messages::{ChatCompletionRequest, ChatCompletionResponse, ModelsResponse},
    provider::ChatCompletionStream,
    request::RequestContext,
    server::{LlmServer, LlmService, metrics::LlmServerWithMetrics},
};

/// LLM handler that optionally applies metrics based on configuration
#[derive(Clone)]
pub(crate) enum LlmHandler {
    /// Server with metrics recording enabled
    WithMetrics(LlmServerWithMetrics<LlmServer>),
    /// Server without metrics (direct calls)
    WithoutMetrics(LlmServer),
}

impl LlmHandler {
    /// List all available models from all providers.
    pub fn models(&self) -> ModelsResponse {
        match self {
            LlmHandler::WithMetrics(server) => server.models(),
            LlmHandler::WithoutMetrics(server) => server.models(),
        }
    }

    /// Process a chat completion request.
    pub async fn completions(
        &self,
        request: ChatCompletionRequest,
        context: &RequestContext,
    ) -> crate::Result<ChatCompletionResponse> {
        match self {
            LlmHandler::WithMetrics(server) => server.completions(request, context).await,
            LlmHandler::WithoutMetrics(server) => server.completions(request, context).await,
        }
    }

    /// Process a streaming chat completion request.
    pub async fn completions_stream(
        &self,
        request: ChatCompletionRequest,
        context: &RequestContext,
    ) -> crate::Result<ChatCompletionStream> {
        match self {
            LlmHandler::WithMetrics(server) => server.completions_stream(request, context).await,
            LlmHandler::WithoutMetrics(server) => server.completions_stream(request, context).await,
        }
    }
}
