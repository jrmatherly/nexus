//! Mistral output types for AWS Bedrock.

use serde::Deserialize;

use crate::messages::{
    ChatChoice, ChatChoiceDelta, ChatCompletionChunk, ChatCompletionResponse, ChatMessage, ChatMessageDelta, ChatRole,
    FinishReason, ObjectType, Usage,
};

/// Mistral stop reason enum with forward compatibility.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub(crate) enum MistralStopReason {
    /// Natural stop point
    Stop,
    /// Maximum token limit reached
    Length,
    /// Any other stop reason not yet known
    #[serde(untagged)]
    Other(String),
}

impl From<MistralStopReason> for FinishReason {
    fn from(reason: MistralStopReason) -> Self {
        match reason {
            MistralStopReason::Stop => FinishReason::Stop,
            MistralStopReason::Length => FinishReason::Length,
            MistralStopReason::Other(s) => {
                log::warn!("Unknown stop reason from Bedrock Mistral: {s}");
                FinishReason::Other(s)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct MistralResponse {
    pub outputs: Vec<MistralOutput>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MistralOutput {
    pub text: String,
    pub stop_reason: Option<MistralStopReason>,
}

impl From<MistralOutput> for ChatChoice {
    fn from(output: MistralOutput) -> Self {
        let finish_reason = output.stop_reason.map(FinishReason::from).unwrap_or(FinishReason::Stop);

        Self {
            index: 0,
            message: ChatMessage {
                role: ChatRole::Assistant,
                content: output.text,
            },
            finish_reason,
        }
    }
}

impl From<MistralResponse> for ChatCompletionResponse {
    fn from(response: MistralResponse) -> Self {
        let choice = response
            .outputs
            .into_iter()
            .next()
            .map(ChatChoice::from)
            .unwrap_or_else(|| {
                log::error!("No outputs in Mistral response, creating error choice");
                ChatChoice {
                    index: 0,
                    message: ChatMessage {
                        role: ChatRole::Assistant,
                        content: "Error: No outputs in response".to_string(),
                    },
                    finish_reason: FinishReason::Other("ERROR".to_string()),
                }
            });

        Self {
            id: format!("mistral-{}", uuid::Uuid::new_v4()),
            object: ObjectType::ChatCompletion,
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            model: String::new(), // Will be set by transform_response
            choices: vec![choice],
            usage: Usage {
                prompt_tokens: 0, // Mistral doesn't provide token counts in this format
                completion_tokens: 0,
                total_tokens: 0,
            },
        }
    }
}

// Mistral streaming types

#[derive(Debug, Deserialize)]
pub(crate) struct MistralStreamChunk {
    outputs: Vec<MistralStreamOutput>,
}

#[derive(Debug, Deserialize)]
struct MistralStreamOutput {
    text: Option<String>,
    stop_reason: Option<MistralStopReason>,
}

impl From<MistralStreamChunk> for Option<ChatCompletionChunk> {
    fn from(chunk: MistralStreamChunk) -> Self {
        chunk.outputs.first().and_then(|output| {
            let finish_reason = output.stop_reason.clone().map(Into::into);

            if output.text.is_some() || finish_reason.is_some() {
                Some(ChatCompletionChunk {
                    id: String::new(), // Will be set by caller
                    object: ObjectType::ChatCompletionChunk,
                    created: 0,           // Will be set by caller
                    model: String::new(), // Will be set by caller
                    system_fingerprint: None,
                    choices: vec![ChatChoiceDelta {
                        index: 0,
                        delta: ChatMessageDelta {
                            role: output.text.as_ref().map(|_| ChatRole::Assistant),
                            content: output.text.clone(),
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
        })
    }
}
