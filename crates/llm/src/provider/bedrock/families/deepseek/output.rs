//! DeepSeek output types for AWS Bedrock.

use crate::messages::{
    ChatChoice, ChatChoiceDelta, ChatCompletionChunk, ChatCompletionResponse, ChatMessage, ChatMessageDelta, ChatRole,
    FinishReason, ObjectType, Usage,
};
use serde::Deserialize;
use uuid::Uuid;

/// DeepSeek finish reason enum with forward compatibility.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeepSeekFinishReason {
    /// Natural completion.
    Stop,
    /// Maximum token limit reached.
    Length,
    /// Any other finish reason not yet known.
    #[serde(untagged)]
    Other(String),
}

impl From<DeepSeekFinishReason> for FinishReason {
    fn from(reason: DeepSeekFinishReason) -> Self {
        match reason {
            DeepSeekFinishReason::Stop => FinishReason::Stop,
            DeepSeekFinishReason::Length => FinishReason::Length,
            DeepSeekFinishReason::Other(s) => {
                log::warn!("Unknown finish reason from DeepSeek: {s}");
                FinishReason::Other(s)
            }
        }
    }
}

/// Response from DeepSeek models.
#[derive(Debug, Deserialize)]
pub(crate) struct DeepSeekResponse {
    /// Array of completion choices.
    pub choices: Vec<DeepSeekChoice>,

    /// Usage statistics (optional).
    #[serde(default)]
    pub usage: Option<DeepSeekUsage>,
}

/// A single completion choice from DeepSeek.
#[derive(Debug, Deserialize)]
pub(crate) struct DeepSeekChoice {
    /// The generated text.
    pub text: String,

    /// Reason the generation stopped.
    pub stop_reason: Option<DeepSeekFinishReason>,

    /// Optional reasoning content (for R1 models).
    #[serde(default)]
    pub reasoning_content: Option<DeepSeekReasoningContent>,
}

/// Reasoning content from DeepSeek R1.
#[derive(Debug, Deserialize)]
pub(crate) struct DeepSeekReasoningContent {
    /// The reasoning text showing chain of thought.
    pub reasoning_text: Option<String>,
}

/// Usage statistics from DeepSeek.
#[derive(Debug, Deserialize)]
pub(crate) struct DeepSeekUsage {
    /// Number of input tokens.
    pub input_tokens: Option<u32>,

    /// Number of output tokens.
    pub output_tokens: Option<u32>,

    /// Total tokens (may be calculated if not provided).
    pub total_tokens: Option<u32>,
}

impl From<DeepSeekResponse> for ChatCompletionResponse {
    fn from(response: DeepSeekResponse) -> Self {
        // Take the first choice (DeepSeek typically returns one)
        let choice = response.choices.into_iter().next().unwrap_or_else(|| DeepSeekChoice {
            text: String::new(),
            stop_reason: Some(DeepSeekFinishReason::Stop),
            reasoning_content: None,
        });

        // Extract the main text and clean up any leaked conversation markers
        let mut content = choice.text;

        // Remove trailing "User" that might leak through from the conversation format
        // This can happen when DeepSeek doesn't properly stop at the stop sequence
        // We check if the response ends with "User" (with any whitespace/punctuation before it)
        let trimmed = content.trim();
        if trimmed.ends_with("User") || trimmed.ends_with("User\n") || trimmed.ends_with("User\r\n") {
            // Find the last occurrence of "User" at the end
            if let Some(pos) = trimmed.rfind("User") {
                // Only remove if "User" is at the very end (after trimming)
                let before_user = &trimmed[..pos];
                let after_user = &trimmed[pos + 4..]; // "User" is 4 chars

                // Check if there's nothing substantial after "User"
                if after_user.trim().is_empty() {
                    content = before_user.trim_end().to_string();
                    log::debug!("Cleaned trailing 'User' marker from DeepSeek response");
                }
            }
        }

        // Log reasoning if present
        if let Some(reasoning) = &choice.reasoning_content
            && let Some(text) = &reasoning.reasoning_text
        {
            log::debug!("DeepSeek reasoning trace: {}", text);
        }

        let chat_choice = ChatChoice {
            index: 0,
            message: ChatMessage {
                role: ChatRole::Assistant,
                content,
            },
            finish_reason: choice.stop_reason.map(FinishReason::from).unwrap_or(FinishReason::Stop),
        };

        let usage = response
            .usage
            .map(|u| {
                let input = u.input_tokens.unwrap_or(0);
                let output = u.output_tokens.unwrap_or(0);
                let total = u.total_tokens.unwrap_or(input + output);

                Usage {
                    prompt_tokens: input,
                    completion_tokens: output,
                    total_tokens: total,
                }
            })
            .unwrap_or_else(|| Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            });

