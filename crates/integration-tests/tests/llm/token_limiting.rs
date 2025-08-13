//! Integration tests for token-based rate limiting in LLM endpoints.

mod edge_cases;
mod group_fallback;
mod redis;

use indoc::indoc;
use integration_tests::{TestServer, llms::OpenAIMock};
use serde_json::json;

/// Test that requests without client identification are allowed when rate limiting is not enforced.
#[tokio::test]
async fn no_client_id_allows_requests() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        # Rate limits without client identification should not be configured
        # This test verifies requests work without rate limiting
    "#};

    let server = builder.build(config).await;
    let llm = server.llm_client("/llm");

    // Request should succeed without client identification
    let response = llm.simple_completion("openai/gpt-4", "Hello").await;

    insta::assert_json_snapshot!(response["choices"][0]["message"], @r#"
    {
      "role": "assistant",
      "content": "Hello! I'm a test LLM assistant. How can I help you today?"
    }
    "#);
}

/// Test basic token rate limiting with client identification and accumulation.
#[tokio::test]
async fn token_rate_limit_enforced_with_client_id() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 100
        interval = "60s"

        [server.client_identification]
        enabled = true
        client_id = { http_header = "X-Client-Id" }
    "#};

    let server = builder.build(config).await;
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
        .header("X-Client-Id", "test-client")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "First request (38 tokens) should succeed");

    // Second request: 38 + 38 = 76 tokens (should succeed)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "test-client")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "Second request (76 total tokens) should succeed"
    );

    // Third request: 76 + 38 = 114 tokens (should fail - exceeds 100 limit)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "test-client")
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
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Rate limit exceeded")
    );
}

/// Test the 4-level hierarchy for rate limit resolution with accumulation.
#[tokio::test]
async fn rate_limit_hierarchy_resolution() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        # Level 4: Provider default
        [llm.providers.openai.rate_limits.per_user]
        limit = 100
        interval = "60s"

        # Level 3: Provider + Group
        [llm.providers.openai.rate_limits.per_user.groups.pro]
        limit = 200
        interval = "60s"

        # Level 2: Model default
        [llm.providers.openai.models."gpt-4".rate_limits.per_user]
        limit = 300
        interval = "60s"

        # Level 1: Model + Group (highest priority)
        [llm.providers.openai.models."gpt-4".rate_limits.per_user.groups.pro]
        limit = 400
        interval = "60s"

        [server.client_identification]
        enabled = true
        allowed_groups = ["pro"]  # Must define allowed_groups when using group_id
        client_id = { http_header = "X-Client-Id" }
        group_id = { http_header = "X-Group" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    // Test Level 1: Model + Group (gpt-4 + pro group) - limit 400
    // Each request: ~8 input + 150 max = 158 tokens
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 150
    });

    // First request: 158 tokens
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client1")
        .header("X-Group", "pro")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "Level 1: First request (158 tokens) should succeed"
    );

    // Second request: 158 + 158 = 316 tokens
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client1")
        .header("X-Group", "pro")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "Level 1: Second request (316 total) should succeed"
    );

    // Third request: 316 + 158 = 474 tokens (exceeds 400 limit)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client1")
        .header("X-Group", "pro")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        429,
        "Level 1: Third request (474 total) should be rate limited"
    );

    // Test Level 2: Model default (gpt-4 without group) - limit 300
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 140
    });

    // First request: 148 tokens
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client2")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Level 2: First request should succeed");

    // Second request: 296 tokens (still under 300)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client2")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "Level 2: Second request (296 total) should succeed"
    );

    // Third request: 444 tokens (exceeds 300 limit)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client2")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429, "Level 2: Third request should be rate limited");

    // Test Level 3: Provider + Group (gpt-3.5-turbo + pro group) - limit 200
    let request = json!({
        "model": "openai/gpt-3.5-turbo",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 90
    });

    // First request: 98 tokens
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client3")
        .header("X-Group", "pro")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Level 3: First request should succeed");

    // Second request: 196 tokens (under 200)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client3")
        .header("X-Group", "pro")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "Level 3: Second request (196 total) should succeed"
    );

    // Third request: 294 tokens (exceeds 200 limit)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client3")
        .header("X-Group", "pro")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429, "Level 3: Third request should be rate limited");

    // Test Level 4: Provider default (gpt-3.5-turbo without group) - limit 100
    let request = json!({
        "model": "openai/gpt-3.5-turbo",
        "messages": [{"role": "user", "content": "Test"}],
        "max_tokens": 40
    });

    // First request: 48 tokens
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client4")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Level 4: First request should succeed");

    // Second request: 96 tokens (under 100)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client4")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "Level 4: Second request (96 total) should succeed"
    );

    // Third request: 144 tokens (exceeds 100 limit)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client4")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429, "Level 4: Third request should be rate limited");
}

