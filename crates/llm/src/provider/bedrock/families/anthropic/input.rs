//! Anthropic input types for AWS Bedrock.
//!
//! This module provides request types specifically for Anthropic Claude models on AWS Bedrock.
//! Bedrock's Anthropic models require a slightly different format than the direct Anthropic API,
//! particularly requiring the `anthropic_version` field and omitting the `model` field.
//!
//! # Supported Models
//! - `anthropic.claude-3-opus-20240229-v1:0`: Most capable model for complex tasks
//! - `anthropic.claude-3-sonnet-20240229-v1:0`: Balanced performance and speed
//! - `anthropic.claude-3-haiku-20240307-v1:0`: Fastest model for simple tasks
//! - `anthropic.claude-instant-v1`: Previous generation, optimized for speed
//! - `anthropic.claude-v2:1`: Previous generation general-purpose model
//!
//! # Model Characteristics
//! - **Input Format**: Messages array with system prompts
//! - **Context Window**: Up to 200,000 tokens (Claude 3) or 100,000 tokens (Claude 2)
//! - **Languages**: Excellent multilingual capabilities
//! - **Streaming**: Controlled by API endpoint, not request parameters
//! - **Safety**: Advanced constitutional AI safety measures
//!
//! # Key Differences from Direct API
//! - Model ID resolution: Uses Bedrock model IDs instead of Anthropic model names
//! - No `model` field: Model is specified in the API call, not the request body
//! - Required `anthropic_version`: Must include the API version field
//! - Streaming control: Managed by API method selection, not request body
//! - Authentication: Uses AWS credentials instead of Anthropic API keys
//!
//! # Request Format
//! ```json
//! {
//!   "anthropic_version": "bedrock-2023-05-31",
//!   "max_tokens": 4096,
//!   "messages": [
//!     {"role": "user", "content": "Hello!"}
//!   ],
//!   "system": "You are a helpful assistant"
//! }
//! ```
//!
//! # Official Documentation
//! - [Claude Models on Bedrock](https://docs.aws.amazon.com/bedrock/latest/userguide/claude-models.html)
//! - [Anthropic API Reference](https://docs.anthropic.com/en/api/messages)

use crate::messages::{ChatCompletionRequest, ChatMessage, ChatRole};
use serde::Serialize;

/// Request payload for Anthropic Claude models on AWS Bedrock.
///
/// This is similar to the standard Anthropic request but with Bedrock-specific requirements:
/// - Must include `anthropic_version` field
/// - Must NOT include `model` field (model is specified in the API call)
/// - Streaming is controlled by the API endpoint, not the request body
#[derive(Debug, Serialize)]
pub struct BedrockAnthropicRequest {
    /// The Anthropic API version to use.
    /// Required by Bedrock. Use "bedrock-2023-05-31" for compatibility.
    pub anthropic_version: String,

    /// The maximum number of tokens to generate before stopping.
    pub max_tokens: u32,

    /// Input messages for the conversation.
    /// Messages must alternate between user and assistant roles.
    pub messages: Vec<ChatMessage>,

    /// System prompt that sets the behavior of the assistant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,

    /// Amount of randomness injected into the response.
    /// Ranges from 0.0 to 1.0. Defaults to 1.0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Use nucleus sampling to limit the cumulative probability of tokens.
    /// Ranges from 0.0 to 1.0. Defaults to 0.999.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Only sample from the top K options for each subsequent token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,

    /// Custom text sequences that will cause the model to stop generating.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

impl From<ChatCompletionRequest> for BedrockAnthropicRequest {
    fn from(request: ChatCompletionRequest) -> Self {
        // Separate system messages from user/assistant messages
        let mut system_prompt = None;
        let mut messages = Vec::new();

        for msg in request.messages {
            match &msg.role {
                ChatRole::System => {
                    // Concatenate multiple system messages if present
                    if let Some(existing) = system_prompt {
                        system_prompt = Some(format!("{}\n{}", existing, msg.content));
                    } else {
                        system_prompt = Some(msg.content.clone());
                    }
                }
                _ => messages.push(msg),
            }
        }

        Self {
            anthropic_version: "bedrock-2023-05-31".to_string(),
            max_tokens: request.max_tokens.unwrap_or(4096),
            messages,
            system: system_prompt,
            temperature: request.temperature,
            top_p: request.top_p,
            top_k: None, // top_k is not directly mapped from OpenAI's n parameter
            stop_sequences: request.stop,
        }
    }
}