        Self {
            id: format!("deepseek-{}", Uuid::new_v4()),
            object: ObjectType::ChatCompletion,
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            model: String::new(), // Will be set by transform_response
            choices: vec![chat_choice],
            usage,
        }
    }
}

// DeepSeek streaming types

/// Streaming chunk from DeepSeek models.
#[derive(Debug, Deserialize)]
pub(crate) struct DeepSeekStreamChunk {
    /// Array of chunk choices.
    pub choices: Vec<DeepSeekStreamChoice>,

    /// Usage statistics (only in final chunk).
    #[serde(default)]
    pub usage: Option<DeepSeekUsage>,
}

/// A single streaming choice from DeepSeek.
#[derive(Debug, Deserialize)]
pub(crate) struct DeepSeekStreamChoice {
    /// The incremental text (DeepSeek uses 'text' directly in streaming).
    pub text: Option<String>,

    /// Delta containing the incremental text (alternative format).
    pub delta: Option<DeepSeekDelta>,

    /// Finish reason (only in final chunk).
    #[serde(rename = "stop_reason")]
    pub finish_reason: Option<DeepSeekFinishReason>,

    /// Index of this choice.
    #[serde(default)]
    pub index: u32,
}

/// Delta content in streaming response.
#[derive(Debug, Deserialize)]
pub(crate) struct DeepSeekDelta {
    /// Incremental text content.
    pub content: Option<String>,

    /// Role (only in first chunk).
    pub role: Option<String>,
}

