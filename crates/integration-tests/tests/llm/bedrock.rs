mod live;

use integration_tests::{
    TestServer,
    llms::{BedrockMock, ModelConfig},
};

#[tokio::test]
async fn bedrock_list_models() {
    let mock = BedrockMock::new("bedrock")
        .with_models(vec![
            "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
            "amazon.titan-text-express-v1".to_string(),
            "deepseek.r1-v1:0".to_string(),
        ])
        .with_model_configs(vec![
            ModelConfig::new("claude-3-sonnet").with_rename("anthropic.claude-3-sonnet-20240229-v1:0"),
            ModelConfig::new("titan-express").with_rename("amazon.titan-text-express-v1"),
            ModelConfig::new("r1").with_rename("deepseek.r1-v1:0"),
        ]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(mock).await;

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
          "id": "bedrock/claude-3-sonnet",
          "object": "model",
          "created": "[created]",
          "owned_by": "bedrock"
        },
        {
          "id": "bedrock/r1",
          "object": "model",
          "created": "[created]",
          "owned_by": "bedrock"
        },
        {
          "id": "bedrock/titan-express",
          "object": "model",
          "created": "[created]",
          "owned_by": "bedrock"
        }
      ]
    }
    "#);
}

#[tokio::test]
async fn bedrock_deepseek_chat_completion() {
    let mock = BedrockMock::new("bedrock")
        .with_models(vec!["deepseek.r1-v1:0".to_string()])
        .with_model_configs(vec![ModelConfig::new("r1").with_rename("deepseek.r1-v1:0")])
        // DeepSeek formats messages as "User: <content>\nAssistant:"
        .with_response("User: What is 2+2?\nAssistant:", "The answer is 4.");

    let mut builder = TestServer::builder();
    builder.spawn_llm(mock).await;

    let server = builder.build("").await;
    let llm = server.llm_client("/llm");
    let body = llm.simple_completion("bedrock/r1", "What is 2+2?").await;

    insta::assert_json_snapshot!(body, {
        ".id" => "[uuid]",
        ".created" => "[created]"
    }, @r#"
    {
      "id": "[uuid]",
      "object": "chat.completion",
      "created": "[created]",
      "model": "bedrock/r1",
      "choices": [
        {
          "index": 0,
          "message": {
            "role": "assistant",
            "content": "The answer is 4."
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
