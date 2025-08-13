use std::sync::Arc;

use config::{LlmConfig, ProviderType, StorageConfig};
use futures::stream::StreamExt;
use itertools::Itertools;
use rate_limit::{TokenRateLimitManager, TokenRateLimitRequest};

use crate::{
    error::LlmError,
    messages::{ChatCompletionRequest, ChatCompletionResponse, Model, ModelsResponse, ObjectType},
    provider::{
        ChatCompletionStream, Provider, anthropic::AnthropicProvider, google::GoogleProvider, openai::OpenAIProvider,
    },
    request::RequestContext,
};

#[derive(Clone)]
pub(crate) struct LlmServer {
    shared: Arc<LlmServerInner>,
}

struct LlmServerInner {
    providers: Vec<Box<dyn Provider>>,
    config: LlmConfig,
    token_rate_limiter: Option<TokenRateLimitManager>,
}

impl LlmServer {
    pub async fn new(config: LlmConfig, storage_config: &StorageConfig) -> crate::Result<Self> {
        log::debug!("Initializing LLM server with {} providers", config.providers.len());
        let mut providers = Vec::with_capacity(config.providers.len());

        for (name, provider_config) in config.providers.clone().into_iter() {
            log::debug!("Initializing provider: {name}");

            match provider_config.provider_type {
                ProviderType::Openai => {
                    let provider = Box::new(OpenAIProvider::new(name.clone(), provider_config)?);
                    providers.push(provider as Box<dyn Provider>)
                }
                ProviderType::Anthropic => {
                    let provider = Box::new(AnthropicProvider::new(name.clone(), provider_config)?);
                    providers.push(provider as Box<dyn Provider>)
                }
                ProviderType::Google => {
                    let provider = Box::new(GoogleProvider::new(name.clone(), provider_config)?);
                    providers.push(provider as Box<dyn Provider>)
                }
            }
        }

        // Check if any providers were successfully initialized
        if providers.is_empty() {
            return Err(LlmError::InternalError(Some(
                "Failed to initialize any LLM providers.".to_string(),
            )));
        } else {
            log::debug!("LLM server initialized with {} active provider(s)", providers.len());
        }

        // Initialize token rate limiter if any provider has rate limits configured
        let has_token_rate_limits = config
            .providers
            .values()
            .any(|p| p.rate_limits.is_some() || p.models.values().any(|m| m.rate_limits.is_some()));

        let token_rate_limiter = if has_token_rate_limits {
            Some(TokenRateLimitManager::new(storage_config).await.map_err(|e| {
                log::error!("Failed to initialize token rate limiter: {e}");
                LlmError::InternalError(None)
            })?)
        } else {
            None
        };

        Ok(Self {
            shared: Arc::new(LlmServerInner {
                providers,
                config: config.clone(),
                token_rate_limiter,
            }),
        })
    }

    /// Check token rate limits for a request.
    ///
    /// Returns `Some(Duration)` indicating how long to wait if rate limited, or `None` if allowed.
    ///
    /// # Token Counting Algorithm
    ///
    /// Total tokens = Input tokens + Output allowance where:
    /// - Input tokens: Counted from request messages using tiktoken
    /// - Output allowance: Determined by (in order of precedence):
    ///   1. Request's `max_tokens` parameter if specified
    ///   2. Rate limit's `output_buffer` configuration if available
    ///   3. 0 if neither is specified
    ///
    /// # Rate Limiting Behavior
    ///
    /// Token rate limiting is only enforced when:
    /// - Client identification is enabled and a client_id is present
    /// - The provider or model has rate limits configured
    /// - The token rate limiter is initialized
    ///
    /// When these conditions aren't met, the request is allowed (returns `None`).
    /// This is safe because the middleware layer enforces client identification
    /// when it's required by the configuration. And configuration enforces
    /// client identification when rate limiting is enabled.
    pub async fn check_rate_limit(
        &self,
        context: &RequestContext,
        request: &ChatCompletionRequest,
    ) -> Option<std::time::Duration> {
        // If no client identity, can't apply token rate limits
        // This is safe because middleware enforces client identification when required
        let Some(client_id) = context.client_id.as_ref() else {
            log::debug!(
                "No client identity available for rate limiting - allowing request. \
                Token rate limits require client identification to be enabled."
            );
            return None;
        };

        log::debug!(
            "Checking token rate limit for client_id={client_id}, group={:?}, model={}",
            context.group,
            request.model
        );

        // Extract provider and model from the request
        let (provider_name, model_name) = request.model.split_once('/')?;
        log::debug!("Parsed model: provider={}, model={}", provider_name, model_name);

        // Get provider config
        let provider_config = self.shared.config.providers.get(provider_name)?;

        // Get model config if it exists
        let model_config = provider_config.models.get(model_name);

        // Check rate limit if token rate limiter is configured
        let Some(ref token_rate_limiter) = self.shared.token_rate_limiter else {
            log::debug!(
                "Token rate limiter not initialized - no providers have token rate limits configured. \
                Allowing request without token rate limiting."
            );
            return None;
        };

        // Gather provider and model rate limit configurations
        let (provider_limits, model_limits) = (
            provider_config.rate_limits.as_ref(),
            model_config.and_then(|m| m.rate_limits.as_ref()),
        );

        // Resolve the applicable rate limit using the 4-level hierarchy:
        // 1. Model + Group, 2. Model default, 3. Provider + Group, 4. Provider default
        let rate_limit = rate_limit::resolve_token_rate_limit(context.group.as_deref(), provider_limits, model_limits);

        // Count request tokens
        let request_tokens = crate::token_counter::count_request_tokens(request);

        // Determine output allowance (pre-allocated tokens for response):
        // Priority: request.max_tokens > rate_limit.output_buffer > 0
        let output_allowance = if let Some(max) = request.max_tokens {
            max as usize
        } else if let Some(limit) = rate_limit {
            limit.output_buffer.unwrap_or(0) as usize
        } else {
            0
        };

        let total_tokens = request_tokens + output_allowance;
        log::debug!(
            "Token accounting: input={request_tokens}, output_allowance={output_allowance}, total={total_tokens}",
        );

        // Create token rate limit request
        let token_request = TokenRateLimitRequest {
            client_id: client_id.to_string(),
            group: context.group.clone(),
            provider: provider_name.to_string(),
            model: Some(model_name.to_string()),
            tokens: total_tokens,
        };

        match token_rate_limiter
            .check_request(&token_request, provider_limits, model_limits)
            .await
        {
            Ok(duration) => duration,
            Err(e) => {
                log::error!("Error checking token rate limit: {e}");
                None
            }
        }
    }

