//! Model family detection and routing for AWS Bedrock models.
//!
//! AWS Bedrock hosts models from multiple vendors, each with different request/response
//! formats. This module provides utilities for detecting the model family from model IDs
//! and routing requests to the appropriate transformation logic.

// Declare family modules
pub mod ai21;
pub mod amazon;
pub mod anthropic;
pub mod cohere;
pub mod deepseek;
pub mod meta;
pub mod mistral;

use aws_sdk_bedrockruntime::primitives::Blob;
use std::str::FromStr;

use crate::{
    error::LlmError,
    messages::{ChatCompletionRequest, ChatCompletionResponse},
};

/// Represents the different model families available in AWS Bedrock.
///
/// Each family corresponds to a different vendor and has its own request/response format:
///
/// - **Anthropic**: Claude models (claude-3-opus, claude-3-sonnet, claude-3-haiku, etc.)
/// - **Amazon**: Titan models (titan-text-express, titan-embed-text, etc.)
/// - **Meta**: Llama models (llama3-70b-instruct, llama2-70b-chat, etc.)
/// - **Mistral**: Mistral and Mixtral models (mistral-7b-instruct, mixtral-8x7b, etc.)
/// - **Cohere**: Command and Embed models (command-text, command-light-text, etc.)
/// - **AI21**: Jurassic and Jamba models (j2-ultra, jamba-instruct, etc.)
/// - **Stability**: Stable Diffusion models (stable-diffusion-xl, etc.) - Image generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFamily {
    /// Anthropic models (Claude family)
    /// Model IDs: `anthropic.claude-*`
    Anthropic,

    /// Amazon Titan models (legacy text generation)
    /// Model IDs: `amazon.titan-*`
    AmazonTitan,

    /// Amazon Nova models (new generation with messages format)
    /// Model IDs: `amazon.nova-*`
    AmazonNova,

    /// Meta models (Llama family)
    /// Model IDs: `meta.llama*`
    Meta,

    /// Mistral AI models (Mistral and Mixtral families)
    /// Model IDs: `mistral.mistral-*`, `mistral.mixtral-*`
    Mistral,

    /// Cohere Command-R models
    /// Model IDs: `cohere.command-r-*`, `cohere.command-r-plus-*`
    /// Uses structured message/chat_history format
    CohereCommandR,

    /// AI21 Labs models (Jurassic and Jamba families)
    /// Model IDs: `ai21.j2-*`, `ai21.jamba-*`
    AI21,

    /// DeepSeek models (R1 reasoning model)
    /// Model IDs: `deepseek.r1-*`
    DeepSeek,

    /// Stability AI models (Stable Diffusion family)
    /// Model IDs: `stability.stable-diffusion-*`
    /// Note: These are image generation models, not text completion
    Stability,
}

