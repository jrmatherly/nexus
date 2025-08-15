//! Anthropic output types for AWS Bedrock.
//!
//! This module contains streaming response types for Anthropic Claude models on Bedrock.
//! For regular (non-streaming) responses, the standard `AnthropicResponse` type from
//! the main Anthropic provider is used directly.
//!
//! # Streaming Response Format
//! Anthropic models use Server-Sent Events (SSE) with different event types:
//! - `message_start`: Indicates the beginning of a response
//! - `content_block_start`: Start of a content block
//! - `content_block_delta`: Incremental content tokens
//! - `content_block_stop`: End of a content block  
//! - `message_delta`: Usage statistics and completion metadata
//! - `message_stop`: End of the complete response
//! - `ping`: Keep-alive events (ignored)
//! - `error`: Error events (handled separately)
//!
//! # Event Processing
//! The streaming implementation converts these events into OpenAI-compatible
//! `ChatCompletionChunk` format for consistency across providers.
//!
//! # Official Documentation
//! - [Claude Streaming API](https://docs.anthropic.com/en/api/messages-streaming)

use serde::Deserialize;

use crate::messages::{
    ChatChoiceDelta, ChatCompletionChunk, ChatMessageDelta, ChatRole, FinishReason, ObjectType, Usage,
};

// Anthropic streaming event types
#[derive(Debug, Deserialize, PartialEq)]
enum AnthropicEventType {
    #[serde(rename = "message_start")]
    MessageStart,
    #[serde(rename = "content_block_start")]
    ContentBlockStart,
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta,
    #[serde(rename = "content_block_stop")]
    ContentBlockStop,
    #[serde(rename = "message_delta")]
    MessageDelta,
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error,
    /// Any other event type not yet known.
    /// Captures the actual string value for forward compatibility.
    #[serde(untagged)]
    Unknown(String),
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicStreamChunk {
    #[serde(rename = "type")]
    event_type: AnthropicEventType,
    #[serde(default)]
    delta: Option<AnthropicDelta>,
    #[serde(default)]
    #[allow(dead_code)]
    message: Option<AnthropicMessage>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
enum AnthropicStopReason {
    #[serde(rename = "end_turn")]
    EndTurn,
    #[serde(rename = "max_tokens")]
    MaxTokens,
    #[serde(rename = "stop_sequence")]
    StopSequence,
    #[serde(rename = "tool_use")]
    ToolUse,
    /// Any other stop reason not yet known.
    /// Captures the actual string value for forward compatibility.
    #[serde(untagged)]
    Unknown(String),
}

#[derive(Debug, Deserialize)]
struct AnthropicDelta {
    // Note: message_delta events don't have a "type" field in their delta
    #[serde(rename = "type")]
    #[serde(default)]
    #[allow(dead_code)]
    delta_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    stop_reason: Option<AnthropicStopReason>,
    #[serde(default)]
    #[allow(dead_code)]
    stop_sequence: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessage {
    #[serde(default)]
    #[allow(dead_code)]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

impl From<AnthropicStopReason> for FinishReason {
    fn from(reason: AnthropicStopReason) -> Self {
        match reason {
            AnthropicStopReason::EndTurn => FinishReason::Stop,
            AnthropicStopReason::MaxTokens => FinishReason::Length,
            AnthropicStopReason::StopSequence => FinishReason::Stop,
            AnthropicStopReason::ToolUse => FinishReason::ToolCalls,
            AnthropicStopReason::Unknown(s) => FinishReason::Other(s),
        }
    }
}

impl From<AnthropicStreamChunk> for Option<ChatCompletionChunk> {
    fn from(chunk: AnthropicStreamChunk) -> Self {
        match chunk.event_type {
            AnthropicEventType::MessageStart => {
                // First chunk with role
                Some(ChatCompletionChunk {
                    id: String::new(), // Will be set by caller
                    object: ObjectType::ChatCompletionChunk,
                    created: 0,           // Will be set by caller
                    model: String::new(), // Will be set by caller
                    system_fingerprint: None,
                    choices: vec![ChatChoiceDelta {
                        index: 0,
                        delta: ChatMessageDelta {
                            role: Some(ChatRole::Assistant),
                            content: None,
                            function_call: None,
                            tool_calls: None,
                        },
                        finish_reason: None,
                        logprobs: None,
                    }],
                    usage: None,
                })
            }
            AnthropicEventType::ContentBlockDelta => {
                // Content chunk
                chunk.delta.and_then(|delta| {
                    delta.text.map(|text| ChatCompletionChunk {
                        id: String::new(), // Will be set by caller
                        object: ObjectType::ChatCompletionChunk,
                        created: 0,           // Will be set by caller
                        model: String::new(), // Will be set by caller
                        system_fingerprint: None,
                        choices: vec![ChatChoiceDelta {
                            index: 0,
                            delta: ChatMessageDelta {
                                role: None,
                                content: Some(text),
                                function_call: None,
                                tool_calls: None,
                            },
                            finish_reason: None,
                            logprobs: None,
                        }],
                        usage: None,
                    })
                })
            }
            AnthropicEventType::MessageDelta => {
                // Final chunk with usage and stop reason
                let finish_reason = chunk.delta.and_then(|d| d.stop_reason.map(Into::into));

                let usage = chunk.usage.map(|u| Usage {
                    prompt_tokens: u.input_tokens.unwrap_or(0),
                    completion_tokens: u.output_tokens.unwrap_or(0),
                    total_tokens: u.input_tokens.unwrap_or(0) + u.output_tokens.unwrap_or(0),
                });

                Some(ChatCompletionChunk {
                    id: String::new(), // Will be set by caller
                    object: ObjectType::ChatCompletionChunk,
                    created: 0,           // Will be set by caller
                    model: String::new(), // Will be set by caller
                    system_fingerprint: None,
                    choices: vec![ChatChoiceDelta {
                        index: 0,
                        delta: ChatMessageDelta {
                            role: None,
                            content: None,
                            function_call: None,
                            tool_calls: None,
                        },
                        finish_reason,
                        logprobs: None,
                    }],
                    usage,
                })
            }
            _ => None, // Skip other event types (ContentBlockStart, ContentBlockStop, MessageStop, Ping, Error, Unknown)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_delta_with_usage() {
        // This is the exact format that was causing the error
        let json = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":6}}"#;

        let chunk: AnthropicStreamChunk = sonic_rs::from_str(json).expect("Failed to parse message_delta");
        assert_eq!(chunk.event_type, AnthropicEventType::MessageDelta);
        assert!(chunk.delta.is_some());
        assert!(chunk.usage.is_some());

        let delta = chunk.delta.unwrap();
        assert_eq!(delta.stop_reason, Some(AnthropicStopReason::EndTurn));

        let usage = chunk.usage.unwrap();
        assert_eq!(usage.output_tokens, Some(6));
    }

    #[test]
    fn test_content_block_delta() {
        let json = r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello"}}"#;

        let chunk: AnthropicStreamChunk = sonic_rs::from_str(json).expect("Failed to parse content_block_delta");
        assert_eq!(chunk.event_type, AnthropicEventType::ContentBlockDelta);
        assert!(chunk.delta.is_some());

        let delta = chunk.delta.unwrap();
        assert_eq!(delta.text, Some("Hello".to_string()));
    }

    #[test]
    fn test_message_start() {
        let json = r#"{"type":"message_start","message":{"usage":{"input_tokens":10}}}"#;

        let chunk: AnthropicStreamChunk = sonic_rs::from_str(json).expect("Failed to parse message_start");
        assert_eq!(chunk.event_type, AnthropicEventType::MessageStart);
        assert!(chunk.message.is_some());
    }

    #[test]
    fn test_message_delta_conversion_to_completion_chunk() {
        let json = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":6}}"#;

        let chunk: AnthropicStreamChunk = sonic_rs::from_str(json).unwrap();
        let completion_chunk: Option<ChatCompletionChunk> = chunk.into();

        assert!(completion_chunk.is_some());
        let completion_chunk = completion_chunk.unwrap();

        assert!(completion_chunk.usage.is_some());
        let usage = completion_chunk.usage.unwrap();
        assert_eq!(usage.completion_tokens, 6);

        assert_eq!(completion_chunk.choices[0].finish_reason, Some(FinishReason::Stop));
    }
}
