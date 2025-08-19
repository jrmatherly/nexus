//! Live integration tests for AWS Bedrock.
//!
//! These tests make real API calls to AWS Bedrock and incur charges.
//! They are disabled by default and only run when BEDROCK_LIVE_TESTS=true.
//!
//! To run these tests:
//! ```bash
//! BEDROCK_LIVE_TESTS=true cargo test -p integration-tests bedrock::live -- --ignored
//! ```
//!
//! Required environment variables:
//! - BEDROCK_LIVE_TESTS=true (to enable tests)
//! - AWS credentials (via AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY or AWS_PROFILE)
//! - AWS_REGION (optional, defaults to us-east-1)
//!
//! Cost estimates (per full test run):
//! - Total: < $0.01
//! - Each test uses max_tokens=10 and minimal prompts

use indoc::formatdoc;
use integration_test_macros::live_provider_test;
use integration_tests::TestServer;
use serde_json::json;

/// Helper to create a Bedrock configuration with specified models.
/// All models are configured with max_tokens=10 to minimize costs.
fn create_bedrock_config(models: &[(&str, &str)]) -> String {
    let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());

    let mut model_configs = String::new();
    for (alias, real_model) in models {
        model_configs.push_str(&formatdoc! {r#"
            [llm.providers.bedrock.models.{alias}]
            rename = "{real_model}"
            max_tokens = 10
        "#});
    }

    formatdoc! {r#"
        [llm]
        enabled = true

        [llm.providers.bedrock]
        type = "bedrock"
        region = "{region}"

        {model_configs}
    "#}
}

// ============================================================================
// Anthropic Claude Tests
// ============================================================================

#[live_provider_test(bedrock)]
async fn anthropic_claude_instant_minimal() {
    let config = create_bedrock_config(&[("claude-instant", "anthropic.claude-instant-v1")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    // Minimal prompt to reduce costs
    let response = llm
        .completions(json!({
            "model": "bedrock/claude-instant",
            "messages": [{
                "role": "user",
                "content": "Say yes"
            }],
            "max_tokens": 10,
            "temperature": 0
        }))
        .await;

    // Verify response structure with snapshot
    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[response]",
        ".usage.prompt_tokens" => "[tokens]",
        ".usage.completion_tokens" => "[tokens]",
        ".usage.total_tokens" => "[tokens]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/claude-instant",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[response]"
          },
          "finish_reason": "stop"
        }
      ],
      "usage": {
        "prompt_tokens": "[tokens]",
        "completion_tokens": "[tokens]",
        "total_tokens": "[tokens]"
      }
    }
    "#);
}

// ===== AI21 Jamba Tests =====

#[live_provider_test(bedrock)]
async fn ai21_jamba_1_5_mini_minimal() {
    let config = create_bedrock_config(&[("jamba-mini", "ai21.jamba-1-5-mini-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let response = llm
        .completions(json!({
            "model": "bedrock/jamba-mini",
            "messages": [{
                "role": "user",
                "content": "Reply with one word: yes"
            }],
            "max_tokens": 10,
            "temperature": 0
        }))
        .await;

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[response]",
        ".usage" => "[usage]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/jamba-mini",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[response]"
          },
          "finish_reason": "stop"
        }
      ],
      "usage": "[usage]"
    }
    "#);
}

// ===== Anthropic Claude Tests =====

#[live_provider_test(bedrock)]
async fn anthropic_claude3_sonnet_minimal() {
    let config = create_bedrock_config(&[("claude3-sonnet", "anthropic.claude-3-sonnet-20240229-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let response = llm
        .completions(json!({
            "model": "bedrock/claude3-sonnet",
            "messages": [{
                "role": "user",
                "content": "Reply with one word: yes"
            }],
            "max_tokens": 10,
            "temperature": 0
        }))
        .await;

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[response]",
        ".usage" => "[usage]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/claude3-sonnet",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[response]"
          },
          "finish_reason": "stop"
        }
      ],
      "usage": "[usage]"
    }
    "#);
}

