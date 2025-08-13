//! Integration tests for Redis-based rate limiting in LLM endpoints.

use indoc::formatdoc;
use integration_tests::{TestServer, llms::OpenAIMock};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_prefix(test_name: &str) -> String {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    format!("{test_name}_{timestamp}_")
}

#[tokio::test]
async fn rate_limiting_with_headers() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let prefix = unique_prefix("headers");

    let config = formatdoc! {r#"
        [server.rate_limits]
        enabled = true
        storage = {{ type = "redis", url = "redis://127.0.0.1:6379", key_prefix = "{prefix}" }}

        [llm.providers.openai.rate_limits.per_user]
        limit = 100
        interval = "60s"

        [server.client_identification]
        enabled = true
        client_id = {{ http_header = "X-Client-Id" }}
    "#};

    let server = builder.build(&config).await;
    let client = &server.client;

    // Each request: ~8 input tokens + 30 max_tokens = 38 tokens
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Hello"}],
        "max_tokens": 30
    });

    // First request: 38 tokens (should succeed)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "redis-client-headers-test")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200, "First request (38 tokens) should succeed");

    // Second request: 76 tokens total (should succeed)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "redis-client-headers-test")
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        200,
        "Second request (76 total tokens) should succeed"
    );

    // Third request: 114 tokens total (should fail - exceeds 100 limit)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "redis-client-headers-test")
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        429,
        "Third request (114 total tokens) should be rate limited"
    );

    // Verify error response format
    let body = response.json::<serde_json::Value>().await.unwrap();

    insta::assert_json_snapshot!(body, {
        ".error.message" => "[rate limit message]"
    }, @r#"
    {
      "error": {
        "message": "[rate limit message]",
        "type": "rate_limit_error",
        "code": 429
      }
    }
    "#);
}

/// Test Redis rate limiting with group-based limits using headers.
#[tokio::test]
async fn group_based_rate_limiting() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let prefix = unique_prefix("groups");

    let config = formatdoc! {r#"
        [server.rate_limits]
        enabled = true
        storage = {{ type = "redis", url = "redis://127.0.0.1:6379", key_prefix = "{prefix}" }}

        [llm.providers.openai.rate_limits.per_user]
        limit = 100
        interval = "60s"

        [llm.providers.openai.rate_limits.per_user.groups.premium]
        limit = 500
        interval = "60s"

        [llm.providers.openai.rate_limits.per_user.groups.basic]
        limit = 50
        interval = "60s"

        [server.client_identification]
        enabled = true
        allowed_groups = ["premium", "basic"]  # Must define allowed_groups when using group_id
        client_id = {{ http_header = "X-Client-Id" }}
        group_id = {{ http_header = "X-Group" }}
    "#};

    let server = builder.build(&config).await;
    let client = &server.client;

    // Basic tier client with small limit (50 tokens)
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 40  // ~48 tokens total
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "basic-group-client")
        .header("X-Group", "basic")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Basic tier first request should succeed");

    // Basic tier should be rate limited on next request (48 + 48 = 96 > 50)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "basic-group-client")
        .header("X-Group", "basic")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429, "Basic tier should be rate limited");

    // Premium tier client with larger limit (500 tokens)
    let large_request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 200  // ~208 tokens total
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&large_request)
        .header("X-Client-Id", "premium-group-client")
        .header("X-Group", "premium")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200, "Premium tier should handle large requests");

    // Premium tier should still have capacity for another request
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&large_request)
        .header("X-Client-Id", "premium-group-client")
        .header("X-Group", "premium")
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        200,
        "Premium tier should still have capacity (416/500)"
    );
}

/// Test that different clients have independent Redis rate limits.
#[tokio::test]
async fn independent_client_rate_limits() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let prefix = unique_prefix("independent");
    let config = format!(
        r#"
        [server.rate_limits]
        enabled = true
        storage = {{ type = "redis", url = "redis://127.0.0.1:6379", key_prefix = "{}" }}

        [llm.providers.openai.rate_limits.per_user]
        limit = 100
        interval = "60s"

        [server.client_identification]
        enabled = true
        client_id = {{ http_header = "X-Client-Id" }}
    "#,
        prefix
    );

    let server = builder.build(&config).await;
    let client = &server.client;

    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Hello"}],
        "max_tokens": 80  // ~88 tokens total
    });

    // Client 1 uses most of their limit
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "redis-independent-client-1")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Client 1 first request should succeed");

    // Client 1 should be rate limited on next request (88 + 88 = 176 > 100)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "redis-independent-client-1")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429, "Client 1 should be rate limited");

    // But Client 2 should still be able to make requests (independent rate limit)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "redis-independent-client-2")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Client 2 should have independent rate limit");
}

