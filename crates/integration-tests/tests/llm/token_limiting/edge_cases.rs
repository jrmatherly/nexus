//! Edge case tests for token-based rate limiting.

use indoc::indoc;
use integration_tests::{TestServer, llms::OpenAIMock};
use serde_json::json;

/// Test edge cases: empty string client_id is accepted.
#[tokio::test]
async fn empty_client_id_accepted() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 1000
        interval = "60s"

        [server.client_identification]
        enabled = true
        client_id = { http_header = "X-Client-Id" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 10
    });

    // Empty string client ID is treated as a valid (though strange) client ID
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "")
        .send()
        .await
        .unwrap();

    // Empty string is a valid client ID, just unusual
    assert_eq!(response.status(), 200, "Empty string client ID should be accepted");
}

/// Test edge cases: whitespace-only client_id.
#[tokio::test]
async fn whitespace_client_id_accepted() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 1000
        interval = "60s"

        [server.client_identification]
        enabled = true
        client_id = { http_header = "X-Client-Id" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 10
    });

    // Whitespace-only client ID (spaces only, as tabs/newlines are invalid in headers)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "     ")
        .send()
        .await
        .unwrap();

    // Whitespace-only is also treated as a valid client ID
    assert_eq!(response.status(), 200, "Whitespace client ID should be accepted");
}

/// Test edge cases: very long client_id and group_id.
#[tokio::test]
async fn very_long_identifiers() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 1000
        interval = "60s"

        [llm.providers.openai.rate_limits.per_user.groups.enterprise]
        limit = 5000
        interval = "60s"

        [server.client_identification]
        enabled = true
        allowed_groups = ["enterprise"]
        client_id = { http_header = "X-Client-Id" }
        group_id = { http_header = "X-Group" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 100
    });

    // Very long identifiers (256 characters)
    let long_client_id = "a".repeat(256);

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", &long_client_id)
        .header("X-Group", "enterprise")
        .send()
        .await
        .unwrap();

    // Should work with long identifiers
    assert_eq!(response.status(), 200, "Long client ID should be accepted");
}

/// Test edge cases: special characters in identifiers.
#[tokio::test]
async fn special_characters_in_identifiers() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 1000
        interval = "60s"

        [server.client_identification]
        enabled = true
        client_id = { http_header = "X-Client-Id" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 100
    });

    // Special characters in client ID
    let special_client_id = "user@example.com:123-456_789/test";

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", special_client_id)
        .send()
        .await
        .unwrap();

    // Should work with special characters
    assert_eq!(
        response.status(),
        200,
        "Special characters in client ID should be accepted"
    );
}

/// Test error response for rate limit exceeded.
#[tokio::test]
async fn rate_limit_exceeded_response_format() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 50
        interval = "60s"

        [server.client_identification]
        enabled = true
        client_id = { http_header = "X-Client-Id" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 60  // Exceeds limit of 50
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "rate-limit-test")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 429);

    // No Retry-After headers are sent to maintain consistency with downstream LLM providers
    assert!(!response.headers().contains_key("retry-after"));

    let body = response.json::<serde_json::Value>().await.unwrap();
    insta::assert_json_snapshot!(body, @r#"
    {
      "error": {
        "message": "Rate limit exceeded: Token rate limit exceeded. Request requires more tokens than the configured limit allows and cannot be fulfilled.",
        "type": "rate_limit_error",
        "code": 429
      }
    }
    "#);
}

/// Test error response for missing required client identification.
#[tokio::test]
async fn missing_client_id_error_format() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 1000
        interval = "60s"

        [server.client_identification]
        enabled = true
        client_id = { http_header = "X-Client-Id" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}]
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        // Missing X-Client-Id header
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 401);

    let body = response.json::<serde_json::Value>().await.unwrap();
    insta::assert_json_snapshot!(body, @r#"
    {
      "error": "unauthorized",
      "error_description": "Client identification required"
    }
    "#);
}

/// Test error response for unauthorized group.
#[tokio::test]
async fn unauthorized_group_error_format() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 1000
        interval = "60s"

        [server.client_identification]
        enabled = true
        allowed_groups = ["basic", "premium"]
        client_id = { http_header = "X-Client-Id" }
        group_id = { http_header = "X-Group" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}]
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "test-client")
        .header("X-Group", "enterprise") // Not in allowed_groups
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 403);

    let body = response.json::<serde_json::Value>().await.unwrap();
    insta::assert_json_snapshot!(body, @r#"
    {
      "error": "forbidden",
      "error_description": "Access denied"
    }
    "#);
}