#[live_provider_test(bedrock)]
async fn anthropic_claude3_haiku_minimal() {
    let config = create_bedrock_config(&[("claude3-haiku", "anthropic.claude-3-haiku-20240307-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let response = llm
        .completions(json!({
            "model": "bedrock/claude3-haiku",
            "messages": [{
                "role": "user",
                "content": "Reply: ok"
            }],
            "max_tokens": 10,
            "temperature": 0
        }))
        .await;

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[response]",
        ".usage" => "[usage]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/claude3-haiku",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[response]"
          },
          "finish_reason": "stop"
        }
      ],
      "usage": "[usage]"
    }
    "#);
}

// ============================================================================
// Amazon Nova Tests
// ============================================================================

#[live_provider_test(bedrock)]
async fn amazon_nova_micro_minimal() {
    let config = create_bedrock_config(&[("nova-micro", "amazon.nova-micro-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let response = llm
        .completions(json!({
            "model": "bedrock/nova-micro",
            "messages": [{
                "role": "user",
                "content": "Reply with: hello"
            }],
            "max_tokens": 10,
            "temperature": 0
        }))
        .await;

    // Verify response structure
    let content = response["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(
        content.to_lowercase().contains("hello"),
        "Expected response to contain 'hello', got: {}",
        content
    );

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[response]",
        ".usage" => "[usage]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/nova-micro",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[response]"
          },
          "finish_reason": "stop"
        }
      ],
      "usage": "[usage]"
    }
    "#);
}

// ============================================================================
// Amazon Titan Tests
// ============================================================================

#[live_provider_test(bedrock)]
async fn amazon_titan_lite_minimal() {
    let config = create_bedrock_config(&[("titan-lite", "amazon.titan-text-lite-v1")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let response = llm
        .completions(json!({
            "model": "bedrock/titan-lite",
            "messages": [{
                "role": "user",
                "content": "Hi"
            }],
            "max_tokens": 10,
            "temperature": 0
        }))
        .await;

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[response]",
        ".usage" => "[usage]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/titan-lite",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[response]"
          },
          "finish_reason": "length"
        }
      ],
      "usage": "[usage]"
    }
    "#);
}

#[live_provider_test(bedrock)]
async fn amazon_titan_express_minimal() {
    let config = create_bedrock_config(&[("titan-express", "amazon.titan-text-express-v1")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let response = llm
        .completions(json!({
            "model": "bedrock/titan-express",
            "messages": [{
                "role": "user",
                "content": "Reply with: hello"
            }],
            "max_tokens": 10,
            "temperature": 0
        }))
        .await;

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[response]",
        ".usage" => "[usage]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/titan-express",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[response]"
          },
          "finish_reason": "length"
        }
      ],
      "usage": "[usage]"
    }
    "#);
}

// ============================================================================
// Meta Llama Tests
// ============================================================================

#[live_provider_test(bedrock)]
async fn meta_llama3_8b_minimal() {
    let config = create_bedrock_config(&[("llama3-8b", "meta.llama3-8b-instruct-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let response = llm
        .completions(json!({
            "model": "bedrock/llama3-8b",
            "messages": [{
                "role": "user",
                "content": "2+2="
            }],
            "max_tokens": 10,
            "temperature": 0
        }))
        .await;

    // Verify it's a valid completion and contains "4" in response
    let content = response["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(
        content.contains("4") || content.contains("four"),
        "Expected response to contain '4' or 'four', got: {}",
        content
    );

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[contains_4]",
        ".usage" => "[usage]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/llama3-8b",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[contains_4]"
          },
          "finish_reason": "length"
        }
      ],
      "usage": "[usage]"
    }
    "#);
}

// ============================================================================
// Mistral Tests
// ============================================================================

