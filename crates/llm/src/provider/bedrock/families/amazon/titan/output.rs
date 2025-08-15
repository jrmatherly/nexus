//! Amazon Titan output types for AWS Bedrock.
//!
//! This module contains response types for Amazon Titan models, including both
//! standard (non-streaming) and streaming response formats. Titan models return
//! responses in a specific format that differs from other model families.
//!
//! # Response Structure
//! Titan models return responses with:
//! - Token usage statistics (input and output token counts)
//! - Generated text with completion metadata
//! - Completion reasons indicating why generation stopped
//! - Support for streaming responses with partial results
//!
//! # Completion Reasons
//! - `FINISH`: Natural completion of the response
//! - `LENGTH`: Maximum token limit reached
//! - `STOP_CRITERIA_MET`: Stop sequence encountered
//! - `CONTENT_FILTERED`: Content filtered by safety mechanisms
//!
//! # Official Documentation
//! - [Titan Response Format](https://docs.aws.amazon.com/bedrock/latest/userguide/titan-text-models.html#titan-text-response-format)
//! - [Streaming Response Format](https://docs.aws.amazon.com/bedrock/latest/userguide/model-streaming.html)

use serde::Deserialize;

use crate::messages::{
    ChatChoice, ChatChoiceDelta, ChatCompletionChunk, ChatCompletionResponse, ChatMessage, ChatMessageDelta, ChatRole,
    FinishReason, ObjectType, Usage,
};

/// Completion reasons returned by Amazon Titan models.
///
/// These values indicate why the model stopped generating text. Understanding the
/// completion reason is important for handling different scenarios in your application.
///
/// # Serialization
/// Values are serialized in `SCREAMING_SNAKE_CASE` format as returned by the Titan API.
/// The `#[serde(untagged)]` attribute on `Other` ensures forward compatibility with
/// future completion reasons that may be added by AWS.
///
/// # Official Values
/// Based on AWS Bedrock documentation for Titan models.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum TitanCompletionReason {
    /// The model completed its response naturally.
    ///
    /// This indicates the model determined it had finished its response based on
    /// the context and conversation flow. This is the most common completion reason
    /// for well-formed requests and typically indicates a complete, coherent response.
    ///
    /// **Action**: Response is complete and ready to use.
    Finish,

    /// The maximum token limit was reached.
    ///
    /// The model hit the `maxTokenCount` specified in the request configuration
    /// before naturally completing its response. The response may be truncated
    /// and incomplete.
    ///
    /// **Action**: Consider increasing `maxTokenCount` if you need longer responses,
    /// or inform the user that the response was truncated.
    Length,

    /// A stop sequence was encountered.
    ///
    /// The model generated one of the strings specified in the `stopSequences`
    /// parameter and stopped generation. The stop sequence itself is not included
    /// in the response text.
    ///
    /// **Action**: Response is likely complete up to the intended stopping point.
    /// This is normal behavior when using stop sequences for formatting control.
    #[serde(rename = "STOP_CRITERIA_MET")]
    StopCriteriaMet,

    /// Content was filtered by safety mechanisms.
    ///
    /// Titan's built-in safety filters detected and blocked content that violates
    /// AWS's responsible AI policies. This can happen for both input prompts and
    /// generated responses.
    ///
    /// **Action**: Review the prompt for potentially problematic content, rephrase
    /// the request, or inform the user that the content was filtered for safety.
    #[serde(rename = "CONTENT_FILTERED")]
    ContentFiltered,

    /// Unknown completion reason for forward compatibility.
    ///
    /// Captures any completion reason not explicitly handled above. This ensures
    /// the code continues to work if AWS adds new completion reasons in the future.
    ///
    /// **Action**: Log the unknown reason for investigation and treat as a generic
    /// completion case. Consider updating the code if this becomes a known reason.
    #[serde(untagged)]
    Other(String),
}

impl From<TitanCompletionReason> for FinishReason {
    fn from(reason: TitanCompletionReason) -> Self {
        match reason {
            TitanCompletionReason::Finish | TitanCompletionReason::StopCriteriaMet => FinishReason::Stop,
            TitanCompletionReason::Length => FinishReason::Length,
            TitanCompletionReason::ContentFiltered => FinishReason::ContentFilter,
            TitanCompletionReason::Other(s) => {
                log::warn!("Unknown completion reason from Bedrock Titan: {s}");
                FinishReason::Other(s)
            }
        }
    }
}

