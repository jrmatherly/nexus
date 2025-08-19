//! AWS Bedrock LLM provider implementation.
//!
//! This module provides integration with AWS Bedrock foundation models through
//! the Nexus LLM interface. It supports multiple model families (Anthropic, Amazon,
//! Meta, Mistral, Cohere, AI21) with automatic request/response format transformation.

mod families;

use async_trait::async_trait;
use aws_config::Region;
use aws_credential_types::Credentials;
use aws_sdk_bedrockruntime::{
    Client as BedrockRuntimeClient, error::ProvideErrorMetadata,
    operation::invoke_model_with_response_stream::InvokeModelWithResponseStreamOutput,
};
use aws_smithy_runtime_api::client::result::SdkError;
use futures::{Stream, stream};
use secrecy::ExposeSecret;
use std::{collections::HashMap, pin::Pin};

use crate::{
    error::LlmError,
    messages::{ChatCompletionRequest, ChatCompletionResponse, Model},
    provider::{ChatCompletionStream, ModelManager, Provider},
    request::RequestContext,
};

use config::BedrockProviderConfig;

pub use families::ModelFamily;

/// AWS Bedrock provider implementation.
///
/// This provider supports multiple model families available through AWS Bedrock:
/// - Anthropic (Claude models)
/// - Amazon (Titan models)
/// - Meta (Llama models)
/// - Mistral (Mistral models)
/// - Cohere (Command models)
/// - AI21 (Jurassic/Jamba models)
///
/// Each model family has its own request/response format that is automatically
/// transformed to maintain compatibility with the unified Nexus interface.
pub(crate) struct BedrockProvider {
    /// AWS Bedrock Runtime client for making API calls
    client: BedrockRuntimeClient,
    /// AWS region for this provider instance
    #[allow(dead_code)]
    region: String,
    /// Provider instance name
    name: String,
    /// Model manager for resolving and validating configured models
    model_manager: ModelManager,
    /// Cache of model family mappings for quick lookup
    model_families: HashMap<String, ModelFamily>,
}

impl BedrockProvider {
    /// Create a new Bedrock provider instance.
    ///
    /// # Arguments
    /// * `name` - Unique name for this provider instance
    /// * `config` - Configuration containing AWS credentials, region, and models
    ///
    /// # Errors
    /// Returns an error if:
    /// - Required region is not specified
    /// - AWS credentials cannot be resolved
    /// - No models are configured
    /// - Model family cannot be determined from model IDs
    pub async fn new(name: String, config: BedrockProviderConfig) -> crate::Result<Self> {
        let sdk_config = create_aws_config(&config).await?;
        let client = BedrockRuntimeClient::new(&sdk_config);

        let model_manager = ModelManager::new(config.models.clone(), &name);

        // Pre-compute model family mappings for performance
        let mut model_families = HashMap::new();

        for model_id in config.models.keys() {
            let actual_model_id = model_manager
                .resolve_model(model_id)
                .ok_or_else(|| LlmError::InvalidModelFormat(format!("Model '{}' not found", model_id)))?;

            let family = actual_model_id.parse::<ModelFamily>().map_err(|e| {
                LlmError::InvalidModelFormat(format!("Unable to determine model family for '{actual_model_id}': {e}"))
            })?;

            model_families.insert(model_id.to_string(), family);
        }

        Ok(Self {
            client,
            region: config.region.clone(),
            name,
            model_manager,
            model_families,
        })
    }

    /// Prepare a streaming request by validating and transforming the input.
    ///
    /// This method handles model resolution, family lookup, streaming validation,
    /// and request body transformation in a single step.
    fn prepare_streaming_request(&self, mut request: ChatCompletionRequest) -> crate::Result<StreamingSetup> {
        // Resolve the configured model to the actual Bedrock model ID
        let actual_model_id = self
            .model_manager
            .resolve_model(&request.model)
            .ok_or_else(|| LlmError::ModelNotFound(format!("Model '{}' is not configured", request.model)))?;

        log::debug!(
            "Resolved model '{}' to Bedrock model ID: {}",
            request.model,
            actual_model_id
        );

        // Get the model family for this model
        let model_family = self.model_families.get(&request.model).ok_or_else(|| {
            LlmError::InternalError(Some(format!("Model family not found for model: {}", request.model)))
        })?;

        // Check if the model family supports streaming
        if !model_family.supports_streaming() {
            return Err(LlmError::StreamingNotSupported);
        }

        log::debug!(
            "Using model family: {:?} for streaming model: {}",
            model_family,
            request.model
        );

        // Keep track of original model name for response
        let original_model = request.model.clone();

        // Set the actual model ID for the request transformation
        request.model = actual_model_id.clone();

        // Transform the request to the appropriate vendor format with streaming enabled
        let request_body = model_family.transform_streaming_request(request)?;

        Ok(StreamingSetup {
            actual_model_id,
            original_model,
            request_body,
            model_family: *model_family,
        })
    }