#[live_provider_test(bedrock)]
async fn mistral_7b_instruct_minimal() {
    let config = create_bedrock_config(&[("mistral-7b", "mistral.mistral-7b-instruct-v0:2")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let response = llm
        .completions(json!({
            "model": "bedrock/mistral-7b",
            "messages": [{
                "role": "user",
                "content": "Yes or no?"
            }],
            "max_tokens": 10,
            "temperature": 0
        }))
        .await;

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[response]",
        ".usage" => "[usage]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/mistral-7b",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[response]"
          },
          "finish_reason": "length"
        }
      ],
      "usage": "[usage]"
    }
    "#);
}

// ============================================================================
// Cohere Tests
// ============================================================================

#[live_provider_test(bedrock)]
async fn cohere_command_r_minimal() {
    let config = create_bedrock_config(&[("command-r", "cohere.command-r-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let response = llm
        .completions(json!({
            "model": "bedrock/command-r",
            "messages": [{
                "role": "user",
                "content": "Hello"
            }],
            "max_tokens": 10,
            "temperature": 0
        }))
        .await;

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[response]",
        ".choices[0].finish_reason" => "[finish_reason]",
        ".usage" => "[usage]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/command-r",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[response]"
          },
          "finish_reason": "[finish_reason]"
        }
      ],
      "usage": "[usage]"
    }
    "#);
}

#[live_provider_test(bedrock)]
async fn cohere_command_r_multi_turn() {
    let config = create_bedrock_config(&[("command-r", "cohere.command-r-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    // Test multi-turn conversation with chat history
    let response = llm
        .completions(json!({
            "model": "bedrock/command-r",
            "messages": [
                {
                    "role": "user",
                    "content": "What is 2+2?"
                },
                {
                    "role": "assistant",
                    "content": "2+2 equals 4."
                },
                {
                    "role": "user",
                    "content": "What about 3+3?"
                }
            ],
            "max_tokens": 50,
            "temperature": 0
        }))
        .await;

    // Verify response acknowledges context
    let content = response["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(
        content.contains("6") || content.contains("six") || content.contains("equals"),
        "Expected response to answer 3+3, got: {}",
        content
    );

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[contextual_response]",
        ".choices[0].finish_reason" => "[finish_reason]",
        ".usage" => "[usage]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/command-r",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[contextual_response]"
          },
          "finish_reason": "[finish_reason]"
        }
      ],
      "usage": "[usage]"
    }
    "#);
}

#[live_provider_test(bedrock)]
async fn cohere_command_r_with_system() {
    let config = create_bedrock_config(&[("command-r", "cohere.command-r-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    // Test system message handling
    let response = llm
        .completions(json!({
            "model": "bedrock/command-r",
            "messages": [
                {
                    "role": "system",
                    "content": "You are a helpful assistant. Be concise."
                },
                {
                    "role": "user",
                    "content": "Hi"
                }
            ],
            "max_tokens": 20,
            "temperature": 0
        }))
        .await;

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[response]",
        ".choices[0].finish_reason" => "[finish_reason]",
        ".usage" => "[usage]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/command-r",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[response]"
          },
          "finish_reason": "[finish_reason]"
        }
      ],
      "usage": "[usage]"
    }
    "#);
}

// ============================================================================
// DeepSeek Tests
// ============================================================================

#[live_provider_test(bedrock)]
async fn deepseek_r1_minimal() {
    let config = create_bedrock_config(&[("r1", "us.deepseek.r1-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    // Minimal prompt to reduce costs
    let response = llm
        .completions(json!({
            "model": "bedrock/r1",
            "messages": [{
                "role": "user",
                "content": "Say yes"
            }],
            "max_tokens": 10,
            "temperature": 0
        }))
        .await;

    // Verify response structure
    assert_eq!(response["object"], "chat.completion");
    assert_eq!(response["model"], "bedrock/r1");

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[response]",
        ".usage.prompt_tokens" => "[tokens]",
        ".usage.completion_tokens" => "[tokens]",
        ".usage.total_tokens" => "[tokens]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/r1",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[response]"
          },
          "finish_reason": "length"
        }
      ],
      "usage": {
        "prompt_tokens": "[tokens]",
        "completion_tokens": "[tokens]",
        "total_tokens": "[tokens]"
      }
    }
    "#);
}

