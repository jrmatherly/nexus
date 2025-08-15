//! Amazon Titan input types for AWS Bedrock.
//!
//! This module contains request types for Amazon Titan models available through AWS Bedrock.
//! Titan models are Amazon's family of foundation models optimized for text generation tasks.
//!
//! # Supported Models
//! - `amazon.titan-text-express-v1`: Fast, cost-effective text generation
//! - `amazon.titan-text-lite-v1`: Lightweight model for simple tasks
//! - `amazon.titan-embed-text-v1`: Text embeddings model (not supported for chat completion)
//!
//! # Model Characteristics
//! - **Input Format**: Single text prompt with role-based prefixes
//! - **Context Window**: Up to 32,768 tokens (varies by model)
//! - **Languages**: Primarily English, with some multilingual capabilities
//! - **Streaming**: Supported via the `stream` parameter
//!
//! # Official Documentation
//! - [Amazon Titan Text Models](https://docs.aws.amazon.com/bedrock/latest/userguide/titan-text-models.html)
//! - [Titan Text API Reference](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_InvokeModel.html)

use serde::Serialize;

use crate::messages::{ChatCompletionRequest, ChatRole};

/// Request payload for Amazon Titan text generation models.
///
/// Titan models expect a simplified request format with a single `inputText` field
/// containing the complete prompt and a `textGenerationConfig` object for parameters.
/// Unlike chat-based models, Titan processes all conversation history as a single
/// concatenated text prompt with role prefixes.
///
/// # Request Format
/// ```json
/// {
///   "inputText": "System: You are helpful.\nUser: Hello\nAssistant: ",
///   "textGenerationConfig": {
///     "maxTokenCount": 4096,
///     "temperature": 0.7,
///     "topP": 0.9,
///     "stopSequences": ["Human:", "AI:"]
///   }
/// }
/// ```
///
/// # Prompt Format
/// Titan expects role-based prefixes in the input text:
/// - `System: {content}` for system messages
/// - `User: {content}` for user messages
/// - `Assistant: {content}` for assistant messages
/// - Ends with `Assistant: ` to prompt the model to respond
///
/// This format helps the model understand conversation context and respond appropriately.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TitanRequest {
    /// The complete input prompt including all conversation history.
    ///
    /// This field contains the entire conversation formatted as a single string
    /// with role prefixes (e.g., "System: ...\nUser: ...\nAssistant: ").
    /// The prompt should end with "Assistant: " to indicate where the model
    /// should begin its response.
    ///
    /// # Example
    /// ```text
    /// System: You are a helpful assistant.
    /// User: What is the capital of France?
    /// Assistant:
    /// ```
    pub input_text: String,

    /// Configuration parameters for text generation.
    ///
    /// This object contains all the parameters that control how Titan generates
    /// text, including token limits, randomness controls, and streaming options.
    pub text_generation_config: TitanTextGenerationConfig,
}

/// Configuration parameters for Amazon Titan text generation.
///
/// These parameters control various aspects of the text generation process,
/// including output length, randomness, and stopping conditions. All parameters
/// except `maxTokenCount` are optional and will use model defaults if not specified.
///
/// # Parameter Ranges
/// - `maxTokenCount`: 1-4096 (varies by model)
/// - `temperature`: 0.0-1.0 (higher = more random)
/// - `topP`: 0.0-1.0 (nucleus sampling threshold)
/// - `stopSequences`: Up to 4 strings, each up to 20 characters
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TitanTextGenerationConfig {
    /// Maximum number of tokens to generate in the response.
    ///
    /// This is the only required parameter. The model will stop generating
    /// once it reaches this limit, encounters a stop sequence, or naturally
    /// completes its response. Different Titan models have different maximum
    /// token limits:
    /// - Titan Text Express: Up to 8,000 tokens
    /// - Titan Text Lite: Up to 4,000 tokens
    ///
    /// # Range
    /// 1-4096 (actual maximum depends on the specific model)
    pub max_token_count: u32,

    /// Controls randomness in the generated text.
    ///
    /// Temperature affects the probability distribution over possible next tokens:
    /// - **0.0**: Deterministic output (always picks the most likely token)
    /// - **0.1-0.3**: Conservative, focused responses
    /// - **0.4-0.7**: Balanced creativity and coherence (recommended range)
    /// - **0.8-1.0**: More creative but potentially less coherent
    ///
    /// # Range
    /// 0.0-1.0
    ///
    /// # Default
    /// Model-specific default (typically around 0.7)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Controls diversity via nucleus sampling.
    ///
    /// Top-p sampling considers only the top tokens whose cumulative probability
    /// mass exceeds the threshold:
    /// - **0.1**: Very focused, only the most likely tokens
    /// - **0.5**: Moderately focused
    /// - **0.9**: Allows for more diverse token selection (recommended)
    /// - **1.0**: Considers all tokens (equivalent to no top-p filtering)
    ///
    /// Works together with temperature to control output randomness.
    ///
    /// # Range
    /// 0.0-1.0
    ///
    /// # Default
    /// Model-specific default (typically 0.9)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// List of strings that will stop text generation when encountered.
    ///
    /// When the model generates any of these sequences, it will immediately
    /// stop generating and return the response up to that point. The stop
    /// sequence itself is not included in the response.
    ///
    /// # Common Use Cases
    /// - Conversation markers: `["Human:", "AI:", "User:", "Assistant:"]`
    /// - Formatting markers: `["\n\n", "---", "###"]`
    /// - Code block endings: `["```", "</code>"]`
    ///
    /// # Constraints
    /// - Maximum of 4 stop sequences
    /// - Each sequence can be up to 20 characters long
    /// - Case-sensitive matching
    ///
    /// # Default
    /// No stop sequences (model will generate until max tokens or natural completion)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    /// Enable streaming response delivery.
    ///
    /// When set to `true`, the model will return partial results as they are generated,
    /// allowing for real-time display of the response. This is useful for long responses
    /// or when you want to show progress to users.
    ///
    /// # Streaming Behavior
    /// - Response is delivered as a series of server-sent events (SSE)
    /// - Each event contains a partial response chunk
    /// - Final event indicates completion and may include usage statistics
    /// - Network errors during streaming may result in incomplete responses
    ///
    /// # Default
    /// `false` (return complete response in a single payload)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

impl From<ChatCompletionRequest> for TitanRequest {
    fn from(request: ChatCompletionRequest) -> Self {
        // Concatenate all messages into a single prompt with role prefixes
        let mut prompt = String::new();

        for msg in request.messages {
            match msg.role {
                ChatRole::System => {
                    prompt.push_str(&format!("System: {}\n", msg.content));
                }
                ChatRole::User => {
                    prompt.push_str(&format!("User: {}\n", msg.content));
                }
                ChatRole::Assistant => {
                    prompt.push_str(&format!("Assistant: {}\n", msg.content));
                }
                ChatRole::Other(role) => {
                    prompt.push_str(&format!("{}: {}\n", role, msg.content));
                }
            }
        }

        prompt.push_str("Assistant: ");

        Self {
            input_text: prompt,
            text_generation_config: TitanTextGenerationConfig {
                max_token_count: request.max_tokens.unwrap_or(4096),
                temperature: request.temperature,
                top_p: request.top_p,
                stop_sequences: request.stop.clone(),
                stream: None,
            },
        }
    }
}
