//! Meta Llama input types for AWS Bedrock.
//!
//! This module contains request types for Meta's Llama models available through AWS Bedrock.
//! Llama models use a distinctive prompt format with special control tokens that must be
//! formatted correctly for optimal performance.
//!
//! # Supported Models
//! - `meta.llama3-70b-instruct-v1:0`: Large instruction-following model
//! - `meta.llama3-8b-instruct-v1:0`: Smaller instruction-following model
//! - `meta.llama2-70b-chat-v1`: Previous generation chat model
//! - `meta.llama2-13b-chat-v1`: Smaller previous generation model
//!
//! # Model Characteristics
//! - **Input Format**: Special control tokens with role-based headers
//! - **Context Window**: Up to 32,768 tokens (varies by model)
//! - **Languages**: Multilingual with strong English performance
//! - **Streaming**: Supported for real-time response generation
//!
//! # Prompt Format
//! Llama models use special control tokens for conversation structure:
//! ```text
//! <|begin_of_text|><|start_header_id|>system<|end_header_id|>
//!
//! {system_message}<|eot_id|><|start_header_id|>user<|end_header_id|>
//!
//! {user_message}<|eot_id|><|start_header_id|>assistant<|end_header_id|>
//!
//! ```
//!
//! # Official Documentation
//! - [Llama Models on Bedrock](https://docs.aws.amazon.com/bedrock/latest/userguide/llama-models.html)
//! - [Llama Prompt Engineering](https://docs.aws.amazon.com/bedrock/latest/userguide/prompt-engineering-llama.html)

use serde::Serialize;

use crate::messages::{ChatCompletionRequest, ChatRole};

/// Request payload for Meta Llama models.
///
/// Llama models require a specific prompt format using control tokens to structure
/// conversations. The prompt must include proper role headers and token boundaries
/// to ensure the model understands the conversation context correctly.
///
/// # Control Tokens
/// - `<|begin_of_text|>`: Marks the beginning of the input
/// - `<|start_header_id|>role<|end_header_id|>`: Role identifier (system/user/assistant)
/// - `<|eot_id|>`: End of turn marker
///
/// # Request Format
/// ```json
/// {
///   "prompt": "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\nYou are helpful.<|eot_id|><|start_header_id|>user<|end_header_id|>\n\nHello<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
///   "temperature": 0.7,
///   "top_p": 0.9,
///   "max_gen_len": 4096
/// }
/// ```
///
/// # Important Notes
/// - The prompt must end with the assistant header to prompt generation
/// - Proper token structure is critical for model performance
/// - System message is required (defaults to "You are a helpful assistant")
#[derive(Debug, Serialize)]
pub(crate) struct LlamaRequest {
    /// The formatted prompt with control tokens and conversation history.
    ///
    /// This field contains the complete conversation formatted using Llama's specific
    /// control token format. The prompt structure is critical for model performance:
    ///
    /// 1. Starts with `<|begin_of_text|>`
    /// 2. System message in `<|start_header_id|>system<|end_header_id|>` block
    /// 3. Each user/assistant message in appropriate header blocks
    /// 4. Ends with `<|start_header_id|>assistant<|end_header_id|>\n\n` to prompt response
    ///
    /// # Example Structure
    /// ```text
    /// <|begin_of_text|><|start_header_id|>system<|end_header_id|>
    ///
    /// You are a helpful assistant.<|eot_id|><|start_header_id|>user<|end_header_id|>
    ///
    /// What is machine learning?<|eot_id|><|start_header_id|>assistant<|end_header_id|>
    ///
    /// ```
    pub prompt: String,

    /// Controls randomness in token selection.
    ///
    /// Temperature parameter affects the probability distribution over tokens:
    /// - **0.0**: Deterministic, always selects most likely token
    /// - **0.1-0.3**: Conservative, focused responses
    /// - **0.4-0.7**: Balanced creativity and coherence
    /// - **0.8-1.0**: More creative, potentially less coherent
    ///
    /// Llama models typically perform well with temperatures in the 0.6-0.8 range
    /// for conversational tasks.
    ///
    /// # Range
    /// 0.0-1.0
    ///
    /// # Default
    /// Model default (typically ~0.6)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Nucleus sampling parameter for controlling diversity.
    ///
    /// Top-p sampling considers only tokens in the top percentile of the probability
    /// distribution:
    /// - **0.1**: Very focused, only highest probability tokens
    /// - **0.5**: Moderately focused selection
    /// - **0.9**: Allows diverse token selection (recommended)
    /// - **1.0**: All tokens considered (no filtering)
    ///
    /// Combined with temperature, this helps balance creativity with coherence.
    ///
    /// # Range
    /// 0.0-1.0
    ///
    /// # Default
    /// Model default (typically 0.9)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Maximum number of tokens to generate.
    ///
    /// Controls the length of the generated response. The model will stop when:
    /// 1. It reaches this token limit
    /// 2. It generates a natural ending
    /// 3. It encounters stopping criteria
    ///
    /// Different Llama models have different context limits:
    /// - Llama 3 70B: Up to 8,192 output tokens
    /// - Llama 3 8B: Up to 8,192 output tokens
    /// - Llama 2 models: Varies by specific model
    ///
    /// # Range
    /// 1-8192 (varies by model)
    ///
    /// # Default
    /// No default (model-specific behavior if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_gen_len: Option<u32>,
}

impl From<ChatCompletionRequest> for LlamaRequest {
    fn from(request: ChatCompletionRequest) -> Self {
        // Build prompt using Llama's specific header tag format
        let mut prompt = String::new();

        // Add system message if present
        let default_system = "You are a helpful assistant.";

        let system_msg = request
            .messages
            .iter()
            .find(|m| matches!(m.role, ChatRole::System))
            .map(|m| m.content.as_str())
            .unwrap_or(default_system);

        prompt.push_str(&format!(
            "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n{system_msg}<|eot_id|>"
        ));

        // Add conversation history
        for msg in request.messages {
            match msg.role {
                ChatRole::System => {} // Already handled
                ChatRole::User => {
                    prompt.push_str(&format!(
                        "<|start_header_id|>user<|end_header_id|>\n\n{content}<|eot_id|>",
                        content = msg.content
                    ));
                }
                ChatRole::Assistant => {
                    prompt.push_str(&format!(
                        "<|start_header_id|>assistant<|end_header_id|>\n\n{content}<|eot_id|>",
                        content = msg.content
                    ));
                }
                ChatRole::Other(role) => {
                    log::warn!("Unknown role {role} in Llama request, treating as user");
                    prompt.push_str(&format!(
                        "<|start_header_id|>user<|end_header_id|>\n\n{content}<|eot_id|>",
                        content = msg.content
                    ));
                }
            }
        }

        prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");

        Self {
            prompt,
            temperature: request.temperature,
            top_p: request.top_p,
            max_gen_len: request.max_tokens,
        }
    }
}