/// Complete response structure returned by Amazon Titan models.
///
/// This is the top-level response object for non-streaming requests to Titan models.
/// It includes token usage statistics and an array of generated results. Typically,
/// Titan returns a single result, but the API design allows for multiple results.
///
/// # Response Format
/// ```json
/// {
///   "inputTextTokenCount": 15,
///   "results": [{
///     "tokenCount": 42,
///     "outputText": "The generated response text...",
///     "completionReason": "FINISH"
///   }]
/// }
/// ```
///
/// # Usage Statistics
/// Token counts are provided for both input and output, allowing for accurate
/// usage tracking and billing calculations. These counts are based on Titan's
/// internal tokenization, which may differ slightly from other tokenizers.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TitanResponse {
    /// Number of tokens in the input prompt.
    ///
    /// This count includes all tokens from the `inputText` field in the request,
    /// including role prefixes, conversation history, and formatting. This is
    /// useful for tracking input costs and understanding token consumption patterns.
    ///
    /// **Note**: Token counting is model-specific and may vary between different
    /// Titan model versions.
    pub input_text_token_count: u32,

    /// Array of generated results.
    ///
    /// While the array structure allows for multiple results, Titan models
    /// typically return exactly one result per request. Each result contains
    /// the generated text, token count, and completion reason.
    ///
    /// **Note**: If multiple results were supported, they would represent
    /// alternative completions, but current Titan models generate only one.
    pub results: Vec<TitanResult>,
}

/// Individual result within a Titan response.
///
/// Each result represents a single generated completion with its associated
/// metadata. This structure contains the actual generated text along with
/// information about why generation stopped and how many tokens were used.
///
/// # Token Counting
/// The `tokenCount` represents only the generated tokens, not including the
/// input prompt. Combined with `inputTextTokenCount` from the parent response,
/// this gives you complete usage information.
///
/// # Content Safety
/// The generated text has already been processed through Titan's safety filters.
/// If content was filtered, this will be indicated in the `completionReason`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TitanResult {
    /// Number of tokens in the generated text.
    ///
    /// This count includes only the tokens in `outputText`, excluding the input
    /// prompt. Use this for tracking generation costs and understanding response
    /// length in token terms.
    ///
    /// **Billing Note**: Both input and output tokens are typically charged,
    /// often at different rates. Check current AWS Bedrock pricing for details.
    token_count: u32,

    /// The generated text response.
    ///
    /// This is the primary result of the generation request - the text that the
    /// model generated in response to your input prompt. The text does not include
    /// any role prefixes or formatting that was in the input prompt.
    ///
    /// **Content**: May be truncated if generation hit token limits or stop sequences.
    /// Check `completionReason` to understand if the response is complete.
    output_text: String,

    /// Reason why text generation stopped.
    ///
    /// This optional field indicates why the model stopped generating text.
    /// Common reasons include natural completion (`FINISH`), hitting token limits
    /// (`LENGTH`), or encountering stop sequences (`STOP_CRITERIA_MET`).
    ///
    /// **Handling**: If `None`, assume natural completion. Always check this
    /// value to handle edge cases like content filtering or truncation.
    completion_reason: Option<TitanCompletionReason>,
}

impl From<TitanResult> for ChatChoice {
    fn from(result: TitanResult) -> Self {
        let finish_reason = result
            .completion_reason
            .map(FinishReason::from)
            .unwrap_or(FinishReason::Stop);

        Self {
            index: 0,
            message: ChatMessage {
                role: ChatRole::Assistant,
                content: result.output_text,
            },
            finish_reason,
        }
    }
}

impl From<TitanResponse> for ChatCompletionResponse {
    fn from(response: TitanResponse) -> Self {
        let first_result = response.results.into_iter().next().unwrap_or_else(|| {
            log::error!("No results in Titan response, creating error result");
            TitanResult {
                token_count: 0,
                output_text: "Error: No results in response".to_string(),
                completion_reason: Some(TitanCompletionReason::Other("ERROR".to_string())),
            }
        });

        let usage = Usage {
            prompt_tokens: response.input_text_token_count,
            completion_tokens: first_result.token_count,
            total_tokens: response.input_text_token_count + first_result.token_count,
        };

        let choice = ChatChoice::from(first_result);

        ChatCompletionResponse {
            id: format!("titan-{}", uuid::Uuid::new_v4()),
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

// Amazon Titan streaming types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TitanStreamChunk {
    output_text: Option<String>,
    completion_reason: Option<TitanCompletionReason>,
    #[allow(dead_code)]
    total_output_text_token_count: Option<u32>,
}

impl From<TitanStreamChunk> for Option<ChatCompletionChunk> {
    fn from(chunk: TitanStreamChunk) -> Self {
        let finish_reason = chunk.completion_reason.map(Into::into);

        if chunk.output_text.is_some() || finish_reason.is_some() {
            Some(ChatCompletionChunk {
                id: String::new(), // Will be set by caller
                object: ObjectType::ChatCompletionChunk,
                created: 0,           // Will be set by caller
                model: String::new(), // Will be set by caller
                system_fingerprint: None,
                choices: vec![ChatChoiceDelta {
                    index: 0,
                    delta: ChatMessageDelta {
                        role: chunk.output_text.as_ref().map(|_| ChatRole::Assistant),
                        content: chunk.output_text,
                        function_call: None,
                        tool_calls: None,
                    },
                    finish_reason,
                    logprobs: None,
                }],
                usage: None, // Titan doesn't provide incremental usage in streaming
            })
        } else {
            None
        }
    }
}
