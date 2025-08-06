use std::sync::Arc;
use std::time::Duration;

use config::{LlmConfig, LlmProvider};
use futures::{
    lock::Mutex,
    stream::{FuturesUnordered, StreamExt},
};
use itertools::Itertools;
use mini_moka::sync::Cache;

use crate::{
    error::LlmError,
    messages::{ChatCompletionRequest, ChatCompletionResponse, ModelsResponse, ObjectType},
    provider::{Provider, anthropic::AnthropicProvider, google::GoogleProvider, openai::OpenAIProvider},
};

// Cache models for 5 minutes
const MODELS_CACHE_DURATION: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub(crate) struct LlmServer {
    shared: Arc<LlmServerInner>,
}

struct LlmServerInner {
    providers: Vec<Box<dyn Provider>>,
    models_cache: Cache<(), ModelsResponse>,
    refresh_lock: Mutex<()>,
}

impl LlmServer {
    pub async fn new(config: LlmConfig) -> crate::Result<Self> {
        log::debug!("Initializing LLM server with {} providers", config.providers.len());
        let mut providers = Vec::with_capacity(config.providers.len());

        for (name, provider_config) in config.providers.into_iter() {
            log::debug!("Initializing provider: {name}");

            match provider_config {
                LlmProvider::Openai(config) => {
                    let provider = Box::new(OpenAIProvider::new(name.clone(), config)?);
                    providers.push(provider as Box<dyn Provider>)
                }
                LlmProvider::Anthropic(config) => {
                    let provider = Box::new(AnthropicProvider::new(name.clone(), config)?);
                    providers.push(provider as Box<dyn Provider>)
                }
                LlmProvider::Google(config) => {
                    let provider = Box::new(GoogleProvider::new(name.clone(), config)?);
                    providers.push(provider as Box<dyn Provider>)
                }
            }
        }

        log::debug!("LLM server initialized with {} active providers", providers.len());

        // Create cache with TTL
        let models_cache = Cache::builder().time_to_live(MODELS_CACHE_DURATION).build();

        Ok(Self {
            shared: Arc::new(LlmServerInner {
                providers,
                models_cache,
                refresh_lock: Mutex::new(()),
            }),
        })
    }

    /// Process a chat completion request.
    pub async fn completions(&self, mut request: ChatCompletionRequest) -> crate::Result<ChatCompletionResponse> {
        // Check if streaming was requested
        if request.stream.unwrap_or(false) {
            return Err(LlmError::StreamingNotSupported);
        }

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

        let mut response = provider.chat_completion(request).await?;

        // Restore the full model name with provider prefix in the response
        response.model = original_model;

        Ok(response)
    }

    /// List available models.
    pub async fn list_models(&self) -> crate::Result<ModelsResponse> {
        // Check cache first
        if let Some(cached) = self.shared.models_cache.get(&()) {
            log::debug!("Returning cached models (cache hit)");
            return Ok(cached);
        }

        let _guard = self.shared.refresh_lock.lock().await;

        if let Some(cached) = self.shared.models_cache.get(&()) {
            log::debug!("Returning cached models (cache hit)");
            return Ok(cached);
        }

        log::debug!(
            "Cache miss, fetching models from {} providers",
            self.shared.providers.len()
        );

        // Create futures for fetching models from each provider concurrently
        let mut model_futures = FuturesUnordered::new();

        for provider in &self.shared.providers {
            let provider_name = provider.name().to_string();
            let provider_ref = provider.as_ref();

            model_futures.push(async move {
                log::debug!("Fetching models from provider: {provider_name}");

                let models = match provider_ref.list_models().await {
                    Ok(models) => models,
                    Err(e) => {
                        log::warn!("Failed to list models from provider {provider_name}: {e}");

                        return Err(e);
                    }
                };

                // Prefix model IDs with provider name for clarity
                let prefixed_models: Vec<_> = models
                    .into_iter()
                    .map(|mut model| {
                        model.id = format!("{}/{}", provider_name, model.id);
                        model
                    })
                    .collect();

                Ok(prefixed_models)
            });
        }

        // Collect results from all providers concurrently
        let mut all_models = Vec::new();
        while let Some(result) = model_futures.next().await {
            if let Ok(models) = result {
                all_models.extend(models);
            }
            // Errors are already logged above, so we just skip them
        }

        let response = ModelsResponse {
            object: ObjectType::List,
            data: all_models,
        };

        // Cache the response
        self.shared.models_cache.insert((), response.clone());

        Ok(response)
    }

    fn get_provider(&self, name: &str) -> Option<&dyn Provider> {
        self.shared.providers.iter().find(|p| p.name() == name).map(|v| &**v)
    }
}
