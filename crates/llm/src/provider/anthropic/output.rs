use serde::Deserialize;

use crate::messages::{
    ChatChoice, ChatCompletionResponse, ChatMessage, ChatRole, FinishReason, Model, ObjectType, Usage,
};

/// Describes the type of content in an Anthropic message.
///
/// Used to distinguish between different content blocks in the response.
#[derive(Debug, Deserialize, PartialEq)]
pub(super) enum ContentType {
    /// Plain text content.
    #[serde(rename = "text")]
    Text,
    /// Tool use request from the model.
    #[serde(rename = "tool_use")]
    ToolUse,
    /// Result from a tool execution.
    #[serde(rename = "tool_result")]
    ToolResult,
    /// Image content (for multi-modal inputs).
    #[serde(rename = "image")]
    Image,
    /// Any other content type not yet known.
    /// Captures the actual string value for forward compatibility.
    #[serde(untagged)]
    Other(String),
}

/// The reason why the model stopped generating tokens.
///
/// Provides insight into why the generation ended.
#[derive(Debug, Deserialize, PartialEq)]
pub(super) enum StopReason {
    /// The model reached a natural stopping point.
    /// This is the most common stop reason for conversational responses.
    #[serde(rename = "end_turn")]
    EndTurn,
    /// The generation exceeded the maximum token limit specified in the request.
    #[serde(rename = "max_tokens")]
    MaxTokens,
    /// The model encountered a stop sequence specified in the request.
    #[serde(rename = "stop_sequence")]
    StopSequence,
    /// The model invoked a tool.
    #[serde(rename = "tool_use")]
    ToolUse,
    /// The model paused its turn (for advanced use cases).
    #[serde(rename = "pause_turn")]
    PauseTurn,
    /// The model refused to generate content due to safety concerns.
    #[serde(rename = "refusal")]
    Refusal,
    /// Any other stop reason not yet known.
    /// Captures the actual string value for forward compatibility.
    #[serde(untagged)]
    Other(String),
}

/// The type of response from the Anthropic API.
#[derive(Debug, Deserialize, PartialEq)]
pub(super) enum ResponseType {
    /// A standard message response.
    #[serde(rename = "message")]
    Message,
    /// Any other response type not yet known.
    /// Captures the actual string value for forward compatibility.
    #[serde(untagged)]
    Other(String),
}

/// Response from Anthropic Messages API.
///
/// This struct represents the response format from creating messages with Claude
/// as documented in the [Anthropic API Reference](https://docs.anthropic.com/en/api/messages).
#[derive(Debug, Deserialize)]
pub(super) struct AnthropicResponse {
    /// Unique identifier for the message.
    pub id: String,

    /// Object type. Always "message" for message responses.
    #[allow(dead_code)]
    pub r#type: ResponseType,

    /// Conversational role of the generated message.
    /// This will always be "assistant".
    pub role: ChatRole,

    /// Content blocks in the response.
    /// Each block contains a portion of the response with its type.
    pub content: Vec<AnthropicContent>,

    /// The model that handled the request.
    #[allow(dead_code)]
    pub model: String,

    /// The reason the model stopped generating.
    /// See [`StopReason`] for possible values.
    pub stop_reason: Option<StopReason>,

    /// Which custom stop sequence was triggered, if any.
    #[allow(dead_code)]
    pub stop_sequence: Option<String>,

    /// Billing and rate limit usage information.
    pub usage: AnthropicUsage,
}

/// A content block in an Anthropic message response.
///
/// Represents a single piece of content which could be text, tool use, etc.
#[derive(Debug, Deserialize)]
pub(super) struct AnthropicContent {
    /// The type of this content block.
    pub r#type: ContentType,

    /// Text content if this is a text block.
    /// Will be `None` for non-text content types.
    #[serde(default)]
    pub text: Option<String>,
    // Additional fields for other content types can be added here:
    // pub id: Option<String>,  // for tool_use
    // pub name: Option<String>,  // for tool_use
    // pub input: Option<serde_json::Value>,  // for tool_use
}

/// Token usage information for an Anthropic API request.
///
/// Used for tracking consumption and billing.
#[derive(Debug, Deserialize)]
pub(super) struct AnthropicUsage {
    /// Number of tokens in the input prompt.
    /// This includes the system prompt, messages, and any other input.
    pub input_tokens: i32,

    /// Number of tokens generated in the response.
    pub output_tokens: i32,
}

/// Response from listing available Anthropic models.
///
/// Note: Anthropic doesn't have an official models listing endpoint,
/// so this structure is used for compatibility with our unified interface.
#[derive(Debug, Deserialize)]
pub(super) struct AnthropicModelsResponse {
    /// List of available Anthropic models.
    pub data: Vec<AnthropicModel>,
}

/// Describes an Anthropic model.
///
/// Contains metadata about a specific Claude model.
/// Note: Since Anthropic doesn't have an official models endpoint,
/// some fields may be populated with default values.
#[derive(Debug, Deserialize)]
pub(super) struct AnthropicModel {
    /// The model identifier.
    /// Examples: "claude-3-opus-20240229", "claude-3-sonnet-20240229"
    pub id: String,

    /// Unix timestamp of when the model was created.
    /// May be None as Anthropic doesn't provide this information.
    #[serde(default)]
    pub created: Option<u64>,
}

impl From<AnthropicResponse> for ChatCompletionResponse {
    fn from(response: AnthropicResponse) -> Self {
        let message_content = response
            .content
            .iter()
            .filter_map(|c| match &c.r#type {
                ContentType::Text => c.text.clone(),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        Self {
            id: response.id,
            object: ObjectType::ChatCompletion,
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            model: String::new(), // Will be set by the provider
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: response.role,
                    content: message_content,
                },
                finish_reason: response
                    .stop_reason
                    .map(|sr| match sr {
                        StopReason::EndTurn => FinishReason::Stop,
                        StopReason::MaxTokens => FinishReason::Length,
                        StopReason::StopSequence => FinishReason::Stop,
                        StopReason::ToolUse => FinishReason::ToolCalls,
                        StopReason::PauseTurn => FinishReason::Other("pause".to_string()),
                        StopReason::Refusal => FinishReason::ContentFilter,
                        StopReason::Other(s) => {
                            log::warn!("Unknown stop reason from Anthropic: {s}");
                            FinishReason::Other(s)
                        }
                    })
                    .unwrap_or(FinishReason::Stop),
            }],
            usage: Usage {
                prompt_tokens: response.usage.input_tokens as u32,
                completion_tokens: response.usage.output_tokens as u32,
                total_tokens: (response.usage.input_tokens + response.usage.output_tokens) as u32,
            },
        }
    }
}

impl From<AnthropicModel> for Model {
    fn from(model: AnthropicModel) -> Self {
        Self {
            id: model.id,
            object: ObjectType::Model,
            created: model.created.unwrap_or(0),
            owned_by: "anthropic".to_string(),
        }
    }
}
