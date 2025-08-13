//! Test to verify that users with undefined groups fall back to default limits.

use integration_tests::{TestServer, llms::OpenAIMock};
use serde_json::json;

/// Test that users with undefined groups fall back to default limits.
#[tokio::test]
async fn undefined_group_falls_back_to_default() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    // Configuration with only "premium" group defined, but not "basic"
    let config = r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 100
        interval = "60s"

        [llm.providers.openai.rate_limits.per_user.groups.premium]
        limit = 500
        interval = "60s"

        [server.client_identification]
        enabled = true
        allowed_groups = ["premium", "basic"]  # Must define allowed_groups when using group_id
        client_id = { http_header = "X-Client-Id" }
        group_id = { http_header = "X-Group" }
    "#;

    let server = builder.build(config).await;
    let client = &server.client;

    // Request with undefined group "basic" - should fall back to default (100 tokens)
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 80  // ~88 tokens total
    });

    // First request with undefined group "basic"
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "test-client")
        .header("X-Group", "basic") // This group is NOT defined
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200, "First request should succeed");

    // Second request - total would be 176 tokens
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "test-client")
        .header("X-Group", "basic") // Still using undefined group
        .send()
        .await
        .unwrap();

    // This should be rate limited at 100 tokens (the default), not unlimited
    assert_eq!(
        response.status(),
        429,
        "Should be rate limited by default limit since 'basic' group is not defined"
    );

    // Verify the error message indicates default rate limit
    let body = response.json::<serde_json::Value>().await.unwrap();
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Rate limit exceeded"),
        "Should indicate rate limit exceeded"
    );
}

/// Test that users with a defined group do NOT fall back to default.
#[tokio::test]
async fn defined_group_uses_group_limit_not_default() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 30
        interval = "60s"

        [llm.providers.openai.rate_limits.per_user.groups.premium]
        limit = 200
        interval = "60s"

        [server.client_identification]
        enabled = true
        allowed_groups = ["premium"]  # Must define allowed_groups when using group_id
        client_id = { http_header = "X-Client-Id" }
        group_id = { http_header = "X-Group" }
    "#;

    let server = builder.build(config).await;
    let client = &server.client;

    // Request that would exceed default (30) but not group limit (200)
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 40  // ~48 tokens total
    });

    // Make two requests with premium group (96 tokens total)
    for i in 1..=2 {
        let response = client
            .request(reqwest::Method::POST, "/llm/v1/chat/completions")
            .json(&request)
            .header("X-Client-Id", "premium-client")
            .header("X-Group", "premium")
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            200,
            "Request {} should succeed - using group limit (200) not default (30)",
            i
        );
    }

    // Third request would be 144 tokens total - still under group limit
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "premium-client")
        .header("X-Group", "premium")
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        200,
        "Third request should succeed - group limit (200) overrides default (30)"
    );
}