    /// Create a response stream from the Bedrock streaming output.
    ///
    /// This method processes the raw AWS streaming events and converts them to
    /// OpenAI-compatible chat completion chunks.
    fn create_response_stream(
        &self,
        stream_output: InvokeModelWithResponseStreamOutput,
        model_family: ModelFamily,
        original_model: String,
    ) -> Pin<Box<dyn Stream<Item = crate::Result<crate::messages::ChatCompletionChunk>> + Send>> {
        let created = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let message_id = format!("bedrock-{}", uuid::Uuid::new_v4());
        let provider_name = self.name.clone();
        let original_model_for_closure = original_model;

        Box::pin(stream::unfold(stream_output.body, move |mut event_receiver| {
            let message_id = message_id.clone();
            let provider_name = provider_name.clone();
            let original_model = original_model_for_closure.clone();

            async move {
                loop {
                    match event_receiver.recv().await {
                        Ok(Some(event)) => {
                            if let aws_sdk_bedrockruntime::types::ResponseStream::Chunk(chunk) = event {
                                let json_data = chunk.bytes.as_ref().map(|b| b.as_ref()).unwrap_or(&[]);

                                // Parse the chunk using the model family
                                if let Some(mut parsed_chunk) = model_family.parse_stream_chunk(json_data) {
                                    // Set the metadata fields that the into_chunk methods left empty
                                    parsed_chunk.id = message_id.clone();
                                    parsed_chunk.created = created;
                                    parsed_chunk.model = format!("{provider_name}/{original_model}");

                                    return Some((Ok(parsed_chunk), event_receiver));
                                }
                            }
                            // Continue loop if no chunk was produced
                        }
                        Ok(None) => {
                            // Stream ended
                            return None;
                        }
                        Err(e) => {
                            log::error!("Error receiving from Bedrock stream: {e:?}");
                            return Some((
                                Err(crate::error::LlmError::ConnectionError(format!("Stream error: {e:?}"))),
                                event_receiver,
                            ));
                        }
                    }
                }
            }
        }))
    }
}

#[async_trait]
impl Provider for BedrockProvider {
    async fn chat_completion(
        &self,
        mut request: ChatCompletionRequest,
        _: &RequestContext,
    ) -> crate::Result<ChatCompletionResponse> {
        log::debug!("Processing Bedrock chat completion for model: {}", request.model);

        // Resolve the configured model to the actual Bedrock model ID
        let actual_model_id = self
            .model_manager
            .resolve_model(&request.model)
            .ok_or_else(|| LlmError::ModelNotFound(format!("Model '{}' is not configured", request.model)))?;

        log::debug!(
            "Resolved model '{}' to Bedrock model ID: {actual_model_id}",
            request.model
        );

        // Get the model family for this model
        let family = self.model_families.get(&request.model).ok_or_else(|| {
            LlmError::InternalError(Some(format!("Model family not found for model: {}", request.model)))
        })?;

        log::debug!("Using model family: {:?} for model: {}", family, request.model);

        // Keep the original model name for the response
        let original_model = request.model.clone();

        // Set the actual model ID for Anthropic
        request.model = actual_model_id.clone();

        // Transform the request to the appropriate vendor format
        let request_body = family.transform_request(request).map_err(|e| {
            log::error!("Failed to transform request for model {actual_model_id}: {e}");

            match e {
                // Preserve user-facing errors
                LlmError::InvalidModelFormat(_) | LlmError::InvalidRequest(_) => e,
                // Hide internal errors
                _ => LlmError::InternalError(None),
            }
        })?;

        log::debug!("Transformed request body size: {} bytes", request_body.as_ref().len());

        // Log the actual request body for debugging
        if log::log_enabled!(log::Level::Debug)
            && let Ok(body_str) = std::str::from_utf8(request_body.as_ref())
        {
            log::debug!("Request body JSON: {}", body_str);
        }

        log::debug!(
            "Invoking model with ID: {} in region: {:?}",
            actual_model_id,
            self.region
        );

        // Make the API call to Bedrock
        let invoke_result = self
            .client
            .invoke_model()
            .model_id(&actual_model_id)
            .content_type("application/json")
            .accept("application/json")
            .body(request_body)
            .send()
            .await
            .map_err(|e| {
                log::error!("Failed to invoke model '{}': {:?}", actual_model_id, e);
                e
            })?;

        log::debug!(
            "Received response from Bedrock, body size: {} bytes",
            invoke_result.body.as_ref().len()
        );

        // Log the response body for debugging
        if log::log_enabled!(log::Level::Debug)
            && let Ok(body_str) = std::str::from_utf8(invoke_result.body.as_ref())
        {
            log::debug!("Response body JSON: {}", body_str);
        }

        // Transform the response back to the unified format
        let mut response = family.transform_response(invoke_result.body.as_ref()).map_err(|e| {
            log::error!("Failed to transform response for model {actual_model_id}: {e}");

            match e {
                // Preserve user-facing errors
                LlmError::InvalidModelFormat(_) | LlmError::InvalidRequest(_) => e,
                // Hide internal errors
                _ => LlmError::InternalError(None),
            }
        })?;

        // Set the model name (without provider prefix - server.rs will add it back)
        response.model = original_model;

        log::debug!("Successfully processed chat completion for model: {actual_model_id}");

        Ok(response)
    }