/// Test Redis rate limiting persists across server restarts.
#[tokio::test]
async fn rate_limit_persistence() {
    let prefix = unique_prefix("persistence");
    let config = format!(
        r#"
        [server.rate_limits]
        enabled = true
        storage = {{ type = "redis", url = "redis://127.0.0.1:6379", key_prefix = "{}" }}

        [llm.providers.openai.rate_limits.per_user]
        limit = 100
        interval = "300s"  # 5 minute window for persistence test

        [server.client_identification]
        enabled = true
        client_id = {{ http_header = "X-Client-Id" }}
    "#,
        prefix
    );

    // First server instance
    {
        let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

        let mut builder = TestServer::builder();
        builder.spawn_llm(openai).await;

        let server = builder.build(&config).await;
        let client = &server.client;

        let request = json!({
            "model": "openai/gpt-4",
            "messages": [{"role": "user", "content": "Test"}],
            "max_tokens": 80  // ~88 tokens
        });

        let response = client
            .request(reqwest::Method::POST, "/llm/v1/chat/completions")
            .json(&request)
            .header("X-Client-Id", "redis-persistent-client")
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200, "First server request should succeed");
    }

    // Second server instance (simulating restart)
    {
        let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

        let mut builder = TestServer::builder();
        builder.spawn_llm(openai).await;

        let server = builder.build(&config).await;
        let client = &server.client;

        let request = json!({
            "model": "openai/gpt-4",
            "messages": [{"role": "user", "content": "Test"}],
            "max_tokens": 15  // ~23 tokens, total: 88 + 23 = 111 > 100
        });

        // Should be rate limited because Redis persisted the previous consumption
        let response = client
            .request(reqwest::Method::POST, "/llm/v1/chat/completions")
            .json(&request)
            .header("X-Client-Id", "redis-persistent-client")
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            429,
            "Should be rate limited due to Redis persistence"
        );
    }
}

/// Test concurrent requests with Redis rate limiting.
#[tokio::test]
async fn concurrent_requests() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let prefix = unique_prefix("concurrent");
    let config = format!(
        r#"
        [server.rate_limits]
        enabled = true
        storage = {{ type = "redis", url = "redis://127.0.0.1:6379", key_prefix = "{}" }}

        [llm.providers.openai.rate_limits.per_user]
        limit = 200
        interval = "60s"

        [server.client_identification]
        enabled = true
        client_id = {{ http_header = "X-Client-Id" }}
    "#,
        prefix
    );

    let server = builder.build(&config).await;
    let client = &server.client;

    // Prepare multiple requests that together exceed the limit
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 45  // ~53 tokens per request
    });

    // Send 4 concurrent requests (4 * 53 = 212 tokens > 200 limit)
    let futures = (0..4).map(|_| {
        let client = client.clone();
        let req = request.clone();

        async move {
            client
                .request(reqwest::Method::POST, "/llm/v1/chat/completions")
                .json(&req)
                .header("X-Client-Id", "redis-concurrent-test")
                .send()
                .await
                .unwrap()
        }
    });

    let responses = futures::future::join_all(futures).await;

    // Count successful vs rate-limited responses
    let success_count = responses.iter().filter(|r| r.status() == 200).count();
    let rate_limited_count = responses.iter().filter(|r| r.status() == 429).count();

    // At least one should be rate limited (since 4 * 53 > 200)
    assert!(rate_limited_count >= 1, "At least one request should be rate limited");

    assert!(
        success_count >= 3,
        "At least 3 requests should succeed (200 / 53 = 3.7)"
    );

    assert_eq!(
        success_count + rate_limited_count,
        4,
        "All requests should either succeed or be rate limited"
    );
}