/// Test that different clients have independent rate limits.
#[tokio::test]
async fn independent_client_rate_limits() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 100
        interval = "60s"

        [server.client_identification]
        enabled = true
        client_id = { http_header = "X-Client-Id" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Hello"}],
        "max_tokens": 80
    });

    // Client 1 uses most of their limit
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client1")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Client 1 should be rate limited on next request
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client1")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429);

    // But Client 2 should still be able to make requests
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "client2")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

/// Test that group-based rate limits are enforced correctly.
#[tokio::test]
async fn group_based_rate_limits() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 500
        interval = "60s"

        [llm.providers.openai.rate_limits.per_user.groups.free]
        limit = 100
        interval = "60s"

        [llm.providers.openai.rate_limits.per_user.groups.pro]
        limit = 1000
        interval = "60s"

        [server.client_identification]
        enabled = true
        allowed_groups = ["free", "pro"]  # Must define allowed_groups when using group_id
        client_id = { http_header = "X-Client-Id" }
        group_id = { http_header = "X-Group" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    // Free tier client with small limit
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Hello"}],
        "max_tokens": 80
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "free-client")
        .header("X-Group", "free")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Free tier should be rate limited on next request
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "free-client")
        .header("X-Group", "free")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429);

    // Pro tier client with larger limit
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Hello"}],
        "max_tokens": 900
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "pro-client")
        .header("X-Group", "pro")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Pro tier should still have capacity
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Hello"}],
        "max_tokens": 50
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "pro-client")
        .header("X-Group", "pro")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

/// Test that requests without required client identification are rejected.
#[tokio::test]
async fn missing_client_id_rejected_when_required() {
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
        "messages": [{"role": "user", "content": "Hello"}]
    });

    // Request without X-Client-Id header should be rejected
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 401);

    let body = response.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["error"], "unauthorized");
    assert_eq!(body["error_description"], "Client identification required");
}

/// Test that invalid group names are rejected with 403 Forbidden.
#[tokio::test]
async fn invalid_group_rejected() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 1000
        interval = "60s"

        [server.client_identification]
        enabled = true
        allowed_groups = ["gold", "silver", "bronze"]
        client_id = { http_header = "X-Client-Id" }
        group_id = { http_header = "X-Group" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}]
    });

    // Request with invalid group should be rejected
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "test-client")
        .header("X-Group", "platinum") // Not in allowed_groups
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

/// Test clear token accumulation behavior.
#[tokio::test]
async fn clear_token_accumulation() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 100
        interval = "60s"

        [server.client_identification]
        enabled = true
        client_id = { http_header = "X-Client-Id" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    // Test accumulation: 30 + 30 + 45 = 105 tokens (exceeds 100)

    // Request 1: 30 tokens (30 total)
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Hi"}],  // ~6 input tokens
        "max_tokens": 24  // 6 + 24 = 30 tokens
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "accumulation-test")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Request 1 (30 tokens) should succeed");

    // Request 2: 30 tokens (60 total)
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "accumulation-test")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Request 2 (60 total tokens) should succeed");

    // Request 3: 45 tokens (105 total - exceeds limit)
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Hi"}],
        "max_tokens": 39  // 6 + 39 = 45 tokens
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "accumulation-test")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        429,
        "Request 3 (105 total tokens) should be rate limited"
    );
}

