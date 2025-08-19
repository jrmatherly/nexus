//! Amazon Nova output types for AWS Bedrock.

use crate::messages::{
    ChatChoice, ChatChoiceDelta, ChatCompletionChunk, ChatCompletionResponse, ChatMessage, ChatMessageDelta, ChatRole,
    FinishReason, ObjectType, Usage,
};
use serde::Deserialize;

// Re-use the role enum from input
use super::input::NovaRole;

/// Nova finish reason enum with forward compatibility.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NovaFinishReason {
    /// Natural completion (Nova uses "end_turn")
    #[serde(rename = "end_turn")]
    EndTurn,
    /// Natural completion (alternative)
    Stop,
    /// Maximum token limit reached
    #[serde(rename = "max_tokens")]
    MaxTokens,
    /// Maximum token limit reached (alternative)
    Length,
    /// Stop sequence encountered
    StopSequence,
    /// Content filter triggered
    ContentFiltered,
    /// Any other finish reason not yet known
    #[serde(untagged)]
    Other(String),
}

impl From<NovaFinishReason> for FinishReason {
    fn from(reason: NovaFinishReason) -> Self {
        match reason {
            NovaFinishReason::EndTurn => FinishReason::Stop,
            NovaFinishReason::Stop => FinishReason::Stop,
            NovaFinishReason::MaxTokens => FinishReason::Length,
            NovaFinishReason::Length => FinishReason::Length,
            NovaFinishReason::StopSequence => FinishReason::Stop,
            NovaFinishReason::ContentFiltered => FinishReason::ContentFilter,
            NovaFinishReason::Other(s) => {
                log::warn!("Unknown finish reason from Amazon Nova: {s}");
                FinishReason::Other(s)
            }
        }
    }
}

/// Response from Amazon Nova models.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NovaResponse {
    /// The generated message.
    pub output: NovaOutput,

    /// Reason the generation stopped.
    pub stop_reason: Option<NovaFinishReason>,

    /// Usage statistics.
    pub usage: Option<NovaUsage>,

    /// Additional metrics.
    #[serde(default)]
    #[allow(dead_code)]
    pub metrics: Option<NovaMetrics>,
}

/// Output message from Nova.
#[derive(Debug, Deserialize)]
pub(crate) struct NovaOutput {
    /// The generated message.
    pub message: NovaResponseMessage,
}

/// Response message structure.
#[derive(Debug, Deserialize)]
pub(crate) struct NovaResponseMessage {
    /// Role of the message (always "assistant" in responses).
    #[allow(dead_code)]
    pub role: NovaRole,

    /// Message content.
    pub content: Vec<NovaResponseContent>,
}

/// Response content.
#[derive(Debug, Deserialize)]
pub(crate) struct NovaResponseContent {
    /// Text content.
    pub text: String,
}

/// Usage statistics from Nova.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovaUsage {
    /// Number of input tokens.
    pub input_tokens: u32,

    /// Number of output tokens.
    pub output_tokens: u32,

    /// Total tokens (input + output).
    /// Note: This field is not present in streaming responses.
    #[serde(default)]
    pub total_tokens: u32,

    /// Cache read input token count.
    #[serde(default)]
    #[allow(dead_code)]
    pub cache_read_input_token_count: Option<u32>,

    /// Cache write input token count.
    #[serde(default)]
    #[allow(dead_code)]
    pub cache_write_input_token_count: Option<u32>,
}

/// Additional metrics from Nova.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NovaMetrics {
    /// Latency metrics.
    #[serde(default)]
    #[allow(dead_code)]
    pub latency_ms: Option<u32>,
}

impl From<NovaResponse> for ChatCompletionResponse {
    fn from(response: NovaResponse) -> Self {
        // Extract text from content array
        let content = response
            .output
            .message
            .content
            .into_iter()
            .map(|c| c.text)
            .collect::<Vec<_>>()
            .join("");

        let choice = ChatChoice {
            index: 0,
            message: ChatMessage {
                role: ChatRole::Assistant,
                content,
            },
            finish_reason: response
                .stop_reason
                .map(FinishReason::from)
                .unwrap_or(FinishReason::Stop),
        };

        let usage = response
            .usage
            .map(|u| Usage {
                prompt_tokens: u.input_tokens,
                completion_tokens: u.output_tokens,
                total_tokens: if u.total_tokens > 0 {
                    u.total_tokens
                } else {
                    u.input_tokens + u.output_tokens
                },
            })
            .unwrap_or_else(|| Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            });

