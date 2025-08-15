//! Mistral input types for AWS Bedrock.
//!
//! This module contains request types for Mistral AI models available through AWS Bedrock.
//! Mistral models use an instruction-based format with [INST] tags to structure conversations.
//!
//! # Supported Models
//! - `mistral.mistral-7b-instruct-v0:2`: Efficient instruction-following model
//! - `mistral.mixtral-8x7b-instruct-v0:1`: Mixture of experts model for complex tasks
//! - `mistral.mistral-small-2402-v1:0`: Optimized small model
//! - `mistral.mistral-large-2402-v1:0`: Large model for demanding applications
//!
//! # Model Characteristics
//! - **Input Format**: [INST] instruction tags with conversation flow
//! - **Context Window**: Up to 32,768 tokens (varies by model)
//! - **Languages**: Multilingual with strong performance in English and European languages
//! - **Streaming**: Fully supported with real-time token delivery
//!
//! # Prompt Format
//! Mistral models expect instruction tags:
//! ```text
//! [INST] System message and user input [/INST]
//! Assistant response
//! [INST] Next user input [/INST]
//! ```
//!
//! # Official Documentation
//! - [Mistral Models on Bedrock](https://docs.aws.amazon.com/bedrock/latest/userguide/mistral-models.html)

use serde::Serialize;

use crate::messages::{ChatCompletionRequest, ChatRole};

/// Request payload for Mistral AI models.
///
/// Mistral models use an instruction-based conversation format with [INST] tags
/// to clearly delineate user instructions from model responses. This format
/// optimizes the model's instruction-following capabilities.
///
/// # Request Format
/// ```json
/// {
///   "prompt": "[INST] You are helpful. Hello [/INST]\n",
///   "max_tokens": 4096,
///   "temperature": 0.7,
///   "top_p": 0.9,
///   "stream": false
/// }
/// ```
#[derive(Debug, Serialize)]
pub(crate) struct MistralRequest {
    /// Formatted prompt using Mistral's [INST] instruction format.
    ///
    /// The prompt combines system messages and user input within [INST] tags,
    /// with assistant responses following outside the tags. This structure
    /// helps the model understand when it should respond versus when it's
    /// processing instructions.
    pub prompt: String,

    /// Maximum tokens to generate (1-4096).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Sampling temperature (0.0-1.0).
    /// Lower values = more focused, higher = more creative.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Nucleus sampling threshold (0.0-1.0).
    /// Controls diversity of token selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Stop sequences to halt generation.
    /// Up to 4 sequences, each up to 20 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// Enable streaming response delivery.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

impl From<ChatCompletionRequest> for MistralRequest {
    fn from(request: ChatCompletionRequest) -> Self {
        // Build prompt using Mistral's [INST] instruction format
        let mut prompt = String::new();

        for msg in request.messages {
            match msg.role {
                ChatRole::System => {
                    prompt.push_str(&format!("[INST] {} [/INST]\n", msg.content));
                }
                ChatRole::User => {
                    prompt.push_str(&format!("[INST] {} [/INST]\n", msg.content));
                }
                ChatRole::Assistant => {
                    prompt.push_str(&format!("{}\n", msg.content));
                }
                ChatRole::Other(role) => {
                    log::warn!("Unknown role {role} in Mistral request, treating as user");
                    prompt.push_str(&format!("[INST] {} [/INST]\n", msg.content));
                }
            }
        }

        Self {
            prompt,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            stop: request.stop.clone(),
            stream: None,
        }
    }
}
