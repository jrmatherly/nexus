# LLM Provider Implementation Guide

Unified interface for LLM providers with required tool calling support.

## Required Features
- **Tool Calling**: Function definitions, tool choice ("auto"/"none"/"required"/specific), parallel calls
- **Streaming**: SSE-based streaming with OpenAI-compatible chunks
- **Model Management**: List models, dynamic fetching, caching

## Implementation Checklist

### 1. Config (config crate)
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct YourProviderConfig {
    pub api_key: SecretString,
    pub api_url: Option<String>,
}
```
Add to `LlmProviderConfig` enum, test with insta snapshots.

### 2. Provider Trait (llm crate)
```rust
#[async_trait]
impl Provider for YourProvider {
    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse>;
    async fn chat_completion_stream(&self, request: ChatCompletionRequest) -> Result<ChatCompletionStream>;
    async fn list_models(&self) -> Result<Vec<Model>>;
    fn name(&self) -> &str;
    fn supports_streaming(&self) -> bool;
}
```

### 3. Type Conversion
- `input.rs`: Convert OpenAI → provider format
- `output.rs`: Convert provider → OpenAI format
- Use `Other(String)` variants for forward compatibility
- Transform tool calls bidirectionally

### 4. Error Mapping
- 400 → `InvalidRequest`
- 401 → `AuthenticationFailed` 
- 403 → `InsufficientQuota`
- 404 → `ModelNotFound`
- 429 → `RateLimitExceeded`
- 500 → `InternalError(Some(msg))` for provider errors, `None` for internal

### 5. Streaming
```rust
response.bytes_stream()
    .eventsource()
    .filter_map(|event| /* Convert to ChatCompletionChunk */)
```

### 6. Testing Requirements
- Basic chat completion
- Tool calling (single, parallel, forced)
- Streaming with tool calls
- Error scenarios (auth, rate limits, invalid model)
- Integration tests with mock server

## Architecture Patterns

### Model Names
Format: `provider_name/model_id` (e.g., `openai/gpt-4`)

### Caching
- Cache model lists (5 min TTL)
- Use provider-level cache, not per-model

### Rate Limiting
Integrates with token-based limits via `ClientIdentity`

### Header Rules
Support header forwarding, removal, insertion per provider/model

## AWS Bedrock Notes
- Use unified Converse API, not family-specific implementations
- Single endpoint for all models
- Consistent tool calling across families

## Common Pitfalls
- Missing `finish_reason` in streaming
- Not handling rate limit headers
- Incorrect tool call streaming order
- Missing error context in responses