use indoc::indoc;
use integration_tests::{TestServer, llms::OpenAIMock};
use serde_json::json;

#[tokio::test]
async fn list_models() {
    let mut builder = TestServer::builder();
    builder.spawn_llm(OpenAIMock::new("test_openai")).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");
    let body = llm.list_models().await;

    insta::assert_json_snapshot!(body, {
        ".data[].created" => "[created]"
    }, @r#"
    {
      "object": "list",
      "data": [
        {
          "id": "test_openai/gpt-3.5-turbo",
          "object": "model",
          "created": "[created]",
          "owned_by": "openai"
        },
        {
          "id": "test_openai/gpt-4",
          "object": "model",
          "created": "[created]",
          "owned_by": "openai"
        },
        {
          "id": "test_openai/gpt-4-turbo",
          "object": "model",
          "created": "[created]",
          "owned_by": "openai"
        }
      ]
    }
    "#);
}

#[tokio::test]
async fn custom_path() {
    let config = indoc! {r#"
        [llm]
        path = "/custom"
    "#};

    let mut builder = TestServer::builder();
    builder.spawn_llm(OpenAIMock::new("test_openai")).await;

    let server = builder.build(config).await;
    let llm = server.llm_client("/custom");
    let body = llm.list_models().await;

    insta::assert_json_snapshot!(body, {
        ".data[].created" => "[created]"
    }, @r#"
    {
      "object": "list",
      "data": [
        {
          "id": "test_openai/gpt-3.5-turbo",
          "object": "model",
          "created": "[created]",
          "owned_by": "openai"
        },
        {
          "id": "test_openai/gpt-4",
          "object": "model",
          "created": "[created]",
          "owned_by": "openai"
        },
        {
          "id": "test_openai/gpt-4-turbo",
          "object": "model",
          "created": "[created]",
          "owned_by": "openai"
        }
      ]
    }
    "#);
}

#[tokio::test]
async fn chat_completions() {
    let mut builder = TestServer::builder();
    builder.spawn_llm(OpenAIMock::new("test_openai")).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");

    let request = json!({
        "model": "test_openai/gpt-3.5-turbo",
        "messages": [
            {
                "role": "user",
                "content": "Hello!"
            }
        ]
    });

    let body = llm.completions(request).await;

    insta::assert_json_snapshot!(body, {
        ".id" => "chatcmpl-test-[uuid]"
    }, @r#"
    {
      "id": "chatcmpl-test-[uuid]",
      "object": "chat.completion",
      "created": 1677651200,
      "model": "test_openai/gpt-3.5-turbo",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "Hello! I'm a test LLM assistant. How can I help you today?"
          },
          "finish_reason": "stop"
        }
      ],
      "usage": {
        "prompt_tokens": 10,
        "completion_tokens": 15,
        "total_tokens": 25
      }
    }
    "#);
}

#[tokio::test]
async fn chat_completions_simple() {
    let mut builder = TestServer::builder();
    builder.spawn_llm(OpenAIMock::new("test_openai")).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");
    let body = llm.simple_completion("test_openai/gpt-3.5-turbo", "Hello!").await;

    insta::assert_json_snapshot!(body, {
        ".id" => "chatcmpl-test-[uuid]"
    }, @r#"
    {
      "id": "chatcmpl-test-[uuid]",
      "object": "chat.completion",
      "created": 1677651200,
      "model": "test_openai/gpt-3.5-turbo",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "Hello! I'm a test LLM assistant. How can I help you today?"
          },
          "finish_reason": "stop"
        }
      ],
      "usage": {
        "prompt_tokens": 10,
        "completion_tokens": 15,
        "total_tokens": 25
      }
    }
    "#);
}

#[tokio::test]
async fn chat_completions_with_parameters() {
    let mut builder = TestServer::builder();
    builder.spawn_llm(OpenAIMock::new("test_openai")).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");

    let request = json!({
        "model": "test_openai/gpt-3.5-turbo",
        "messages": [
            {
                "role": "user",
                "content": "Test message"
            }
        ],
        "temperature": 1.8,
        "max_tokens": 100,
        "top_p": 0.9,
        "frequency_penalty": 0.5,
        "presence_penalty": 0.3,
        "stop": ["\\n", "END"]
    });

    let body = llm.completions(request).await;

    insta::assert_json_snapshot!(body, {
        ".id" => "chatcmpl-test-[uuid]"
    }, @r#"
    {
      "id": "chatcmpl-test-[uuid]",
      "object": "chat.completion",
      "created": 1677651200,
      "model": "test_openai/gpt-3.5-turbo",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "This is a creative response due to high temperature"
          },
          "finish_reason": "stop"
        }
      ],
      "usage": {
        "prompt_tokens": 10,
        "completion_tokens": 15,
        "total_tokens": 25
      }
    }
    "#);
}

#[tokio::test]
async fn chat_completions_streaming_not_supported() {
    let mut builder = TestServer::builder();
    builder.spawn_llm(OpenAIMock::new("test_openai")).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");

    let request = json!({
        "model": "test_openai/gpt-3.5-turbo",
        "messages": [
            {
                "role": "user",
                "content": "Test message"
            }
        ],
        "stream": true
    });

    let response = llm.completions_raw(request).await;

    // Should return 400 because streaming is an invalid request (not supported)
    let status = response.status();
    let body = response.text().await.unwrap();

    assert_eq!(status, 400, "Expected 400 status for streaming request");
    assert!(
        body.contains("Streaming is not yet supported"),
        "Expected error message about streaming not supported"
    );
}
