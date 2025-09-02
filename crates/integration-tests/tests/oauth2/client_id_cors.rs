use indoc::indoc;
use insta::assert_json_snapshot;
use integration_tests::TestServer;
use reqwest::Method;
use serde_json::Value;

#[tokio::test]
async fn cors_preflight_bypasses_client_id_middleware() {
    // Config with client identification required
    let config = indoc! {r#"
        [server.client_identification]
        enabled = true
        
        [server.client_identification.client_id]
        type = "http_header"
        http_header = "X-Client-Id"
        
        [server.client_identification.validation]
        group_values = ["free", "premium"]
        
        [server.cors]
        allow_origins = "*"
        allow_methods = ["GET", "POST", "OPTIONS"]
        allow_headers = ["Authorization", "Content-Type", "X-Client-Id"]
        
        [mcp]
        enabled = true
        
        [mcp.servers.dummy]
        cmd = ["echo", "dummy"]
    "#};

    let server = TestServer::builder().build(config).await;

    // CORS preflight OPTIONS request without client ID header
    // This should NOT be blocked by client ID middleware
    let response = server
        .client
        .request(Method::OPTIONS, "/mcp")
        .header("Origin", "https://example.com")
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "Content-Type, X-Client-Id")
        .send()
        .await
        .unwrap();

    // CORS preflight should succeed even without client ID
    assert_eq!(
        response.status(),
        200,
        "CORS preflight should succeed without client ID"
    );

    // Verify CORS headers are present
    assert!(response.headers().contains_key("access-control-allow-origin"));
    assert!(response.headers().contains_key("access-control-allow-methods"));
    assert!(response.headers().contains_key("access-control-allow-headers"));
}

#[tokio::test]
async fn regular_post_requires_client_id() {
    // Same config as above
    let config = indoc! {r#"
        [server.client_identification]
        enabled = true
        
        [server.client_identification.client_id]
        type = "http_header"
        http_header = "X-Client-Id"
        
        [server.cors]
        allow_origins = "*"
        allow_methods = ["GET", "POST", "OPTIONS"]
        allow_headers = ["Authorization", "Content-Type", "X-Client-Id"]
        
        [mcp]
        enabled = true
        
        [mcp.servers.dummy]
        cmd = ["echo", "dummy"]
    "#};

    let server = TestServer::builder().build(config).await;

    // Regular POST without client ID should be rejected
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 1
    });

    let response = server.client.post("/mcp", &body).await.unwrap();

    assert_eq!(response.status(), 400, "Regular POST should require client ID");

    let body = response.json::<Value>().await.unwrap();
    assert_json_snapshot!(body, @r###"
    {
      "error": "missing_client_id",
      "error_description": "Client identification is required"
    }
    "###);
}

#[tokio::test]
async fn post_with_client_id_succeeds() {
    let config = indoc! {r#"
        [server.client_identification]
        enabled = true
        
        [server.client_identification.client_id]
        type = "http_header"
        http_header = "X-Client-Id"
        
        [mcp]
        enabled = true
        
        [mcp.servers.dummy]
        cmd = ["echo", "dummy"]
    "#};

    let server = TestServer::builder().build(config).await;

    // POST with client ID should succeed
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 1
    });

    // Clone the client so we can mutate it
    let mut client = server.client.clone();
    client.push_header("X-Client-Id", "test-client");

    let response = client.post("/mcp", &body).await.unwrap();

    assert_eq!(response.status(), 200, "POST with client ID should succeed");
}

#[tokio::test]
async fn cors_preflight_with_oauth_and_client_id() {
    // Complex scenario: OAuth + Client ID both enabled
    let config = indoc! {r#"
        [server.oauth]
        url = "http://localhost:4444/.well-known/jwks.json"
        expected_issuer = "http://localhost:4444"
        
        [server.oauth.protected_resource]
        resource = "http://localhost/api"
        authorization_servers = ["http://localhost:4444"]
        
        [server.client_identification]
        enabled = true
        
        [server.client_identification.client_id]
        type = "jwt_claim"
        jwt_claim = "sub"
        
        [server.cors]
        allow_origins = "*"
        allow_methods = ["GET", "POST", "OPTIONS"]
        allow_headers = ["Authorization", "Content-Type"]
        
        [mcp]
        enabled = true
        
        [mcp.servers.dummy]
        cmd = ["echo", "dummy"]
    "#};

    let server = TestServer::builder().build(config).await;

    // CORS preflight without Authorization header should still work
    let response = server
        .client
        .request(Method::OPTIONS, "/mcp")
        .header("Origin", "https://example.com")
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "Authorization, Content-Type")
        .send()
        .await
        .unwrap();

    // CORS preflight should succeed even without auth or client ID
    assert_eq!(
        response.status(),
        200,
        "CORS preflight should succeed without auth/client ID"
    );

    // Verify CORS headers
    assert!(response.headers().contains_key("access-control-allow-origin"));
    assert!(response.headers().contains_key("access-control-allow-methods"));
}
