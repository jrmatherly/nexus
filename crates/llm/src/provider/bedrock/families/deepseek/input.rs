//! DeepSeek input types for AWS Bedrock.
//!
//! DeepSeek R1 is a reasoning model that supports chain-of-thought reasoning
//! and provides transparent reasoning traces in its responses.
//!
//! # Supported Models
//! - `deepseek.r1-v1:0`: DeepSeek R1 reasoning model
//!
//! # Model Characteristics
//! - **Input Format**: Simple prompt-based format
//! - **Reasoning**: Provides optional reasoning traces
//! - **Context Window**: Large context support
//! - **Streaming**: Supported via API endpoint
//!
//! # Official Documentation
//! - [DeepSeek Models on Bedrock](https://docs.aws.amazon.com/bedrock/latest/userguide/model-parameters-deepseek.html)

use crate::messages::{ChatCompletionRequest, ChatRole};
use serde::Serialize;

/// Request payload for DeepSeek models.
///
/// DeepSeek uses a simple prompt-based format similar to older models,
/// where messages are concatenated into a single prompt string.
///
/// # Request Format
/// ```json
/// {
///   "prompt": "User: Hello\nAssistant: Hi! How can I help?\nUser: What is 2+2?\nAssistant:",
///   "temperature": 0.7,
///   "top_p": 0.9,
///   "max_tokens": 100,
///   "stop": ["User:", "\n\n"]
/// }
/// ```
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct DeepSeekRequest {
    /// The prompt string with conversation history.
    pub prompt: String,

    /// Maximum number of tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Temperature for randomness (0.0-1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p nucleus sampling (0.0-1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Stop sequences to end generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

impl From<ChatCompletionRequest> for DeepSeekRequest {
    fn from(request: ChatCompletionRequest) -> Self {
        // Build the prompt by concatenating messages
        let mut prompt = String::new();
        let mut has_system = false;

        // First, handle system messages
        for msg in &request.messages {
            if let ChatRole::System = msg.role {
                if !has_system {
                    prompt.push_str("System: ");
                    has_system = true;
                } else {
                    prompt.push('\n');
                }
                prompt.push_str(&msg.content);
            }
        }

        if has_system {
            prompt.push_str("\n\n");
        }

        // Then handle conversation messages
        for msg in &request.messages {
            match msg.role {
                ChatRole::System => {
                    // Already handled above
                }
                ChatRole::User => {
                    prompt.push_str("User: ");
                    prompt.push_str(&msg.content);
                    prompt.push('\n');
                }
                ChatRole::Assistant => {
                    prompt.push_str("Assistant: ");
                    prompt.push_str(&msg.content);
                    prompt.push('\n');
                }
                ChatRole::Other(ref role) => {
                    log::warn!("Unknown role '{}' in DeepSeek request, mapping to User", role);
                    prompt.push_str("User: ");
                    prompt.push_str(&msg.content);
                    prompt.push('\n');
                }
            }
        }

        // Add the Assistant prefix for the response
        prompt.push_str("Assistant:");

        Self {
            prompt,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            stop: Some(vec!["User:".to_string(), "\n\n".to_string()]),
        }
    }
}
