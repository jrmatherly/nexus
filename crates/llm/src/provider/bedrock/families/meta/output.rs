//! Meta Llama output types for AWS Bedrock.

use serde::Deserialize;

use crate::messages::{
    ChatChoice, ChatChoiceDelta, ChatCompletionChunk, ChatCompletionResponse, ChatMessage, ChatMessageDelta, ChatRole,
    FinishReason, ObjectType, Usage,
};

/// Llama stop reason enum with forward compatibility.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LlamaStopReason {
    /// Natural stop point
    Stop,
    /// Maximum token limit reached
    Length,
    /// Any other stop reason not yet known
    #[serde(untagged)]
    Other(String),
}

impl From<LlamaStopReason> for FinishReason {
    fn from(reason: LlamaStopReason) -> Self {
        match reason {
            LlamaStopReason::Stop => FinishReason::Stop,
            LlamaStopReason::Length => FinishReason::Length,
            LlamaStopReason::Other(s) => {
                log::warn!("Unknown stop reason from Bedrock Llama: {s}");
                FinishReason::Other(s)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct LlamaResponse {
    pub generation: String,
    pub generation_token_count: u32,
    pub prompt_token_count: u32,
    pub stop_reason: Option<LlamaStopReason>,
}

impl From<LlamaResponse> for ChatCompletionResponse {
    fn from(response: LlamaResponse) -> Self {
        let finish_reason = response
            .stop_reason
            .map(FinishReason::from)
            .unwrap_or(FinishReason::Stop);

        Self {
            id: format!("llama-{}", uuid::Uuid::new_v4()),
            object: ObjectType::ChatCompletion,
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            model: String::new(), // Will be set by transform_response
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: ChatRole::Assistant,
                    content: response.generation,
                },
                finish_reason,
            }],
            usage: Usage {
                prompt_tokens: response.prompt_token_count,
                completion_tokens: response.generation_token_count,
                total_tokens: response.prompt_token_count + response.generation_token_count,
            },
        }
    }
}

// Meta Llama streaming types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct LlamaStreamChunk {
    generation: Option<String>,
    stop_reason: Option<LlamaStopReason>,
    #[allow(dead_code)]
    generation_token_count: Option<u32>,
}

impl From<LlamaStreamChunk> for Option<ChatCompletionChunk> {
    fn from(chunk: LlamaStreamChunk) -> Self {
        let finish_reason = chunk.stop_reason.map(Into::into);

        if chunk.generation.is_some() || finish_reason.is_some() {
            Some(ChatCompletionChunk {
                id: String::new(), // Will be set by caller
                object: ObjectType::ChatCompletionChunk,
                created: 0,           // Will be set by caller
                model: String::new(), // Will be set by caller
                system_fingerprint: None,
                choices: vec![ChatChoiceDelta {
                    index: 0,
                    delta: ChatMessageDelta {
                        role: chunk.generation.as_ref().map(|_| ChatRole::Assistant),
                        content: chunk.generation,
                        function_call: None,
                        tool_calls: None,
                    },
                    finish_reason,
                    logprobs: None,
                }],
                usage: None,
            })
        } else {
            None
        }
    }
}