impl From<DeepSeekStreamChunk> for Option<ChatCompletionChunk> {
    fn from(chunk: DeepSeekStreamChunk) -> Self {
        // Take the first choice
        let choice = chunk.choices.into_iter().next()?;

        // Extract content and role - DeepSeek can use either 'text' directly or 'delta'
        let (mut content, role) = if let Some(text) = choice.text {
            // Direct text format (common in DeepSeek streaming)
            (Some(text), None)
        } else if let Some(delta) = choice.delta {
            // Delta format (OpenAI-style)
            let role = delta.role.and_then(|r| {
                if r == "assistant" {
                    Some(ChatRole::Assistant)
                } else {
                    None
                }
            });
            (delta.content, role)
        } else {
            (None, None)
        };

        // Clean up content if this is a final chunk that might have leaked markers
        // Only do this if we have a finish reason (indicating it's a final chunk)
        if choice.finish_reason.is_some()
            && let Some(ref mut text) = content
        {
            let trimmed = text.trim();
            if trimmed.ends_with("User") || trimmed.ends_with("User\n") || trimmed.ends_with("User\r\n") {
                // Find the last occurrence of "User" at the end
                if let Some(pos) = trimmed.rfind("User") {
                    // Only remove if "User" is at the very end (after trimming)
                    let before_user = &trimmed[..pos];
                    let after_user = &trimmed[pos + 4..]; // "User" is 4 chars

                    // Check if there's nothing substantial after "User"
                    if after_user.trim().is_empty() {
                        *text = before_user.trim_end().to_string();
                        log::debug!("Cleaned trailing 'User' marker from DeepSeek streaming chunk");
                    }
                }
            }
        }

        // Map finish reason
        let finish_reason = choice.finish_reason.map(FinishReason::from);

        // Convert usage if present
        let usage = chunk.usage.map(|u| {
            let input = u.input_tokens.unwrap_or(0);
            let output = u.output_tokens.unwrap_or(0);
            let total = u.total_tokens.unwrap_or(input + output);

            Usage {
                prompt_tokens: input,
                completion_tokens: output,
                total_tokens: total,
            }
        });

        Some(ChatCompletionChunk {
            id: String::new(), // Will be set by caller
            object: ObjectType::ChatCompletionChunk,
            created: 0,           // Will be set by caller
            model: String::new(), // Will be set by caller
            system_fingerprint: None,
            choices: vec![ChatChoiceDelta {
                index: choice.index,
                delta: ChatMessageDelta {
                    role,
                    content,
                    function_call: None,
                    tool_calls: None,
                },
                finish_reason,
                logprobs: None,
            }],
            usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_trailing_user_marker() {
        // Test response with trailing "User" marker (with space)
        let response = DeepSeekResponse {
            choices: vec![DeepSeekChoice {
                text: "Here's my response to your question. User".to_string(),
                stop_reason: Some(DeepSeekFinishReason::Stop),
                reasoning_content: None,
            }],
            usage: None,
        };

        let completion: ChatCompletionResponse = response.into();
        assert_eq!(
            completion.choices[0].message.content,
            "Here's my response to your question."
        );
    }

    #[test]
    fn test_clean_trailing_user_marker_emoji() {
        // Test response with emoji before User
        let response = DeepSeekResponse {
            choices: vec![DeepSeekChoice {
                text: "Hello! I'm here to help. 😊 User".to_string(),
                stop_reason: Some(DeepSeekFinishReason::Stop),
                reasoning_content: None,
            }],
            usage: None,
        };

        let completion: ChatCompletionResponse = response.into();
        assert_eq!(completion.choices[0].message.content, "Hello! I'm here to help. 😊");
    }

    #[test]
    fn test_clean_trailing_user_with_newline() {
        let response = DeepSeekResponse {
            choices: vec![DeepSeekChoice {
                text: "Here's my response.\nUser".to_string(),
                stop_reason: Some(DeepSeekFinishReason::Stop),
                reasoning_content: None,
            }],
            usage: None,
        };

        let completion: ChatCompletionResponse = response.into();
        assert_eq!(completion.choices[0].message.content, "Here's my response.");
    }

    #[test]
    fn test_no_clean_when_user_in_middle() {
        let response = DeepSeekResponse {
            choices: vec![DeepSeekChoice {
                text: "The User asked a question and I answered.".to_string(),
                stop_reason: Some(DeepSeekFinishReason::Stop),
                reasoning_content: None,
            }],
            usage: None,
        };

        let completion: ChatCompletionResponse = response.into();
        assert_eq!(
            completion.choices[0].message.content,
            "The User asked a question and I answered."
        );
    }

    #[test]
    fn test_streaming_chunk_cleanup() {
        // Test final chunk with trailing "User"
        let chunk = DeepSeekStreamChunk {
            choices: vec![DeepSeekStreamChoice {
                text: Some("Final response text. User".to_string()),
                delta: None,
                finish_reason: Some(DeepSeekFinishReason::Stop),
                index: 0,
            }],
            usage: None,
        };

        let completion_chunk: Option<ChatCompletionChunk> = chunk.into();
        assert!(completion_chunk.is_some());
        let chunk = completion_chunk.unwrap();
        assert_eq!(chunk.choices[0].delta.content, Some("Final response text.".to_string()));
    }

    #[test]
    fn test_streaming_chunk_cleanup_with_emoji() {
        // Test final chunk with emoji and trailing "User"
        let chunk = DeepSeekStreamChunk {
            choices: vec![DeepSeekStreamChoice {
                text: Some("How can I help? 😊 User".to_string()),
                delta: None,
                finish_reason: Some(DeepSeekFinishReason::Stop),
                index: 0,
            }],
            usage: None,
        };

        let completion_chunk: Option<ChatCompletionChunk> = chunk.into();
        assert!(completion_chunk.is_some());
        let chunk = completion_chunk.unwrap();
        assert_eq!(chunk.choices[0].delta.content, Some("How can I help? 😊".to_string()));
    }

    #[test]
    fn test_no_cleanup_on_non_final_chunk() {
        // Non-final chunk should not be cleaned even if it contains "User"
        let chunk = DeepSeekStreamChunk {
            choices: vec![DeepSeekStreamChoice {
                text: Some("The User".to_string()),
                delta: None,
                finish_reason: None, // Not a final chunk
                index: 0,
            }],
            usage: None,
        };

        let completion_chunk: Option<ChatCompletionChunk> = chunk.into();
        assert!(completion_chunk.is_some());
        let chunk = completion_chunk.unwrap();
        assert_eq!(chunk.choices[0].delta.content, Some("The User".to_string()));
    }
}
