//! Amazon Nova input types for AWS Bedrock.
//!
//! Nova models use a modern messages-based format similar to Claude and other contemporary models,
//! replacing the legacy Titan text concatenation approach.
//!
//! # Supported Models
//! - `amazon.nova-micro-v1:0`: Smallest, fastest model for simple tasks
//! - `amazon.nova-lite-v1:0`: Balanced performance and capability
//! - `amazon.nova-pro-v1:0`: Advanced reasoning and longer contexts (up to 300K tokens)
//! - `amazon.nova-premier-v1:0`: Most capable model with up to 1M token context
//!
//! # Model Characteristics
//! - **Input Format**: Structured messages array with roles
//! - **Context Windows**: 24K to 1M tokens depending on variant
//! - **Languages**: Strong multilingual support
//! - **Streaming**: Supported via API endpoint
//! - **Schema Version**: Uses "messages-v1" schema
//!
//! # Official Documentation
//! - [Amazon Nova Models](https://docs.aws.amazon.com/nova/latest/userguide/using-invoke-api.html)

use crate::messages::{ChatCompletionRequest, ChatRole};
use serde::{Deserialize, Serialize};

/// Schema version for Nova requests.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NovaSchemaVersion {
    /// Messages v1 schema - the current standard.
    #[serde(rename = "messages-v1")]
    MessagesV1,
    /// Any other schema version not yet known.
    #[serde(untagged)]
    Other(String),
}

/// Role in a Nova conversation.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum NovaRole {
    /// User message.
    User,
    /// Assistant response.
    Assistant,
    /// Any other role not yet known.
    #[serde(untagged)]
    Other(String),
}

/// Request payload for Amazon Nova models.
///
/// Nova uses a structured messages format with separate system prompts,
/// similar to modern chat models like Claude.
///
/// # Request Format
/// ```json
/// {
///   "schemaVersion": "messages-v1",
///   "messages": [
///     {"role": "user", "content": [{"text": "Hello"}]}
///   ],
///   "system": [{"text": "You are helpful"}],
///   "inferenceConfig": {
///     "maxTokens": 100,
///     "temperature": 0.7,
///     "topP": 0.9,
///     "topK": 20
///   }
/// }
/// ```
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NovaRequest {
    /// Schema version for the request format.
    /// Currently must be "messages-v1".
    pub schema_version: NovaSchemaVersion,

    /// Array of messages in the conversation.
    pub messages: Vec<NovaMessage>,

    /// System prompts/instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<NovaSystemMessage>>,

    /// Inference configuration parameters.
    pub inference_config: NovaInferenceConfig,
}

/// A message in a Nova conversation.
#[derive(Debug, Serialize)]
pub(crate) struct NovaMessage {
    /// Role of the message sender.
    pub role: NovaRole,

    /// Message content array.
    pub content: Vec<NovaContent>,
}

/// Content within a Nova message.
#[derive(Debug, Serialize)]
pub(crate) struct NovaContent {
    /// Text content of the message.
    pub text: String,
}

/// System message for Nova models.
#[derive(Debug, Serialize)]
pub(crate) struct NovaSystemMessage {
    /// System instruction text.
    pub text: String,
}

/// Inference configuration for Nova models.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NovaInferenceConfig {
    /// Maximum number of tokens to generate (1-4096 or higher for some models).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Temperature for randomness (0.0-1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p nucleus sampling (0.0-1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top-k sampling (0-500).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
}

impl From<ChatCompletionRequest> for NovaRequest {
    fn from(request: ChatCompletionRequest) -> Self {
        let mut messages = Vec::new();
        let mut system_messages = Vec::new();

        // Process messages and extract system prompts
        for msg in request.messages {
            match msg.role {
                ChatRole::System => {
                    // System messages go in the system array
                    system_messages.push(NovaSystemMessage { text: msg.content });
                }
                ChatRole::User => {
                    messages.push(NovaMessage {
                        role: NovaRole::User,
                        content: vec![NovaContent { text: msg.content }],
                    });
                }
                ChatRole::Assistant => {
                    messages.push(NovaMessage {
                        role: NovaRole::Assistant,
                        content: vec![NovaContent { text: msg.content }],
                    });
                }
                ChatRole::Other(role) => {
                    // Map unknown roles to user
                    log::warn!("Unknown role '{}' in Nova request, mapping to user", role);

                    messages.push(NovaMessage {
                        role: NovaRole::User,
                        content: vec![NovaContent { text: msg.content }],
                    });
                }
            }
        }

        // Ensure we have at least one message
        if messages.is_empty() && !system_messages.is_empty() {
            // If only system messages, add a default user message
            messages.push(NovaMessage {
                role: NovaRole::User,
                content: vec![NovaContent {
                    text: "Please respond according to the system instructions.".to_string(),
                }],
            });
        }

        Self {
            schema_version: NovaSchemaVersion::MessagesV1,
            messages,
            system: if system_messages.is_empty() {
                None
            } else {
                Some(system_messages)
            },
            inference_config: NovaInferenceConfig {
                max_tokens: request.max_tokens,
                temperature: request.temperature,
                top_p: request.top_p,
                top_k: None, // Nova supports topK but OpenAI format doesn't include it
            },
        }
    }
}