    /// Process a chat completion request.
    pub async fn completions(
        &self,
        mut request: ChatCompletionRequest,
        context: &RequestContext,
    ) -> crate::Result<ChatCompletionResponse> {
        // Note: Streaming is handled by completions_stream(), this method is for non-streaming only

        // Extract provider name from the model string (format: "provider/model")
        let Some((provider_name, model_name)) = request.model.split_once('/') else {
            return Err(LlmError::InvalidModelFormat(request.model.clone()));
        };

        let Some(provider) = self.get_provider(provider_name) else {
            log::error!(
                "Provider '{provider_name}' not found. Available providers: [{providers}]",
                providers = self.shared.providers.iter().map(|p| p.name()).join(", ")
            );

            return Err(LlmError::ProviderNotFound(provider_name.to_string()));
        };

        // Store the original model name before stripping the prefix
        let original_model = request.model.clone();
        request.model = model_name.to_string();

        let mut response = provider.chat_completion(request, context).await?;

        // Restore the full model name with provider prefix in the response
        response.model = original_model;

        Ok(response)
    }

    /// Process a streaming chat completion request.
    ///
    /// Returns a stream of completion chunks that are sent incrementally as the
    /// model generates the response. The stream is prefixed with the provider name
    /// to maintain consistency with the non-streaming API.
    pub async fn completions_stream(
        &self,
        mut request: ChatCompletionRequest,
        context: &RequestContext,
    ) -> crate::Result<ChatCompletionStream> {
        // Extract provider name from the model string (format: "provider/model")
        let Some((provider_name, model_name)) = request.model.split_once('/') else {
            return Err(LlmError::InvalidModelFormat(request.model.clone()));
        };

        let Some(provider) = self.get_provider(provider_name) else {
            log::error!(
                "Provider '{provider_name}' not found. Available providers: [{providers}]",
                providers = self.shared.providers.iter().map(|p| p.name()).join(", ")
            );

            return Err(LlmError::ProviderNotFound(provider_name.to_string()));
        };

        // Check if provider supports streaming
        if !provider.supports_streaming() {
            log::debug!("Provider '{provider_name}' does not support streaming");
            return Err(LlmError::StreamingNotSupported);
        }

        // Store the original model name for later
        let original_model = request.model.clone();

        // Strip the provider prefix from the model name for the provider
        request.model = model_name.to_string();

        // Get the stream from the provider
        let stream = provider.chat_completion_stream(request, context).await?;

        // Transform the stream to restore the full model name with prefix
        let transformed_stream = stream.map(move |chunk_result| {
            chunk_result.map(|mut chunk| {
                // Restore the full model name with provider prefix
                chunk.model = original_model.clone();
                chunk
            })
        });

        Ok(Box::pin(transformed_stream))
    }

    /// List available models.
    pub fn list_models(&self) -> ModelsResponse {
        let mut data = Vec::new();

        for provider in &self.shared.providers {
            for model in provider.list_models() {
                data.push(Model {
                    id: format!("{}/{}", provider.name(), model.id),
                    object: model.object,
                    created: model.created,
                    owned_by: model.owned_by,
                })
            }
        }

        ModelsResponse {
            object: ObjectType::List,
            data,
        }
    }

    fn get_provider(&self, name: &str) -> Option<&dyn Provider> {
        self.shared.providers.iter().find(|p| p.name() == name).map(|v| &**v)
    }
}