#[live_provider_test(bedrock)]
async fn deepseek_r1_with_system() {
    let config = create_bedrock_config(&[("r1", "us.deepseek.r1-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    // Test system message handling
    let response = llm
        .completions(json!({
            "model": "bedrock/r1",
            "messages": [
                {
                    "role": "system",
                    "content": "You are a helpful assistant that answers in exactly one word."
                },
                {
                    "role": "user",
                    "content": "What is 2+2?"
                }
            ],
            "max_tokens": 10,
            "temperature": 0
        }))
        .await;

    // Just verify response structure
    assert_eq!(response["object"], "chat.completion");
    assert_eq!(response["model"], "bedrock/r1");

    insta::assert_json_snapshot!(response, {
        ".id" => "[id]",
        ".created" => "[timestamp]",
        ".choices[0].message.content" => "[response]",
        ".choices[0].finish_reason" => "[finish_reason]",
        ".usage" => "[usage]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[timestamp]",
      "model": "bedrock/r1",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "[response]"
          },
          "finish_reason": "[finish_reason]"
        }
      ],
      "usage": "[usage]"
    }
    "#);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[live_provider_test(bedrock)]
async fn error_invalid_model() {
    let config = create_bedrock_config(&[("invalid", "anthropic.invalid-model-v99")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let response = llm
        .completions_error(json!({
            "model": "bedrock/invalid",
            "messages": [{
                "role": "user",
                "content": "test"
            }],
            "max_tokens": 10
        }))
        .await;

    assert_eq!(response.status(), 400);
    let error = response.json::<serde_json::Value>().await.unwrap();

    // AWS SDK should return a ValidationException or service error - verify it's there
    let error_msg = error["error"]["message"].as_str().unwrap();
    assert!(
        error_msg.contains("ValidationException")
            || error_msg.contains("does not exist")
            || error_msg.contains("Invalid model")
            || error_msg.contains("service error")
            || error_msg.contains("Invalid request"),
        "Expected validation error, got: {}",
        error_msg
    );

    // Use snapshot for the error structure
    insta::assert_json_snapshot!(error, {
        ".error.message" => "[validation_error]",
        ".error.type" => "[error_type]",
        ".error.code" => "[code]"
    }, @r#"
    {
      "error": {
        "message": "[validation_error]",
        "type": "[error_type]",
        "code": "[code]"
      }
    }
    "#);
}

#[live_provider_test(bedrock)]
async fn error_missing_required_field() {
    let config = create_bedrock_config(&[("claude", "anthropic.claude-instant-v1")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    // Missing messages field
    let response = llm
        .completions_error(json!({
            "model": "bedrock/claude",
            "max_tokens": 10
        }))
        .await;

    assert_eq!(response.status(), 422);
    let error = response.text().await.unwrap();

    // Axum returns a text error for 422 (unprocessable entity) on deserialization failures
    assert!(error.contains("missing field") || error.contains("Failed to deserialize"));
}

// ============================================================================
// Streaming Tests
// ============================================================================

#[live_provider_test(bedrock)]
async fn streaming_claude_instant() {
    let config = create_bedrock_config(&[("claude-instant", "anthropic.claude-instant-v1")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let request = json!({
        "model": "bedrock/claude-instant",
        "messages": [{
            "role": "user",
            "content": "Say hello"
        }],
        "max_tokens": 10,
        "stream": true,
        "temperature": 0
    });

    // Test that we can stream content successfully
    let content = llm.stream_completions_content(request.clone()).await;
    assert!(!content.is_empty(), "Expected streaming content");

    // Also verify chunks are valid
    let chunks = llm.stream_completions(request).await;
    assert!(chunks.len() > 0, "Expected stream chunks, got none");

    // Verify first chunk structure
    let first_chunk = &chunks[0];
    assert_eq!(first_chunk["object"], "chat.completion.chunk");
    assert_eq!(first_chunk["model"], "bedrock/claude-instant");

    // Don't check for finish_reason as it may not always be present
    // The important thing is that streaming works and produces content
}

#[live_provider_test(bedrock)]
async fn streaming_cohere_command_r_with_helpers() {
    let config = create_bedrock_config(&[("command-r", "cohere.command-r-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let request = json!({
        "model": "bedrock/command-r",
        "messages": [{
            "role": "user",
            "content": "Say hello"
        }],
        "max_tokens": 20,
        "stream": true,
        "temperature": 0
    });

    let chunks = llm.stream_completions(request).await;

    // Should have at least one chunk
    assert!(chunks.len() > 0, "Expected at least one chunk, got {}", chunks.len());

    // Verify first chunk structure
    let first_chunk = &chunks[0];
    assert_eq!(first_chunk["object"], "chat.completion.chunk");
    assert_eq!(first_chunk["model"], "bedrock/command-r");
    assert!(first_chunk["choices"][0]["delta"].is_object());

    // Verify at least one chunk has finish_reason
    let has_finish_reason = chunks
        .iter()
        .any(|chunk| chunk["choices"][0]["finish_reason"].is_string());
    assert!(has_finish_reason, "Expected at least one chunk with finish_reason");
}

#[live_provider_test(bedrock)]
async fn streaming_cohere_command_r_multi_turn_with_helpers() {
    let config = create_bedrock_config(&[("command-r", "cohere.command-r-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    // Test streaming with conversation history
    let request = json!({
        "model": "bedrock/command-r",
        "messages": [
            {"role": "user", "content": "Count from 1 to 3"},
            {"role": "assistant", "content": "1, 2, 3"},
            {"role": "user", "content": "Now count from 4 to 6"}
        ],
        "max_tokens": 30,
        "stream": true,
        "temperature": 0
    });

    let chunks = llm.stream_completions(request).await;
    assert!(
        chunks.len() > 0,
        "Expected at least one chunk for multi-turn conversation"
    );

    // Verify first chunk structure
    let first_chunk = &chunks[0];
    assert_eq!(first_chunk["object"], "chat.completion.chunk");
    assert_eq!(first_chunk["model"], "bedrock/command-r");
    assert!(first_chunk["choices"][0]["delta"].is_object());

    // Verify last chunk has finish_reason
    let last_chunk = chunks.last().unwrap();
    assert!(last_chunk["choices"][0]["finish_reason"].is_string());
}

#[live_provider_test(bedrock)]
async fn streaming_nova_micro() {
    let config = create_bedrock_config(&[("nova-micro", "amazon.nova-micro-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let request = json!({
        "model": "bedrock/nova-micro",
        "messages": [{
            "role": "user",
            "content": "Count to 3"
        }],
        "max_tokens": 15,
        "stream": true,
        "temperature": 0
    });

    let chunks = llm.stream_completions(request).await;

    // Should have chunks
    assert!(chunks.len() > 0, "Expected stream chunks, got none");

    // Verify first chunk structure
    let first_chunk = &chunks[0];
    assert_eq!(first_chunk["object"], "chat.completion.chunk");
    assert_eq!(first_chunk["model"], "bedrock/nova-micro");
    assert!(first_chunk["choices"][0]["delta"].is_object());

    // Verify at least one chunk has finish_reason
    let has_finish_reason = chunks
        .iter()
        .any(|chunk| chunk["choices"][0]["finish_reason"].is_string());
    assert!(has_finish_reason, "Expected at least one chunk with finish_reason");
}

#[live_provider_test(bedrock)]
async fn streaming_titan_express() {
    let config = create_bedrock_config(&[("titan-express", "amazon.titan-text-express-v1")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let request = json!({
        "model": "bedrock/titan-express",
        "messages": [{
            "role": "user",
            "content": "Count to 3"
        }],
        "max_tokens": 15,
        "stream": true,
        "temperature": 0
    });

    let chunks = llm.stream_completions(request).await;

    // Should have chunks
    assert!(chunks.len() > 0, "Expected stream chunks, got none");

    // Verify first chunk structure
    let first_chunk = &chunks[0];
    assert_eq!(first_chunk["object"], "chat.completion.chunk");
    assert_eq!(first_chunk["model"], "bedrock/titan-express");
    assert!(first_chunk["choices"][0]["delta"].is_object());

    // Verify at least one chunk has finish_reason
    let has_finish_reason = chunks
        .iter()
        .any(|chunk| chunk["choices"][0]["finish_reason"].is_string());
    assert!(has_finish_reason, "Expected at least one chunk with finish_reason");
}

#[live_provider_test(bedrock)]
async fn streaming_deepseek_r1() {
    let config = create_bedrock_config(&[("r1", "us.deepseek.r1-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let request = json!({
        "model": "bedrock/r1",
        "messages": [{
            "role": "user",
            "content": "Count to 3"
        }],
        "max_tokens": 15,
        "stream": true,
        "temperature": 0
    });

    // Test streaming chunks
    let chunks = llm.stream_completions(request.clone()).await;
    assert!(!chunks.is_empty(), "Expected stream chunks, got none");

    // Verify first chunk structure with snapshot
    insta::assert_json_snapshot!(chunks[0], {
        ".id" => "[id]",
        ".created" => "[created]",
        ".choices[0].delta" => "[delta]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion.chunk",
      "created": "[created]",
      "model": "bedrock/r1",
      "choices": [
        {
          "index": 0,
          "delta": "[delta]"
        }
      ]
    }
    "#);

    // Test that content accumulation works
    let content = llm.stream_completions_content(request).await;
    assert!(!content.is_empty(), "Expected non-empty streaming content");
}

#[live_provider_test(bedrock)]
async fn streaming_ai21_jamba_mini() {
    let config = create_bedrock_config(&[("jamba-mini", "ai21.jamba-1-5-mini-v1:0")]);

    let server = TestServer::builder().build(&config).await;
    let llm = server.llm_client("/llm");

    let request = json!({
        "model": "bedrock/jamba-mini",
        "messages": [{
            "role": "user",
            "content": "Count to 3"
        }],
        "max_tokens": 15,
        "stream": true,
        "temperature": 0
    });

    // Test streaming chunks
    let chunks = llm.stream_completions(request.clone()).await;
    assert!(!chunks.is_empty(), "Expected stream chunks, got none");

    // Verify first chunk structure with snapshot
    // Note: AI21 Jamba includes usage stats in the first chunk
    insta::assert_json_snapshot!(chunks[0], {
        ".id" => "[id]",
        ".created" => "[created]",
        ".choices[0].delta.content" => "[content]",
        ".choices[0].delta.role" => "[role]",
        ".usage.prompt_tokens" => "[tokens]",
        ".usage.completion_tokens" => "[tokens]",
        ".usage.total_tokens" => "[tokens]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion.chunk",
      "created": "[created]",
      "model": "bedrock/jamba-mini",
      "choices": [
        {
          "index": 0,
          "delta": {
            "role": "[role]",
            "content": "[content]"
          }
        }
      ],
      "usage": {
        "prompt_tokens": "[tokens]",
        "completion_tokens": "[tokens]",
        "total_tokens": "[tokens]"
      }
    }
    "#);

    // Test that content accumulation works
    let content = llm.stream_completions_content(request).await;
    assert!(!content.is_empty(), "Expected non-empty streaming content");
}
