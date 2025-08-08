use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow;
use axum::{
    Router,
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

use super::provider::{LlmProviderConfig, TestLlmProvider};

/// Builder for Anthropic test server
pub struct AnthropicMock {
    name: String,
    models: Vec<String>,
    custom_responses: HashMap<String, String>,
}

impl AnthropicMock {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            models: vec![
                "claude-3-5-sonnet-20241022".to_string(),
                "claude-3-5-haiku-20241022".to_string(),
                "claude-3-opus-20240229".to_string(),
                "claude-3-sonnet-20240229".to_string(),
                "claude-3-haiku-20240307".to_string(),
            ],
            custom_responses: HashMap::new(),
        }
    }

    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.models = models;
        self
    }

    pub fn with_response(mut self, trigger: impl Into<String>, response: impl Into<String>) -> Self {
        self.custom_responses.insert(trigger.into(), response.into());
        self
    }
}

impl TestLlmProvider for AnthropicMock {
    fn provider_type(&self) -> &str {
        "anthropic"
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn spawn(self: Box<Self>) -> anyhow::Result<LlmProviderConfig> {
        let state = Arc::new(TestAnthropicState {
            models: self.models,
            custom_responses: self.custom_responses,
        });

        let app = Router::new()
            .route("/v1/messages", post(create_message))
            .route("/v1/models", get(list_models))
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Give the server time to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        Ok(LlmProviderConfig {
            name: self.name.clone(),
            address,
            provider_type: super::provider::ProviderType::Anthropic,
        })
    }
}

/// Test Anthropic server state
struct TestAnthropicState {
    models: Vec<String>,
    custom_responses: HashMap<String, String>,
}

/// Spawn a test Anthropic server on a random port (legacy compatibility)
pub struct TestAnthropicServer {
    pub address: SocketAddr,
    _handle: tokio::task::JoinHandle<()>,
}

impl TestAnthropicServer {
    pub async fn spawn() -> anyhow::Result<Self> {
        let builder = AnthropicMock::new("test_anthropic");
        let config = Box::new(builder).spawn().await?;

        Ok(Self {
            address: config.address,
            _handle: tokio::spawn(async {}), // Dummy handle, server already running
        })
    }

    pub fn url(&self) -> String {
        format!("http://{}/v1", self.address)
    }
}

/// Handle Anthropic message creation requests
async fn create_message(
    State(state): State<Arc<TestAnthropicState>>,
    headers: axum::http::HeaderMap,
    Json(request): Json<AnthropicMessageRequest>,
) -> impl IntoResponse {
    // Validate required headers
    if !headers.contains_key("x-api-key") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {
                    "type": "authentication_error",
                    "message": "Missing x-api-key header"
                }
            })),
        )
            .into_response();
    }

    if !headers.contains_key("anthropic-version") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": {
                    "type": "invalid_request_error",
                    "message": "Missing anthropic-version header"
                }
            })),
        )
            .into_response();
    }

    // Check for custom responses
    let mut response_text = None;
    for message in &request.messages {
        for (trigger, response) in &state.custom_responses {
            if message.content.contains(trigger) {
                response_text = Some(response.clone());
                break;
            }
        }
        if response_text.is_some() {
            break;
        }
    }

    // If no custom response, use default logic
    let response_text = response_text.unwrap_or_else(|| {
        format!(
            "Test response to: {}",
            request.messages.first().map(|m| m.content.as_str()).unwrap_or("empty")
        )
    });

    // Generate a test response
    let response = AnthropicMessageResponse {
        id: format!("msg_{}", uuid::Uuid::new_v4()),
        response_type: "message".to_string(),
        role: "assistant".to_string(),
        content: vec![AnthropicContent {
            content_type: "text".to_string(),
            text: response_text,
        }],
        model: request.model.clone(),
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: AnthropicUsage {
            input_tokens: 10,
            output_tokens: 15,
        },
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// Handle list models requests (Anthropic doesn't have this endpoint, but we mock it for testing)
async fn list_models(State(state): State<Arc<TestAnthropicState>>) -> Json<AnthropicModelsResponse> {
    let models = state
        .models
        .iter()
        .enumerate()
        .map(|(i, id)| {
            AnthropicModel {
                id: id.clone(),
                created: Some(1709164800 + i as u64 * 86400), // Incremental timestamps
            }
        })
        .collect();

    Json(AnthropicModelsResponse { data: models })
}

// Anthropic API types

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicMessageRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(default)]
    system: Option<String>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    top_k: Option<u32>,
    #[serde(default)]
    stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct AnthropicMessageResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String,
    content: Vec<AnthropicContent>,
    model: String,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Serialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct AnthropicUsage {
    input_tokens: i32,
    output_tokens: i32,
}

#[derive(Debug, Serialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}

#[derive(Debug, Serialize)]
struct AnthropicModel {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    created: Option<u64>,
}