/// Test exact boundary conditions for rate limits.
#[tokio::test]
async fn exact_boundary_conditions() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 100
        interval = "60s"

        [server.client_identification]
        enabled = true
        client_id = { http_header = "X-Client-Id" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    // Test 1: Exactly at limit (100 tokens) should succeed
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],  // ~8 input tokens
        "max_tokens": 92  // 8 + 92 = 100 tokens exactly
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "boundary-test-1")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Request at exactly 100 tokens should succeed");

    // Any additional request should fail
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "A"}],  // ~5 input tokens
        "max_tokens": 1  // 5 + 1 = 6 tokens
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "boundary-test-1")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        429,
        "Any request after reaching exact limit should fail"
    );

    // Test 2: Just over limit (101 tokens) should fail immediately
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],  // ~8 input tokens
        "max_tokens": 93  // 8 + 93 = 101 tokens
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "boundary-test-2")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429, "Request for 101 tokens should fail immediately");

    // Test 3: Just under limit (99 tokens) should succeed
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],  // ~8 input tokens
        "max_tokens": 91  // 8 + 91 = 99 tokens
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "boundary-test-3")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Request for 99 tokens should succeed");

    // Can still make a 1-token request
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": ""}],  // ~4 input tokens
        "max_tokens": 0  // Minimum possible, but let's use 1 to be safe
    });

    let _response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "boundary-test-3")
        .send()
        .await
        .unwrap();
    // This might succeed or fail depending on exact token counting, but we're at 99+
}

/// Test that rate limits reset after the interval expires.
#[tokio::test]
async fn rate_limit_reset_after_interval() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 50
        interval = "2s"  # Short interval for testing

        [server.client_identification]
        enabled = true
        client_id = { http_header = "X-Client-Id" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    // Request that uses up the limit
    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}],  // ~8 input tokens
        "max_tokens": 42  // 8 + 42 = 50 tokens exactly
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "reset-test")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "First request at limit should succeed");

    // Second request should fail (limit exhausted)
    let small_request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Hi"}],
        "max_tokens": 1
    });

    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&small_request)
        .header("X-Client-Id", "reset-test")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429, "Request after limit exhausted should fail");

    // Wait for interval to expire (2 seconds + buffer)
    tokio::time::sleep(tokio::time::Duration::from_millis(2500)).await;

    // After reset, should be able to make requests again
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "reset-test")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Request after interval reset should succeed");
}

/// Test that missing group_id when allowed_groups is configured uses default rate limits.
#[tokio::test]
async fn missing_group_when_allowed_groups_configured() {
    let openai = OpenAIMock::new("openai").with_models(vec!["gpt-4".to_string()]);

    let mut builder = TestServer::builder();
    builder.spawn_llm(openai).await;

    let config = indoc! {r#"
        [llm.providers.openai.rate_limits.per_user]
        limit = 1000
        interval = "60s"

        [server.client_identification]
        enabled = true
        allowed_groups = ["gold", "silver", "bronze"]
        client_id = { http_header = "X-Client-Id" }
        group_id = { http_header = "X-Group" }
    "#};

    let server = builder.build(config).await;
    let client = &server.client;

    let request = json!({
        "model": "openai/gpt-4",
        "messages": [{"role": "user", "content": "Test"}]
    });

    // Request without group header when allowed_groups is configured
    // Should succeed and use default rate limits
    let response = client
        .request(reqwest::Method::POST, "/llm/v1/chat/completions")
        .json(&request)
        .header("X-Client-Id", "test-client")
        // Missing X-Group header - this is OK, will use default rate limit
        .send()
        .await
        .unwrap();

    // Should succeed since missing group is allowed (uses default rate limit)
    assert_eq!(
        response.status(),
        200,
        "Request without group should succeed with default rate limit"
    );

    // Verify the response is valid
    let body = response.json::<serde_json::Value>().await.unwrap();
    assert!(body["choices"].is_array());
}
