//! AI21 input types for AWS Bedrock.
//!
//! AI21 Labs models on Bedrock include the Jamba family of models which support
//! structured messages similar to other modern chat models.
//!
//! # Supported Models
//! - `ai21.jamba-1-5-mini-v1:0`: Jamba 1.5 Mini - Efficient model for general tasks
//! - `ai21.jamba-1-5-large-v1:0`: Jamba 1.5 Large - High-performance model
//! - `ai21.jamba-instruct-v1:0`: Original Jamba Instruct model
//!
//! # Model Characteristics
//! - **Input Format**: OpenAI-compatible messages array
//! - **Context Window**: 256K tokens
//! - **Streaming**: Supported
//! - **JSON Mode**: Supports structured JSON output
//! - **Function Calling**: Supported
//!
//! # Official Documentation
//! - [AI21 Models on Bedrock](https://docs.aws.amazon.com/bedrock/latest/userguide/model-parameters-jamba.html)
//! - [Jamba API Reference](https://docs.ai21.com/reference/jamba-15-api-ref)

use crate::messages::{ChatCompletionRequest, ChatMessage, ChatRole};
use serde::Serialize;

/// Request payload for AI21 Jamba models.
///
/// Jamba uses an OpenAI-compatible message format with role-based messages.
///
/// # Request Format
/// ```json
/// {
///   "messages": [
///     {"role": "system", "content": "You are a helpful assistant."},
///     {"role": "user", "content": "Hello!"},
///     {"role": "assistant", "content": "Hi! How can I help you?"},
///     {"role": "user", "content": "What is 2+2?"}
///   ],
///   "temperature": 0.7,
///   "top_p": 0.9,
///   "max_tokens": 100,
///   "frequency_penalty": 0.0,
///   "presence_penalty": 0.0
/// }
/// ```
#[derive(Debug, Serialize)]
pub(crate) struct JambaRequest {
    /// Array of messages in the conversation.
    pub messages: Vec<JambaMessage>,

    /// Maximum number of tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Temperature for randomness (0.0-2.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p nucleus sampling (0.0-1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Frequency penalty to reduce repetition (-2.0 to 2.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Presence penalty to encourage diversity (-2.0 to 2.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// Stop sequences to end generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// Number of responses to generate (for non-streaming).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
}

/// A single message in the Jamba conversation.
#[derive(Debug, Serialize)]
pub(crate) struct JambaMessage {
    /// The role of the message author.
    pub role: String,

    /// The content of the message.
    pub content: String,
}

impl From<ChatMessage> for JambaMessage {
    fn from(msg: ChatMessage) -> Self {
        let role = match msg.role {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::Other(ref s) => s.as_str(),
        }
        .to_string();

        Self {
            role,
            content: msg.content.clone(),
        }
    }
}

impl From<ChatCompletionRequest> for JambaRequest {
    fn from(request: ChatCompletionRequest) -> Self {
        Self {
            messages: request.messages.into_iter().map(JambaMessage::from).collect(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            frequency_penalty: request.frequency_penalty,
            presence_penalty: request.presence_penalty,
            stop: request.stop,
            n: None, // ChatCompletionRequest doesn't have n field
        }
    }
}
