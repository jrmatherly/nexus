//! Cohere input types for AWS Bedrock.
//!
//! This module contains request types for Cohere Command-R models available through AWS Bedrock.
//! Command-R models use a structured chat-based format with message and chat_history fields.
//!
//! # Supported Models
//! - `cohere.command-r-v1:0`: Advanced conversational model
//! - `cohere.command-r-plus-v1:0`: Enhanced version with better performance
//!
//! # Model Characteristics
//! - **Input Format**: Structured chat with message and chat_history
//! - **Context Window**: Up to 128K tokens
//! - **Languages**: Strong multilingual support (100+ languages)
//! - **Streaming**: Supported with token-by-token delivery
//! - **Safety**: Built-in content filtering and safety measures
//!
//! # Official Documentation
//! - [Cohere Models on Bedrock](https://docs.aws.amazon.com/bedrock/latest/userguide/cohere-models.html)
//! - [Command-R Models](https://docs.aws.amazon.com/bedrock/latest/userguide/model-parameters-cohere-command-r-plus.html)

use std::borrow::Cow;

use serde::Serialize;

use crate::messages::{ChatCompletionRequest, ChatRole};

/// Chat history entry for Command-R models.
#[derive(Debug, Serialize)]
pub(crate) struct ChatHistoryEntry {
    /// Role of the speaker ("USER" or "CHATBOT").
    pub role: Cow<'static, str>,

    /// The message content.
    pub message: String,
}

/// Request payload for Cohere Command-R models.
///
/// Command-R models use a different API format with structured chat history
/// instead of simple role prefixes. This format is more similar to modern
/// chat APIs and provides better conversation tracking.
///
/// # Request Format
/// ```json
/// {
///   "message": "What is 2+2?",
///   "chat_history": [
///     {"role": "USER", "message": "Hi"},
///     {"role": "CHATBOT", "message": "Hello!"}
///   ],
///   "max_tokens": 100,
///   "temperature": 0.3
/// }
/// ```
#[derive(Debug, Serialize)]
pub(crate) struct CohereCommandRRequest {
    /// The current user message to respond to.
    pub message: String,

    /// Conversation history.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_history: Option<Vec<ChatHistoryEntry>>,

    /// Maximum tokens to generate (1-4096).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Sampling temperature (0.0-1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Nucleus sampling parameter (0.01-0.99).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p: Option<f32>,

    /// Top-k sampling parameter (0-500).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k: Option<u32>,

    /// Stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    /// Enable streaming (not used in request, controlled by endpoint).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

impl From<ChatCompletionRequest> for CohereCommandRRequest {
    fn from(request: ChatCompletionRequest) -> Self {
        // Split messages into current message and history
        // The last user message becomes the "message" field
        // All previous messages become chat_history

        let mut chat_history = Vec::new();
        let mut current_message = String::new();

        // Process messages in reverse to find the last user message
        for (i, msg) in request.messages.iter().enumerate().rev() {
            if matches!(msg.role, ChatRole::User) {
                current_message = msg.content.clone();

                // Add all previous messages to chat_history
                for prev_msg in &request.messages[..i] {
                    let role = match &prev_msg.role {
                        ChatRole::System => {
                            // System messages can be treated as USER messages with context
                            Cow::Borrowed("USER")
                        }
                        ChatRole::User => Cow::Borrowed("USER"),
                        ChatRole::Assistant => Cow::Borrowed("CHATBOT"),
                        ChatRole::Other(role) => Cow::Owned(role.to_uppercase()),
                    };

                    chat_history.push(ChatHistoryEntry {
                        role,
                        message: prev_msg.content.clone(),
                    });
                }
                break;
            }
        }

        // If no user message found, use the last message regardless of role
        if current_message.is_empty() && !request.messages.is_empty() {
            current_message = request.messages.last().unwrap().content.clone();
        }

        Self {
            message: current_message,
            chat_history: if chat_history.is_empty() {
                None
            } else {
                Some(chat_history)
            },
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            p: request.top_p,
            k: None,
            stop_sequences: request.stop.clone(),
            stream: None,
        }
    }
}
