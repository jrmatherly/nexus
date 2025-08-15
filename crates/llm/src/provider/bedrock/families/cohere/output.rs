//! Cohere output types for AWS Bedrock.
//!
//! This module supports Command-R models (command-r and command-r-plus).

use serde::Deserialize;
use sonic_rs::Value;

use crate::messages::{
    ChatChoice, ChatChoiceDelta, ChatCompletionChunk, ChatCompletionResponse, ChatMessage, ChatMessageDelta, ChatRole,
    FinishReason, ObjectType, Usage,
};

/// Cohere Command-R finish reason enum.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum CohereCommandRFinishReason {
    /// Natural completion
    #[serde(rename = "COMPLETE")]
    Complete,
    /// Maximum token limit reached
    #[serde(rename = "MAX_TOKENS")]
    MaxTokens,
    /// Stop sequence triggered
    #[serde(rename = "STOP_SEQUENCE")]
    StopSequence,
    /// Tool use requested
    #[serde(rename = "TOOL_CALL")]
    ToolCall,
    /// Any other finish reason not yet known
    #[serde(untagged)]
    Other(String),
}

impl From<CohereCommandRFinishReason> for FinishReason {
    fn from(reason: CohereCommandRFinishReason) -> Self {
        match reason {
            CohereCommandRFinishReason::Complete => FinishReason::Stop,
            CohereCommandRFinishReason::MaxTokens => FinishReason::Length,
            CohereCommandRFinishReason::StopSequence => FinishReason::Stop,
            CohereCommandRFinishReason::ToolCall => FinishReason::ToolCalls,
            CohereCommandRFinishReason::Other(s) => {
                log::warn!("Unknown finish reason from Bedrock Cohere Command-R: {s}");
                FinishReason::Other(s)
            }
        }
    }
}

/// Response from Cohere Command-R models.
#[derive(Debug, Deserialize)]
pub(crate) struct CohereCommandRResponse {
    /// Unique response identifier
    pub response_id: String,
    /// Generated text
    pub text: String,
    /// Unique generation identifier
    #[allow(dead_code)]
    pub generation_id: String,
    /// Finish reason for the generation
    pub finish_reason: CohereCommandRFinishReason,
    /// Chat history including the new response
    #[allow(dead_code)]
    pub chat_history: Vec<Value>,
    /// Token usage statistics (optional)
    #[serde(default)]
    pub meta: Option<CohereCommandRMeta>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereCommandRMeta {
    pub billed_units: Option<CohereCommandRBilledUnits>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereCommandRBilledUnits {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

impl From<CohereCommandRResponse> for ChatCompletionResponse {
    fn from(response: CohereCommandRResponse) -> Self {
        // Extract token usage if available
        let usage = response
            .meta
            .and_then(|m| m.billed_units)
            .map(|bu| Usage {
                prompt_tokens: bu.input_tokens.unwrap_or(0),
                completion_tokens: bu.output_tokens.unwrap_or(0),
                total_tokens: bu.input_tokens.unwrap_or(0) + bu.output_tokens.unwrap_or(0),
            })
            .unwrap_or_else(|| Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            });

        let choice = ChatChoice {
            index: 0,
            message: ChatMessage {
                role: ChatRole::Assistant,
                content: response.text,
            },
            finish_reason: FinishReason::from(response.finish_reason),
        };

        Self {
            id: response.response_id,
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

// Cohere streaming types - shared by Command-R models

#[derive(Debug, Deserialize)]
pub(crate) struct CohereStreamChunk {
    text: Option<String>,
    is_finished: Option<bool>,
    finish_reason: Option<CohereCommandRFinishReason>,
    #[serde(default)]
    response: Option<CohereStreamResponse>,
}

#[derive(Debug, Deserialize)]
struct CohereStreamResponse {
    #[serde(default)]
    token_count: Option<CohereTokenCount>,
}

#[derive(Debug, Deserialize)]
struct CohereTokenCount {
    prompt_tokens: Option<u32>,
    response_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

impl From<CohereStreamChunk> for Option<ChatCompletionChunk> {
    fn from(chunk: CohereStreamChunk) -> Self {
        let finish_reason = if chunk.is_finished.unwrap_or(false) {
            chunk.finish_reason.map(FinishReason::from)
        } else {
            None
        };

        let usage = chunk.response.and_then(|r| r.token_count).map(|tc| Usage {
            prompt_tokens: tc.prompt_tokens.unwrap_or(0),
            completion_tokens: tc.response_tokens.unwrap_or(0),
            total_tokens: tc.total_tokens.unwrap_or(0),
        });

        if chunk.text.is_some() || finish_reason.is_some() {
            Some(ChatCompletionChunk {
                id: String::new(), // Will be set by caller
                object: ObjectType::ChatCompletionChunk,
                created: 0,           // Will be set by caller
                model: String::new(), // Will be set by caller
                system_fingerprint: None,
                choices: vec![ChatChoiceDelta {
                    index: 0,
                    delta: ChatMessageDelta {
                        role: chunk.text.as_ref().map(|_| ChatRole::Assistant),
                        content: chunk.text,
                        function_call: None,
                        tool_calls: None,
                    },
                    finish_reason,
                    logprobs: None,
                }],
                usage,
            })
        } else {
            None
        }
    }
}
