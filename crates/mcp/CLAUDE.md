# MCP Router Guidelines

This crate implements the core MCP (Model Context Protocol) routing functionality for Nexus, aggregating multiple MCP servers and providing intelligent tool discovery and execution.

## Purpose

The MCP crate provides:
- Tool discovery and aggregation from multiple MCP servers
- Intelligent routing of tool execution requests
- Full-text search indexing with Tantivy
- Support for static and dynamic (auth-forwarding) servers
- Protocol translation between different MCP transports
- Per-MCP-server and per-tool rate limiting

## Architecture Overview

### Core Components

```
mcp/
├── server.rs         # Main MCP server implementation
├── downstream/       # Downstream MCP client connections
│   ├── client.rs     # Protocol-agnostic MCP client
│   └── ids.rs        # Tool ID management
├── index.rs          # Tantivy search index for tools
├── cache.rs          # Dynamic downstream caching
└── server/
    ├── search.rs     # Search tool implementation
    └── execute.rs    # Execute tool implementation
```

### Key Concepts

1. **Static Servers**: MCP servers that don't require authentication forwarding
2. **Dynamic Servers**: MCP servers that need user-specific auth tokens
3. **Tool Aggregation**: Combining tools from multiple servers with namespacing
4. **Search Index**: Full-text search across all available tools

## Implementation Guidelines

### Tool Naming Convention

Tools are namespaced with their server name:

```rust
// Format: "server_name__tool_name"
"math_server__calculator"
"file_system__read_file"
"weather_api__get_forecast"
```

### Error Handling

Always use proper error context:

```rust
// Good: Contextual error with server information
async fn call_tool(&self, tool_name: &str) -> anyhow::Result<CallToolResult> {
    let result = self.client
        .call_tool(params)
        .await
        .with_context(|| format!("Failed to call tool {} on server {}", tool_name, self.name))?;

    Ok(result)
}

// Bad: Generic error without context
async fn call_tool(&self, tool_name: &str) -> anyhow::Result<CallToolResult> {
    Ok(self.client.call_tool(params).await?)
}
```

### Downstream Client Creation

Support all MCP transport types:

```rust
impl DownstreamClient {
    pub async fn new_stdio(name: &str, config: &StdioConfig) -> anyhow::Result<Self> {
        // STDIO transport for local processes
    }

    pub async fn new_http(name: &str, config: &HttpConfig) -> anyhow::Result<Self> {
        // HTTP transport with SSE or streamable-http
    }
}
```

### Search Implementation

The search tool uses Tantivy for full-text indexing:

```rust
pub struct SearchTool {
    /// All available tools sorted by name for binary search
    tools: Vec<Tool>,
    /// Tantivy index for keyword search
    index: Arc<ToolIndex>,
}

impl SearchTool {
    /// Binary search for exact tool name matches
    pub fn find_exact(&self, tool_name: &str) -> Option<&Tool> {
        self.tools
            .binary_search_by(|tool| tool.name.as_ref().cmp(tool_name))
            .ok()
            .map(|idx| &self.tools[idx])
    }

    /// Full-text search using Tantivy
    pub async fn find_by_keywords(&self, keywords: Vec<String>) -> anyhow::Result<Vec<SearchResult>> {
        // Returns tools ranked by relevance score
    }
}
```

### Dynamic Server Caching

Cache downstream connections for auth-forwarding servers:

```rust
pub struct DynamicDownstreamCache {
    config: McpConfig,
    cache: Arc<Mutex<LruCache<CacheKey, CachedEntry>>>,
}

// Cache key includes auth token for user-specific connections
struct CacheKey {
    auth_token: Option<SecretString>,
}
```

## Built-in Tools

The MCP router always provides two built-in tools:

### 1. Search Tool

```rust
Tool {
    name: "search",
    description: "Search for relevant tools",
    input_schema: {
        "type": "object",
        "properties": {
            "keywords": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "required": ["keywords"]
    }
}
```

### 2. Execute Tool

```rust
Tool {
    name: "execute",
    description: "Executes a tool with the given parameters",
    input_schema: {
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "arguments": { "type": "object" }
        },
        "required": ["name", "arguments"]
    }
}
```

## Server Capabilities

Always enable all MCP capabilities:

```rust
ServerCapabilities::builder()
    .enable_tools()      // Tool discovery and execution
    .enable_prompts()    // Prompt templates
    .enable_resources()  // Resource access
    .build()
```

## Authentication Forwarding

Handle auth forwarding for dynamic servers:

