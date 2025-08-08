use integration_tests::{TestServer, llms::GoogleMock};
use serde_json::json;

#[tokio::test]
async fn list_models() {
    let mut builder = TestServer::builder();
    builder.spawn_llm(GoogleMock::new("google")).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");
    let body = llm.list_models().await;

    insta::assert_json_snapshot!(body, @r#"
    {
      "object": "list",
      "data": [
        {
          "id": "google/gemini-1.5-flash",
          "object": "model",
          "created": 0,
          "owned_by": "google"
        },
        {
          "id": "google/gemini-1.5-pro",
          "object": "model",
          "created": 0,
          "owned_by": "google"
        },
        {
          "id": "google/gemini-pro",
          "object": "model",
          "created": 0,
          "owned_by": "google"
        }
      ]
    }
    "#);
}

#[tokio::test]
async fn chat_completion() {
    let mut builder = TestServer::builder();
    builder.spawn_llm(GoogleMock::new("google")).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");

    let request = json!({
        "model": "google/gemini-1.5-flash",
        "messages": [
            {
                "role": "system",
                "content": "You are a helpful assistant"
            },
            {
                "role": "user",
                "content": "Hello!"
            }
        ],
        "temperature": 0.7,
        "max_tokens": 100
    });

    let body = llm.completions(request).await;

    insta::assert_json_snapshot!(body, {
        ".id" => "[id]",
        ".created" => "[created]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[created]",
      "model": "google/gemini-1.5-flash",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "Hello! I'm Gemini, a test assistant. How can I help you today?"
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
async fn handles_system_messages() {
    let mut builder = TestServer::builder();
    builder.spawn_llm(GoogleMock::new("google")).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");

    // Test with system message
    let request = json!({
        "model": "google/gemini-1.5-pro",
        "messages": [
            {
                "role": "system",
                "content": "You are a creative writing assistant. Always respond in a poetic style."
            },
            {
                "role": "user",
                "content": "How are you?"
            }
        ]
    });

    let body = llm.completions(request).await;

    insta::assert_json_snapshot!(body, {
        ".id" => "[id]",
        ".created" => "[created]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[created]",
      "model": "google/gemini-1.5-pro",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "Test response to: How are you?"
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
async fn rejects_streaming() {
    let mut builder = TestServer::builder();
    builder.spawn_llm(GoogleMock::new("google")).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");

    let request = json!({
        "model": "google/gemini-1.5-flash",
        "messages": [
            {
                "role": "user",
                "content": "Hello!"
            }
        ],
        "stream": true
    });

    let response = llm.completions_raw(request).await;
    let status = response.status();
    let body = response.text().await.unwrap();

    // Should return 400 because streaming is an invalid request (not supported)
    assert_eq!(status, 400);
    insta::assert_snapshot!(body, @r#"{"error":{"message":"Streaming is not yet supported. Please set stream=false or omit the parameter.","type":"invalid_request_error","code":400}}"#);
}

#[tokio::test]
async fn simple_completion() {
    let mut builder = TestServer::builder();
    builder.spawn_llm(GoogleMock::new("google")).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");

    let body = llm.simple_completion("google/gemini-pro", "Quick test").await;

    insta::assert_json_snapshot!(body, {
        ".id" => "[id]",
        ".created" => "[created]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[created]",
      "model": "google/gemini-pro",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "Test response to: Quick test"
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
async fn with_parameters() {
    let mut builder = TestServer::builder();
    builder.spawn_llm(GoogleMock::new("google")).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");

    // Test with various Google-compatible parameters
    let request = json!({
        "model": "google/gemini-1.5-flash",
        "messages": [
            {
                "role": "user",
                "content": "Test with parameters"
            }
        ],
        "temperature": 1.8,
        "max_tokens": 200,
        "top_p": 0.95,
        "top_k": 40,
        "stop": ["\\n\\n", "END"]
    });

    let body = llm.completions(request).await;

    insta::assert_json_snapshot!(body, {
        ".id" => "[id]",
        ".created" => "[created]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[created]",
      "model": "google/gemini-1.5-flash",
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
async fn custom_models() {
    let mut builder = TestServer::builder();
    let models = vec!["gemini-experimental".to_string(), "gemini-custom".to_string()];

    let mock = GoogleMock::new("google").with_models(models);
    builder.spawn_llm(mock).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");
    let body = llm.list_models().await;

    insta::assert_json_snapshot!(body, @r#"
    {
      "object": "list",
      "data": [
        {
          "id": "google/gemini-experimental",
          "object": "model",
          "created": 0,
          "owned_by": "google"
        },
        {
          "id": "google/gemini-custom",
          "object": "model",
          "created": 0,
          "owned_by": "google"
        }
      ]
    }
    "#);
}

#[tokio::test]
async fn custom_response() {
    let mut builder = TestServer::builder();

    let mock = GoogleMock::new("google").with_response("special trigger", "This is a special custom response");
    builder.spawn_llm(mock).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");

    let request = json!({
        "model": "google/gemini-1.5-flash",
        "messages": [
            {
                "role": "user",
                "content": "Please respond with special trigger in your reply"
            }
        ]
    });

    let body = llm.completions(request).await;

    insta::assert_json_snapshot!(body, {
        ".id" => "[id]",
        ".created" => "[created]"
    }, @r#"
    {
      "id": "[id]",
      "object": "chat.completion",
      "created": "[created]",
      "model": "google/gemini-1.5-flash",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "This is a special custom response"
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