        Self {
            id: format!("nova-{}", uuid::Uuid::new_v4()),
            object: ObjectType::ChatCompletion,
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            model: String::new(), // Will be set by transform_response
            choices: vec![choice],
            usage,
        }
    }
}

// Nova streaming types

/// Streaming chunk from Nova models.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum NovaStreamChunk {
    /// Message start event
    #[allow(dead_code)]
    MessageStart {
        #[serde(rename = "messageStart")]
        message_start: NovaMessageStart,
    },
    /// Content block delta event
    ContentBlockDelta {
        #[serde(rename = "contentBlockDelta")]
        content_block_delta: NovaContentBlockDelta,
    },
    /// Content block stop event
    #[allow(dead_code)]
    ContentBlockStop {
        #[serde(rename = "contentBlockStop")]
        content_block_stop: NovaContentBlockStop,
    },
    /// Message stop event
    MessageStop {
        #[serde(rename = "messageStop")]
        message_stop: NovaMessageStop,
    },
    /// Metadata chunk (sent at the end)
    /// This must be last in the enum to avoid matching other variants
    Metadata {
        /// Metadata for the chunk.
        metadata: NovaStreamMetadata,

        /// Amazon Bedrock invocation metrics (we store but don't use this)
        /// It duplicates token counts from metadata.usage and adds latency metrics
        #[serde(rename = "amazon-bedrock-invocationMetrics")]
        #[allow(dead_code)]
        invocation_metrics: Option<NovaInvocationMetrics>,
    },
}

/// Content block delta in streaming response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NovaContentBlockDelta {
    /// Delta containing the text.
    pub delta: NovaDelta,

    /// Index of the content block.
    pub content_block_index: u32,
}

/// Delta containing the actual text.
#[derive(Debug, Deserialize)]
pub(crate) struct NovaDelta {
    /// Text content.
    pub text: Option<String>,
}

/// Content block stop event.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NovaContentBlockStop {
    /// Index of the content block that stopped.
    #[allow(dead_code)]
    pub content_block_index: u32,
}

/// Message start event in streaming.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct NovaMessageStart {
    /// Role of the message (always "assistant").
    pub role: NovaRole,
}

/// Message stop event in streaming.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NovaMessageStop {
    /// Stop reason.
    pub stop_reason: Option<NovaFinishReason>,
}

/// Empty object for metrics/trace fields we don't use.
#[derive(Debug, Deserialize, Default)]
pub(crate) struct EmptyObject {}

/// Stream metadata.
#[derive(Debug, Deserialize)]
pub(crate) struct NovaStreamMetadata {
    /// Usage statistics.
    #[serde(default)]
    pub usage: Option<NovaUsage>,

    /// Additional metrics (empty object for now)
    #[serde(default)]
    #[allow(dead_code)]
    pub metrics: Option<EmptyObject>,

    /// Trace information (empty object for now)
    #[serde(default)]
    #[allow(dead_code)]
    pub trace: Option<EmptyObject>,
}

/// Amazon Bedrock invocation metrics (performance data, not needed for billing).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct NovaInvocationMetrics {
    /// Input token count (duplicates metadata.usage.inputTokens)
    pub input_token_count: Option<u32>,

    /// Output token count (duplicates metadata.usage.outputTokens)
    pub output_token_count: Option<u32>,

    /// Total invocation latency in milliseconds
    pub invocation_latency: Option<u32>,

    /// Time to first byte in milliseconds
    pub first_byte_latency: Option<u32>,

    /// Cache read input token count
    pub cache_read_input_token_count: Option<u32>,

    /// Cache write input token count
    pub cache_write_input_token_count: Option<u32>,
}