impl ModelFamily {
    /// Get the vendor prefix for this model family.
    ///
    /// # Examples
    /// ```
    /// use llm::provider::bedrock::ModelFamily;
    ///
    /// assert_eq!(ModelFamily::Anthropic.vendor_prefix(), "anthropic");
    /// assert_eq!(ModelFamily::Amazon.vendor_prefix(), "amazon");
    /// ```
    pub fn vendor_prefix(&self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::AmazonTitan => "amazon",
            Self::AmazonNova => "amazon",
            Self::Meta => "meta",
            Self::Mistral => "mistral",
            Self::CohereCommandR => "cohere",
            Self::AI21 => "ai21",
            Self::DeepSeek => "deepseek",
            Self::Stability => "stability",
        }
    }

    /// Check if this model family supports streaming responses.
    pub fn supports_streaming(&self) -> bool {
        match self {
            Self::Anthropic => true,
            Self::AmazonTitan => true, // Note: Only Text models, not Embed models
            Self::AmazonNova => true,
            Self::Meta => true,
            Self::Mistral => true,
            Self::CohereCommandR => true,
            Self::AI21 => true,       // Jamba models support streaming
            Self::DeepSeek => true,   // DeepSeek R1 supports streaming
            Self::Stability => false, // Image generation doesn't use text streaming
        }
    }

    /// Get a human-readable description of this model family.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic Claude models",
            Self::AmazonTitan => "Amazon Titan models (legacy)",
            Self::AmazonNova => "Amazon Nova models",
            Self::Meta => "Meta Llama models",
            Self::Mistral => "Mistral AI models",
            Self::CohereCommandR => "Cohere Command-R models",
            Self::AI21 => "AI21 Labs Jurassic and Jamba models",
            Self::DeepSeek => "DeepSeek R1 reasoning models",
            Self::Stability => "Stability AI Stable Diffusion models",
        }
    }

    /// Transform a unified chat completion request to the appropriate vendor format for Bedrock.
    /// Note: For Anthropic models, the request.model field should already contain the resolved Bedrock model ID.
    pub(crate) fn transform_request(&self, request: ChatCompletionRequest) -> crate::Result<Blob> {
        use amazon::titan::input::TitanRequest;
        use anthropic::input::BedrockAnthropicRequest;
        use meta::input::LlamaRequest;
        use mistral::input::MistralRequest;

        match self {
            Self::Anthropic => {
                // Use Bedrock-specific Anthropic request format
                // This omits the model field and adds anthropic_version
                let req = BedrockAnthropicRequest::from(request);
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize Anthropic request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::AmazonTitan => {
                let req = TitanRequest::from(request);
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize Titan request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::AmazonNova => {
                use amazon::nova::input::NovaRequest;
                let req = NovaRequest::from(request);
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize Nova request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::Meta => {
                let req = LlamaRequest::from(request);
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize Llama request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::Mistral => {
                let req = MistralRequest::from(request);
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize Mistral request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::CohereCommandR => {
                use cohere::input::CohereCommandRRequest;
                let req = CohereCommandRRequest::from(request);
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize Cohere Command-R request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::DeepSeek => {
                use deepseek::input::DeepSeekRequest;

                let req = DeepSeekRequest::from(request);

                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize DeepSeek request: {}", e);
                    LlmError::InternalError(None)
                })?;

                Ok(Blob::new(json_bytes))
            }
            Self::AI21 => {
                use ai21::JambaRequest;

                let req = JambaRequest::from(request);

                let json_str = sonic_rs::to_string(&req).map_err(|e| {
                    log::error!("Failed to serialize AI21 request: {}", e);
                    LlmError::InternalError(None)
                })?;

                log::debug!("AI21 request JSON: {}", json_str);
                let json_bytes = json_str.into_bytes();

                Ok(Blob::new(json_bytes))
            }
            Self::Stability => Err(LlmError::InvalidRequest(
                "Stability AI models are for image generation, not chat completion".to_string(),
            )),
        }
    }

    /// Transform a unified chat completion request for streaming to the appropriate vendor format for Bedrock.
    /// This enables streaming-specific settings for each model family.
    pub(crate) fn transform_streaming_request(&self, request: ChatCompletionRequest) -> crate::Result<Blob> {
        use amazon::titan::input::TitanRequest;
        use anthropic::input::BedrockAnthropicRequest;
        use meta::input::LlamaRequest;
        use mistral::input::MistralRequest;

        match self {
            Self::Anthropic => {
                // Anthropic doesn't use a stream field - streaming is controlled by endpoint
                // Use Bedrock-specific format (no model field, has anthropic_version)
                let req = BedrockAnthropicRequest::from(request);
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize Anthropic streaming request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::AmazonTitan => {
                let req = TitanRequest::from(request);
                // Titan streaming is controlled by API endpoint, not request body
                // Do NOT set stream=true as it's not permitted in the request
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize Titan streaming request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::AmazonNova => {
                use amazon::nova::input::NovaRequest;
                let req = NovaRequest::from(request);
                // Nova streaming is controlled by API endpoint, not request body
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize Nova streaming request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::Meta => {
                // Llama doesn't use an explicit stream field - controlled by endpoint
                let req = LlamaRequest::from(request);
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize Llama streaming request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::Mistral => {
                let mut req = MistralRequest::from(request);
                // Enable streaming for Mistral
                req.stream = Some(true);
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize Mistral streaming request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::CohereCommandR => {
                use cohere::input::CohereCommandRRequest;
                let req = CohereCommandRRequest::from(request);
                // Command-R streaming is controlled by API endpoint, not request body
                // Do NOT set stream field as AWS Bedrock doesn't accept it
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize Cohere Command-R streaming request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::DeepSeek => {
                use deepseek::input::DeepSeekRequest;
                let req = DeepSeekRequest::from(request);
                // DeepSeek streaming is controlled by API endpoint, not request body
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize DeepSeek streaming request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::AI21 => {
                use ai21::JambaRequest;
                let req = JambaRequest::from(request);
                let json_bytes = sonic_rs::to_vec(&req).map_err(|e| {
                    log::error!("Failed to serialize AI21 streaming request: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(Blob::new(json_bytes))
            }
            Self::Stability => Err(LlmError::InvalidRequest(
                "Stability AI models are for image generation, not chat completion".to_string(),
            )),
        }
    }

    /// Parse a streaming chunk and convert to ChatCompletionChunk.
    /// The caller should set the id, created, and model fields after calling this.
    pub(crate) fn parse_stream_chunk(&self, json_data: &[u8]) -> Option<crate::messages::ChatCompletionChunk> {
        use amazon::titan::output::TitanStreamChunk;
        use anthropic::output::AnthropicStreamChunk;
        use cohere::output::CohereStreamChunk;
        use meta::output::LlamaStreamChunk;
        use mistral::output::MistralStreamChunk;

        match self {
            Self::Anthropic => match sonic_rs::from_slice::<AnthropicStreamChunk>(json_data) {
                Ok(chunk) => chunk.into(),
                Err(e) => {
                    log::error!(
                        "Failed to parse Anthropic stream chunk: {e}, data: {}",
                        String::from_utf8_lossy(json_data)
                    );
                    None
                }
            },
            Self::AmazonTitan => match sonic_rs::from_slice::<TitanStreamChunk>(json_data) {
                Ok(chunk) => chunk.into(),
                Err(e) => {
                    log::error!(
                        "Failed to parse Titan chunk: {e}, data: {}",
                        String::from_utf8_lossy(json_data)
                    );
                    None
                }
            },
            Self::AmazonNova => {
                use amazon::nova::output::NovaStreamChunk;
                match sonic_rs::from_slice::<NovaStreamChunk>(json_data) {
                    Ok(chunk) => chunk.into(),
                    Err(e) => {
                        log::debug!("Failed to parse Nova stream chunk: {e}");
                        None
                    }
                }
            }
            Self::Meta => match sonic_rs::from_slice::<LlamaStreamChunk>(json_data) {
                Ok(chunk) => chunk.into(),
                Err(e) => {
                    log::error!(
                        "Failed to parse Meta Llama stream chunk: {e}, data: {}",
                        String::from_utf8_lossy(json_data)
                    );
                    None
                }
            },
            Self::Mistral => match sonic_rs::from_slice::<MistralStreamChunk>(json_data) {
                Ok(chunk) => chunk.into(),
                Err(e) => {
                    log::error!(
                        "Failed to parse Mistral stream chunk: {e}, data: {}",
                        String::from_utf8_lossy(json_data)
                    );
                    None
                }
            },
            Self::CohereCommandR => match sonic_rs::from_slice::<CohereStreamChunk>(json_data) {
                Ok(chunk) => chunk.into(),
                Err(e) => {
                    log::error!(
                        "Failed to parse Cohere Command-R stream chunk: {e}, data: {}",
                        String::from_utf8_lossy(json_data)
                    );
                    None
                }
            },
            Self::DeepSeek => {
                use deepseek::output::DeepSeekStreamChunk;
                match sonic_rs::from_slice::<DeepSeekStreamChunk>(json_data) {
                    Ok(chunk) => chunk.into(),
                    Err(e) => {
                        log::error!(
                            "Failed to parse DeepSeek stream chunk: {e}, data: {}",
                            String::from_utf8_lossy(json_data)
                        );
                        None
                    }
                }
            }
            Self::AI21 => {
                use ai21::AI21StreamEvent;
                match sonic_rs::from_slice::<AI21StreamEvent>(json_data) {
                    Ok(event) => event.into(),
                    Err(e) => {
                        log::error!(
                            "Failed to parse AI21 stream chunk: {e}, data: {}",
                            String::from_utf8_lossy(json_data)
                        );
                        None
                    }
                }
            }
            _ => None, // Stability doesn't support streaming
        }
    }

    /// Transform a Bedrock response back to the unified format.
    /// The caller should set the model name in the response after calling this.
    pub(crate) fn transform_response(&self, response_body: &[u8]) -> crate::Result<ChatCompletionResponse> {
        use crate::provider::anthropic::output::AnthropicResponse;
        use amazon::titan::output::TitanResponse;
        use meta::output::LlamaResponse;
        use mistral::output::MistralResponse;

        match self {
            Self::Anthropic => {
                let response: AnthropicResponse = sonic_rs::from_slice(response_body).map_err(|e| {
                    log::error!("Failed to deserialize Anthropic response: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(ChatCompletionResponse::from(response))
            }
            Self::AmazonTitan => {
                let response: TitanResponse = sonic_rs::from_slice(response_body).map_err(|e| {
                    log::error!("Failed to deserialize Titan response: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(ChatCompletionResponse::from(response))
            }
            Self::AmazonNova => {
                use amazon::nova::output::NovaResponse;
                let response: NovaResponse = sonic_rs::from_slice(response_body).map_err(|e| {
                    log::error!("Failed to deserialize Nova response: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(ChatCompletionResponse::from(response))
            }
            Self::Meta => {
                let response: LlamaResponse = sonic_rs::from_slice(response_body).map_err(|e| {
                    log::error!("Failed to deserialize Llama response: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(ChatCompletionResponse::from(response))
            }
            Self::Mistral => {
                let response: MistralResponse = sonic_rs::from_slice(response_body).map_err(|e| {
                    log::error!("Failed to deserialize Mistral response: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(ChatCompletionResponse::from(response))
            }
            Self::CohereCommandR => {
                use cohere::output::CohereCommandRResponse;
                let response: CohereCommandRResponse = sonic_rs::from_slice(response_body).map_err(|e| {
                    log::error!("Failed to deserialize Cohere Command-R response: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(ChatCompletionResponse::from(response))
            }
            Self::DeepSeek => {
                use deepseek::output::DeepSeekResponse;
                let response: DeepSeekResponse = sonic_rs::from_slice(response_body).map_err(|e| {
                    log::error!("Failed to deserialize DeepSeek response: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(ChatCompletionResponse::from(response))
            }
            Self::AI21 => {
                use ai21::JambaResponse;
                let response: JambaResponse = sonic_rs::from_slice(response_body).map_err(|e| {
                    log::error!("Failed to deserialize AI21 response: {}", e);
                    LlmError::InternalError(None)
                })?;
                Ok(ChatCompletionResponse::from(response))
            }
            Self::Stability => Err(LlmError::InvalidRequest(
                "Stability AI models are for image generation, not chat completion".to_string(),
            )),
        }
    }
}

impl std::fmt::Display for ModelFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.vendor_prefix())
    }
}

impl FromStr for ModelFamily {
    type Err = LlmError;

    /// Parse a model family from a Bedrock model ID.
    ///
    /// AWS Bedrock model IDs follow the format: `<vendor>.<model-name>-<version>`
    ///
    /// # Examples
    /// ```ignore
    /// use llm::provider::bedrock::ModelFamily;
    /// use std::str::FromStr;
    ///
    /// assert_eq!(
    ///     ModelFamily::from_str("anthropic.claude-3-sonnet-20240229-v1:0").unwrap(),
    ///     ModelFamily::Anthropic
    /// );
    /// assert_eq!(
    ///     ModelFamily::from_str("amazon.titan-text-express-v1").unwrap(),
    ///     ModelFamily::Amazon
    /// );
    /// assert_eq!(
    ///     ModelFamily::from_str("meta.llama3-70b-instruct-v1:0").unwrap(),
    ///     ModelFamily::Meta
    /// );
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - The model ID doesn't contain a dot (invalid format)
    /// - The vendor prefix is not recognized
    fn from_str(model_id: &str) -> Result<Self, Self::Err> {
        // Handle inference profiles (e.g., us.deepseek.r1-v1:0)
        // These have region prefixes before the vendor name
        let effective_model_id = if model_id.contains('.') && model_id.split('.').count() >= 3 {
            // Check if this looks like an inference profile (region.vendor.model)
            let parts: Vec<&str> = model_id.splitn(2, '.').collect();
            if parts.len() == 2 {
                let potential_region = parts[0];
                let rest = parts[1];

                // Common AWS region prefixes for inference profiles
                if matches!(potential_region, "us" | "eu" | "ap" | "ca" | "sa" | "me" | "af") {
                    // This looks like an inference profile, use the part after the region
                    rest
                } else {
                    // Not an inference profile, use as-is
                    model_id
                }
            } else {
                model_id
            }
        } else {
            model_id
        };

        if !effective_model_id.contains('.') {
            return Err(LlmError::InvalidModelFormat(format!(
                "Invalid model ID format: '{}' (missing vendor prefix)",
                model_id
            )));
        }

        let vendor = effective_model_id
            .split('.')
            .next()
            .expect("split always returns at least one element");

        // For Cohere, we only support Command-R models
        if vendor == "cohere" {
            // Get the model name after the vendor prefix
            let model_name = effective_model_id
                .strip_prefix("cohere.")
                .ok_or_else(|| LlmError::InvalidModelFormat(format!("Invalid Cohere model ID: '{}'", model_id)))?;

            // Check if it's a Command-R model (including command-r-plus)
            if model_name.starts_with("command-r") {
                return Ok(Self::CohereCommandR);
            } else {
                return Err(LlmError::InvalidModelFormat(format!(
                    "Unsupported Cohere model: '{}'. Only Command-R models are supported",
                    model_id
                )));
            }
        }

        let family = match vendor {
            "anthropic" => Self::Anthropic,
            "amazon" => {
                // Check if it's a Nova or Titan model
                let model_name = effective_model_id
                    .strip_prefix("amazon.")
                    .ok_or_else(|| LlmError::InvalidModelFormat(format!("Invalid Amazon model ID: '{}'", model_id)))?;

                if model_name.starts_with("nova-") {
                    Self::AmazonNova
                } else if model_name.starts_with("titan-") {
                    Self::AmazonTitan
                } else {
                    return Err(LlmError::InvalidModelFormat(format!(
                        "Unknown Amazon model type: '{}'. Expected 'nova-*' or 'titan-*'",
                        model_name
                    )));
                }
            }
            "meta" => Self::Meta,
            "mistral" => Self::Mistral,
            "deepseek" => Self::DeepSeek,
            "ai21" => Self::AI21,
            "stability" => Self::Stability,
            _ => {
                return Err(LlmError::InvalidModelFormat(format!(
                    "Unknown model family for vendor: '{}'. Supported vendors: anthropic, amazon, meta, mistral, cohere, deepseek. Recognized but not yet implemented: ai21, stability",
                    vendor
                )));
            }
        };

        Ok(family)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_model_detection() {
        assert_eq!(
            "anthropic.claude-3-opus-20240229-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::Anthropic
        );
        assert_eq!(
            "anthropic.claude-3-sonnet-20240229-v1:0"
                .parse::<ModelFamily>()
                .unwrap(),
            ModelFamily::Anthropic
        );
        assert_eq!(
            "anthropic.claude-3-haiku-20240307-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::Anthropic
        );
        assert_eq!(
            "anthropic.claude-instant-v1".parse::<ModelFamily>().unwrap(),
            ModelFamily::Anthropic
        );
    }

    #[test]
    fn amazon_titan_model_detection() {
        assert_eq!(
            "amazon.titan-text-express-v1".parse::<ModelFamily>().unwrap(),
            ModelFamily::AmazonTitan
        );
        assert_eq!(
            "amazon.titan-text-lite-v1".parse::<ModelFamily>().unwrap(),
            ModelFamily::AmazonTitan
        );
        assert_eq!(
            "amazon.titan-embed-text-v1".parse::<ModelFamily>().unwrap(),
            ModelFamily::AmazonTitan
        );
    }

    #[test]
    fn amazon_nova_model_detection() {
        assert_eq!(
            "amazon.nova-micro-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::AmazonNova
        );
        assert_eq!(
            "amazon.nova-lite-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::AmazonNova
        );
        assert_eq!(
            "amazon.nova-pro-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::AmazonNova
        );
    }

    #[test]
    fn meta_model_detection() {
        assert_eq!(
            "meta.llama3-70b-instruct-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::Meta
        );
        assert_eq!(
            "meta.llama2-70b-chat-v1".parse::<ModelFamily>().unwrap(),
            ModelFamily::Meta
        );
        assert_eq!(
            "meta.llama3-8b-instruct-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::Meta
        );
    }

    #[test]
    fn mistral_model_detection() {
        assert_eq!(
            "mistral.mistral-7b-instruct-v0:2".parse::<ModelFamily>().unwrap(),
            ModelFamily::Mistral
        );
        assert_eq!(
            "mistral.mixtral-8x7b-instruct-v0:1".parse::<ModelFamily>().unwrap(),
            ModelFamily::Mistral
        );
    }

    #[test]
    fn cohere_model_detection() {
        // Command-R models are supported
        assert_eq!(
            "cohere.command-r-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::CohereCommandR
        );
        assert_eq!(
            "cohere.command-r-plus-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::CohereCommandR
        );

        // Old Command models are not supported
        assert!("cohere.command-text-v14".parse::<ModelFamily>().is_err());
        assert!("cohere.command-light-text-v14".parse::<ModelFamily>().is_err());

        // Embed models are not supported
        assert!("cohere.embed-english-v3".parse::<ModelFamily>().is_err());
        assert!("cohere.embed-multilingual-v3".parse::<ModelFamily>().is_err());
    }

    #[test]
    fn deepseek_model_detection() {
        assert_eq!(
            "deepseek.r1-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::DeepSeek
        );
        assert_eq!(
            "deepseek.r1-distill-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::DeepSeek
        );
        // Test inference profile with regional prefix
        assert_eq!(
            "us.deepseek.r1-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::DeepSeek
        );
        assert_eq!(
            "eu.deepseek.r1-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::DeepSeek
        );
    }

    #[test]
    fn ai21_model_detection() {
        assert_eq!("ai21.j2-ultra-v1".parse::<ModelFamily>().unwrap(), ModelFamily::AI21);
        assert_eq!("ai21.j2-mid-v1".parse::<ModelFamily>().unwrap(), ModelFamily::AI21);
        assert_eq!(
            "ai21.jamba-instruct-v1:0".parse::<ModelFamily>().unwrap(),
            ModelFamily::AI21
        );
    }

    #[test]
    fn stability_model_detection() {
        assert_eq!(
            "stability.stable-diffusion-xl-v1".parse::<ModelFamily>().unwrap(),
            ModelFamily::Stability
        );
    }

    #[test]
    fn unknown_vendor() {
        let result = "unknown.model-v1".parse::<ModelFamily>();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown model family"));
    }

    #[test]
    fn invalid_format() {
        let result = "no-dot-in-model-id".parse::<ModelFamily>();
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        println!("Actual error message: {}", error_msg);
        assert!(error_msg.contains("Invalid model ID format"));
    }

    #[test]
    fn streaming_support() {
        assert!(ModelFamily::Anthropic.supports_streaming());
        assert!(ModelFamily::AmazonTitan.supports_streaming());
        assert!(ModelFamily::AmazonNova.supports_streaming());
        assert!(ModelFamily::Meta.supports_streaming());
        assert!(ModelFamily::Mistral.supports_streaming());
        assert!(ModelFamily::CohereCommandR.supports_streaming());
        assert!(ModelFamily::DeepSeek.supports_streaming());
        assert!(ModelFamily::AI21.supports_streaming());
        assert!(!ModelFamily::Stability.supports_streaming());
    }

    #[test]
    fn vendor_prefix() {
        assert_eq!(ModelFamily::Anthropic.vendor_prefix(), "anthropic");
        assert_eq!(ModelFamily::AmazonTitan.vendor_prefix(), "amazon");
        assert_eq!(ModelFamily::AmazonNova.vendor_prefix(), "amazon");
        assert_eq!(ModelFamily::Meta.vendor_prefix(), "meta");
        assert_eq!(ModelFamily::Mistral.vendor_prefix(), "mistral");
        assert_eq!(ModelFamily::CohereCommandR.vendor_prefix(), "cohere");
        assert_eq!(ModelFamily::DeepSeek.vendor_prefix(), "deepseek");
        assert_eq!(ModelFamily::AI21.vendor_prefix(), "ai21");
        assert_eq!(ModelFamily::Stability.vendor_prefix(), "stability");
    }

    #[test]
    fn display() {
        assert_eq!(ModelFamily::Anthropic.to_string(), "anthropic");
        assert_eq!(ModelFamily::AmazonTitan.to_string(), "amazon");
        assert_eq!(ModelFamily::AmazonNova.to_string(), "amazon");
        assert_eq!(ModelFamily::Meta.to_string(), "meta");
    }
}