```rust
// Extract auth token from request headers
fn extract_auth_token(headers: &HeaderMap) -> Option<SecretString> {
    headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|token| SecretString::new(token.to_string()))
}

// Use token when creating dynamic downstream
let downstream = if server_config.forwards_authentication() {
    Downstream::new(&config, auth_token).await?
} else {
    // Use cached static downstream
    self.static_downstream.clone()
};
```

## Header Insertion for MCP

The MCP crate supports header insertion for HTTP-based MCP servers. Headers with static values can be configured globally (for all MCP servers) or per-server.

### Header Configuration

MCP currently only supports the `insert` rule for adding static headers:

```rust
// From config/src/headers.rs
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum McpHeaderRule {
    /// Insert a new header with a static value.
    Insert(HeaderInsert),
}

pub struct HeaderInsert {
    pub name: HeaderName,
    pub value: HeaderValue,  // Supports {{ env.VAR }} templating
}
```

### Header Processing

Headers are applied when creating HTTP clients for downstream servers:

```rust
// In downstream/client.rs
pub async fn new_http<'a>(
    name: &str,
    config: &HttpConfig,
    global_headers: impl Iterator<Item = &'a config::McpHeaderRule> + Clone,
) -> anyhow::Result<Self> {
    // Chain global headers with server-specific headers
    let client = create_client(
        config.tls.as_ref(),
        config.auth.as_ref(),
        global_headers.chain(config.get_effective_header_rules()),
    )?;
    // ...
}

fn create_client<'a>(
    tls: Option<&TlsClientConfig>,
    auth: Option<&ClientAuthConfig>,
    header_rules: impl Iterator<Item = &'a config::McpHeaderRule>,
) -> anyhow::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder();
    
    // Process all header rules and build default headers
    let mut headers = HeaderMap::new();
    for rule in header_rules {
        // MCP only supports Insert rule currently
        match rule {
            McpHeaderRule::Insert(insert) => {
                headers.insert(
                    HeaderName::from_str(&insert.name)?,
                    HeaderValue::from_str(&insert.value)?
                );
            }
        }
    }
    
    // Apply headers as defaults to the HTTP client
    builder = builder.default_headers(headers);
    // ...
}
```

### Key Points

- **Static Values Only**: Headers are set at client initialization with static values
- **Environment Variables**: Values support `{{ env.VAR_NAME }}` templating
- **HTTP Only**: Headers only apply to HTTP-based MCP servers, not STDIO
- **Insert Only**: Currently only supports inserting headers, not forwarding/removing
- **Initialization Time**: Headers are configured when the client is created, not per-request

## Logging

Use structured logging with appropriate levels:

```rust
// Debug: Detailed operational information
log::debug!("Creating stdio downstream service for '{name}'");
log::debug!("Indexing tool '{}'", tool.name);

// Info: Important state changes
log::info!("Initialized {server_count} MCP servers");

// Warn: Recoverable issues
log::warn!("Tool '{tool_name}' returned empty response");

// Error: Serious issues that need attention
log::error!("Failed to connect to server '{name}': {error:?}");
```

## Testing

Do not write unit tests, only integration tests. Use the integration-tests crate for end-to-end testing. Delegate testing to the test agent, explaining the changes made to the codebase.

## Performance Considerations

1. **Tool Caching**: Static tools are indexed once at startup
2. **Connection Pooling**: Reuse HTTP connections for downstream servers
3. **Search Optimization**: Use binary search for exact matches, Tantivy for fuzzy search
4. **Lazy Initialization**: Only connect to dynamic servers when needed

## Security

1. **Auth Token Handling**: Use `SecretString` for sensitive data
2. **Input Validation**: Validate tool names and arguments
3. **Error Sanitization**: Don't leak internal details in error messages
4. **Resource Limits**: Implement timeouts and size limits

Remember: The MCP router is the heart of Nexus - it must be reliable, performant, and secure while providing a seamless tool aggregation experience.

## Keeping This Document Updated

**IMPORTANT**: Update this CLAUDE.md when MCP routing logic changes:

1. **Protocol Changes**: Document new MCP protocol features or transports
2. **Routing Logic**: Update when tool discovery or execution changes
3. **Caching Strategy**: Document changes to dynamic server caching
4. **Search Updates**: Update if Tantivy usage or indexing changes
5. **New Features**: Add sections for new MCP capabilities (prompts, resources, etc.)

Update triggers:
- Adding new downstream transport types
- Changing tool naming conventions
- Modifying search algorithms or indexing
- Updating authentication forwarding logic
- Adding new built-in tools beyond search/execute
- Modifying header rule processing or configuration