impl From<NovaStreamChunk> for Option<ChatCompletionChunk> {
    fn from(chunk: NovaStreamChunk) -> Self {
        match chunk {
            NovaStreamChunk::MessageStart { .. } => {
                // Message start just indicates the role, which we handle in ContentBlockDelta
                // We could return an empty chunk with role, but it's cleaner to skip it
                None
            }
            NovaStreamChunk::ContentBlockDelta { content_block_delta } => {
                // Extract text from delta
                let text = content_block_delta.delta.text;

                // Only create a chunk if there's actual text
                if text.is_some() {
                    Some(ChatCompletionChunk {
                        id: String::new(), // Will be set by caller
                        object: ObjectType::ChatCompletionChunk,
                        created: 0,           // Will be set by caller
                        model: String::new(), // Will be set by caller
                        system_fingerprint: None,
                        choices: vec![ChatChoiceDelta {
                            index: 0,
                            delta: ChatMessageDelta {
                                // Include role on first chunk (index 0)
                                role: if content_block_delta.content_block_index == 0 {
                                    text.as_ref().map(|_| ChatRole::Assistant)
                                } else {
                                    None
                                },
                                content: text,
                                function_call: None,
                                tool_calls: None,
                            },
                            finish_reason: None,
                            logprobs: None,
                        }],
                        usage: None,
                    })
                } else {
                    None
                }
            }
            NovaStreamChunk::ContentBlockStop { .. } => {
                // Ignore content block stop events - they don't provide useful info
                None
            }
            NovaStreamChunk::MessageStop { message_stop } => {
                let finish_reason = message_stop.stop_reason.map(FinishReason::from);

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
                    usage: None,
                })
            }
            NovaStreamChunk::Metadata { metadata, .. } => {
                // Handle metadata chunk with usage info
                let usage = metadata.usage.map(|u| Usage {
                    prompt_tokens: u.input_tokens,
                    completion_tokens: u.output_tokens,
                    total_tokens: if u.total_tokens > 0 {
                        u.total_tokens
                    } else {
                        u.input_tokens + u.output_tokens
                    },
                });

                // Return a final chunk with usage info
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
                        finish_reason: None, // Finish reason already sent in MessageStop
                        logprobs: None,
                    }],
                    usage,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_chunk_parsing_with_invocation_metrics() {
        // Test metadata JSON with invocation metrics
        let metadata_json = r#"{
            "metadata":{
                "usage":{
                    "inputTokens":2,
                    "outputTokens":31
                },
                "metrics":{},
                "trace":{}
            },
            "amazon-bedrock-invocationMetrics":{
                "inputTokenCount":2,
                "outputTokenCount":31,
                "invocationLatency":231,
                "firstByteLatency":64,
                "cacheReadInputTokenCount":0,
                "cacheWriteInputTokenCount":0
            }
        }"#;

        // Parse as NovaStreamChunk
        let parsed = sonic_rs::from_str::<NovaStreamChunk>(metadata_json).expect("Failed to parse metadata chunk");

        // Verify it's the Metadata variant with correct usage data
        match parsed {
            NovaStreamChunk::Metadata { metadata, .. } => {
                assert!(metadata.usage.is_some());
                let usage = metadata.usage.unwrap();
                assert_eq!(usage.input_tokens, 2);
                assert_eq!(usage.output_tokens, 31);
            }
            _ => unreachable!("Expected Metadata variant, got {:?}", parsed),
        }
    }

    #[test]
    fn test_content_chunk_parsing() {
        let content_json = r#"{
            "contentBlockDelta":{
                "delta":{
                    "text":"Hello world"
                },
                "contentBlockIndex":0
            }
        }"#;

        let parsed = sonic_rs::from_str::<NovaStreamChunk>(content_json).expect("Failed to parse content chunk");

        match parsed {
            NovaStreamChunk::ContentBlockDelta { content_block_delta } => {
                assert_eq!(content_block_delta.delta.text.unwrap(), "Hello world");
            }
            _ => unreachable!("Expected ContentBlockDelta variant, got {:?}", parsed),
        }
    }
}