    async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
        _: &RequestContext,
    ) -> crate::Result<ChatCompletionStream> {
        log::debug!(
            "Processing Bedrock streaming chat completion for model: {}",
            request.model
        );

        let request = self.prepare_streaming_request(request)?;
        log::debug!("Prepared streaming request for model: {}", request.actual_model_id);

        let stream = self
            .client
            .invoke_model_with_response_stream()
            .model_id(request.actual_model_id.clone())
            .body(request.request_body)
            .send()
            .await
            .map_err(|e| {
                log::error!("Failed to invoke streaming model: {e}");
                log::error!("Error details: {e:?}");
                crate::error::LlmError::ConnectionError(e.to_string())
            })?;

        log::debug!("Successfully invoked streaming model: {}", request.actual_model_id);

        let stream = self.create_response_stream(stream, request.model_family, request.original_model.clone());

        log::debug!(
            "Successfully created streaming response for model: {}",
            request.original_model
        );

        Ok(Box::pin(stream))
    }

    fn supports_streaming(&self) -> bool {
        // Most Bedrock models support streaming
        true
    }

    fn list_models(&self) -> Vec<Model> {
        self.model_manager.get_configured_models()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Streaming request preparation data.
///
/// This structure holds all the information needed to make a streaming request
/// after the initial validation and transformation steps.
struct StreamingSetup {
    /// The actual Bedrock model ID to use for the API call
    actual_model_id: String,
    /// The original model name from the request (for response formatting)
    original_model: String,
    /// The transformed request body ready for Bedrock API
    request_body: aws_smithy_types::Blob,
    /// The model family for this request
    model_family: ModelFamily,
}

/// Create AWS configuration from Bedrock-specific config.
///
/// This handles the AWS credential chain resolution:
/// 1. Explicit credentials from config (access_key_id + secret_access_key)
/// 2. AWS profile specified in config
/// 3. Default AWS credential chain (env vars, files, IAM, etc.)
/// 4. Custom endpoint URL for testing with mock servers
async fn create_aws_config(config: &BedrockProviderConfig) -> crate::Result<aws_config::SdkConfig> {
    let mut aws_config_builder = aws_config::from_env().region(Region::new(config.region.clone()));

    // Set custom endpoint for testing/development if provided
    if let Some(endpoint_url) = &config.base_url {
        log::debug!("Using custom Bedrock endpoint: {endpoint_url}");
        aws_config_builder = aws_config_builder.endpoint_url(endpoint_url);
    }

    // Handle explicit credentials if provided
    if let (Some(access_key_id), Some(secret_access_key)) = (&config.access_key_id, &config.secret_access_key) {
        aws_config_builder = aws_config_builder.credentials_provider(Credentials::new(
            access_key_id.expose_secret(),
            secret_access_key.expose_secret(),
            config.session_token.as_ref().map(|t| t.expose_secret().to_string()),
            None,
            "nexus-bedrock",
        ));
    }
    // Handle profile if specified (only if no explicit credentials)
    else if let Some(profile) = &config.profile {
        aws_config_builder = aws_config_builder.profile_name(profile);
    }

    // Load the AWS configuration
    let aws_config = aws_config_builder.load().await;

    Ok(aws_config)
}

/// Convert AWS SDK errors to LlmError.
///
/// This implementation maps AWS Bedrock service errors to appropriate LlmError variants
/// based on the error code returned by the service.
impl<E, R> From<SdkError<E, R>> for LlmError
where
    E: ProvideErrorMetadata + std::fmt::Display + std::fmt::Debug,
    R: std::fmt::Debug,
{
    fn from(err: SdkError<E, R>) -> Self {
        // Log the error first
        log::error!("AWS Bedrock API error: {}", err);
        log::error!("AWS Bedrock error details: {err:?}");

        // Map based on error code if it's a service error
        if let SdkError::ServiceError(service_err) = &err {
            log::error!("Service error code: {:?}", service_err.err().code());
            match service_err.err().code() {
                Some("ValidationException") => LlmError::InvalidRequest(err.to_string()),
                Some("ResourceNotFoundException") => LlmError::ModelNotFound(err.to_string()),
                Some("AccessDeniedException") => LlmError::AuthenticationFailed(err.to_string()),
                Some("ThrottlingException") => LlmError::RateLimitExceeded {
                    message: err.to_string(),
                },
                Some("ServiceQuotaExceededException") => LlmError::InsufficientQuota(err.to_string()),
                Some("InternalServerException") => {
                    // Provider internal errors should pass through the message
                    LlmError::InternalError(Some(err.to_string()))
                }
                _ => LlmError::ConnectionError(err.to_string()),
            }
        } else {
            // For non-service errors (network, timeout, etc.), treat as connection error
            LlmError::ConnectionError(err.to_string())
        }
    }
}
