# AGENTS.md
This file provides guidance to AI coding assistants working in this repository.

**Note:** CLAUDE.md, .clinerules, .cursorrules, and other AI config files are symlinks to AGENTS.md in this project.

# Nexus - AI Router for MCP Servers and LLM Providers

**Nexus** is a sophisticated Rust-based AI router that aggregates MCP (Model Context Protocol) servers and LLM providers. This system helps reduce tool proliferation by intelligently routing and indexing available tools through a unified endpoint.

## Architecture Overview

- **MCP (Model Context Protocol)**: Protocol for connecting AI models with external tools and data sources
- **AI Router**: Nexus acts as an intelligent intermediary between LLMs and multiple MCP servers
- **Tool Indexing**: Uses Tantivy (full-text search engine) to create searchable indexes of available tools
- **Dynamic vs Static Tools**: Static tools are shared across users; dynamic tools require user authentication

### Workspace Structure

```
nexus/                          # Main binary application
crates/
├── server/                     # Shared HTTP server components (Axum)
├── config/                     # TOML configuration management
├── mcp/                        # MCP routing and tool management core
├── llm/                        # LLM provider routing and unified API
├── rate-limit/                 # Multi-level rate limiting system
├── telemetry/                  # OpenTelemetry metrics and observability
├── header-rules/              # HTTP header transformation
├── integration-tests/         # End-to-end testing with Docker Compose
└── integration-test-macros/   # Test infrastructure macros
```

## Build & Commands

### Development Commands

**CRITICAL**: Always use `cargo nextest run`, NEVER use `cargo test`

The integration tests MUST use nextest because they initialize global OpenTelemetry state. Running tests with `cargo test` causes tests to share global state across threads, leading to flaky failures and incorrect metrics attribution.

```bash
# Build the project
cargo build

# Build for release
cargo build --release

# ALWAYS use nextest for all testing
cargo nextest run

# Run tests with debug output
env TEST_LOG=1 cargo nextest run

# Run integration tests specifically
cargo nextest run -p integration-tests

# Run unit tests only (exclude integration tests)
cargo nextest run --workspace --exclude integration-tests

# Code formatting
cargo fmt

# Check formatting without changes
cargo fmt --check

# Linting
cargo clippy

# Approve snapshot changes (use approve, not review)
cargo insta approve
```

### Integration Test Setup

```bash
# Start OAuth2 server for authentication tests
cd ./crates/integration-tests && docker compose up -d

# Run integration tests
cargo nextest run -p integration-tests
```

### Code Quality Commands

```bash
# Fix old-style format strings (from Makefile)
make fix-format-strings

# Check format string compliance
make check-format-strings
```

### Why nextest is required
- Each test runs in its own process (not thread), providing isolation
- Global telemetry state doesn't leak between tests
- Tests can safely initialize their own service names and metrics
- Prevents flaky test failures due to shared OpenTelemetry providers

## Code Style

### Rust Coding Guidelines

#### Error Handling
**Always handle errors appropriately - never silently discard them:**

```rust
// Good: Propagate errors
let result = some_operation().await?;

// Good: Custom error handling
match some_operation().await {
    Ok(value) => process(value),
    Err(e) => handle_specific_error(e),
}

// Bad: Silent error discarding
let _ = some_operation().await;

// Bad: Panic on errors
let result = some_operation().await.unwrap();
```

**Prefer `anyhow::Result` over verbose error types:**

```rust
// Good: Clean error chain
async fn handle_tool_search(query: String) -> anyhow::Result<Vec<Tool>> {
    let index = get_search_index().await?;
    let results = index.search(&query).await?;
    parse_results(results)
}

// Bad: Verbose error handling
async fn handle_tool_search(query: String) -> Result<Vec<Tool>, anyhow::Error> {
    // ... same implementation
}
```

#### String Formatting
**Use modern Rust string interpolation:**

```rust
// Good
let message = format!("User {username} has {count} items");
log::debug!("creating stdio downstream service for {name}");
assert!(
    startup_duration < Duration::from_secs(5),
    "STDIO server startup took too long: {startup_duration:?}",
);

// Bad
let message = format!("User {} has {} items", username, count);
log::debug!("creating stdio downstream service for {}", name);
assert!(
    startup_duration < Duration::from_secs(5),
    "STDIO server startup took too long: {:?}",
    startup_duration
);
```

**When accessing fields or calling methods, interpolation is not needed:**

```rust
// Good: Direct field/method access
let message = format!("Status: {}", server.status());
let info = format!("User: {}", user.name);

// Bad: Unnecessary named interpolation
let message = format!("Status: {status}", status = server.status());
let info = format!("User: {name}", name = user.name);
```

#### Control Flow and Readability
**Avoid nested if-lets and matches. Use let-else with early return to reduce indentation:**

```rust
// Good: Early return with let-else
let Some(user) = get_user() else {
    return Err(anyhow!("User not found"));
};

let Some(profile) = user.profile() else {
    return Ok(Response::default());
};

process_profile(profile);

// Bad: Nested if-let
if let Some(user) = get_user() {
    if let Some(profile) = user.profile() {
        process_profile(profile);
    }
}
```

#### File Organization
- Prefer flat module structure: `src/user_service.rs` over `src/user_service/mod.rs`
- **Prefer existing files**: Add functionality to existing files unless creating a new logical component
- **Minimal comments**: Only write comments describing _why_ code is written a certain way, or to point out non-obvious details

#### Code Quality Principles
- **Prioritize correctness and clarity** over speed and efficiency unless explicitly required
- **Debug logging**: Use debug level for most logging; avoid info/warn/error unless absolutely necessary

### Import Conventions
- Follow existing patterns in each crate
- Group standard library imports, external crates, then local imports
- Use absolute imports from crate root when possible

### Naming Conventions
- **Functions**: `snake_case`
- **Types**: `PascalCase` 
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Variables**: `snake_case`
- **Modules**: `snake_case`

## Testing

### Framework & Patterns
- **Framework**: Rust with `cargo-nextest` (required for this project)
- **Snapshot testing**: Uses `insta` crate with **INLINE snapshots only**
- **Multi-line strings**: Uses `indoc!` macro for clean formatting
- **Test file patterns**: Standard Rust patterns (`tests/`, `#[cfg(test)]` modules)
- **Integration tests**: Docker Compose setup with Hydra OAuth2 server

### Testing Philosophy
**When tests fail, fix the code, not the test.**

Key principles:
- **Tests should be meaningful** - Avoid tests that always pass regardless of behavior
- **Test actual functionality** - Call the functions being tested, don't just check side effects
- **Failing tests are valuable** - They reveal bugs or missing features
- **Fix the root cause** - When a test fails, fix the underlying issue, don't hide the test
- **Test edge cases** - Tests that reveal limitations help improve the code

### Snapshot Testing (INLINE ONLY - CRITICAL)

**STRICT RULES**:
1. Use `assert_eq!` ONLY for primitives (bool, int, status codes)
2. Use insta snapshots for ALL complex types (structs, vecs, JSON)
3. Snapshots MUST be inline (`@r###"..."###`) - NO external files
4. NEVER use `assert_eq!` to compare complex objects

```rust
// GOOD: Inline snapshot for complex data
insta::assert_json_snapshot!(response, @r###"
{
  "tools": ["search", "execute"],
  "status": "ready"
}
"###);

// GOOD: Simple assertions for primitives only
assert_eq!(response.status(), 200);
assert!(config.enabled);

// BAD: NEVER do this for complex types
assert_eq!(response.tools.len(), 2);  // NO! Use snapshot
assert_eq!(response.tools[0], "search");  // NO! Use snapshot
```

### Test Naming
**Don't prefix test functions with `test_`:**

```rust
// Good: Clean and short test name
#[tokio::test]
async fn user_can_search_tools() { ... }

// Bad: Verbose test name
#[tokio::test]
async fn test_user_can_search_tools() { ... }
```

### Multi-line Strings in Tests
**Use `indoc!` macro for clean multi-line strings:**

```rust
use indoc::{indoc, formatdoc};

// Good: Use indoc for multi-line strings
let message = indoc! {r#"
    This is a string.
    This is another string.
"#};

// Good: Use formatdoc with string interpolation
let name = "Alice";
let message = formatdoc! {r#"
    Hello, {name}!
    Welcome to our platform.
"#};
```

## Security

### Key Security Principles
- **JWT Authentication**: JWT-Compact for token handling with proper validation
- **OAuth2 Integration**: Hydra OAuth2 server for authentication flows
- **TLS Security**: Rustls for secure downstream connections
- **Rate Limiting**: Multi-level protection (global, per-IP, per-user, per-group)
- **Secret Management**: Proper handling of API keys and authentication tokens

### Rate Limiting Configuration
**Token Rate Limiting Hierarchy** (highest to lowest priority):
1. **Model + Group**: Specific model with specific group
2. **Model Default**: Specific model without group  
3. **Provider + Group**: Provider-level with specific group
4. **Provider Default**: Provider-level without group

**Important**: Rate limits only count input tokens (`input_token_limit`). Output tokens are NOT considered.

### Data Protection
- Never log or expose API keys, JWTs, or other secrets
- Validate all user inputs at API boundaries
- Use proper error handling to avoid information leakage

## Configuration

### Configuration Management
- **TOML-based**: Primary configuration in `nexus.toml`
- **Environment Variables**: Support for environment variable substitution
- **Validation**: Comprehensive validation at startup with clear error messages

### Required Configuration
**Nexus requires at least one downstream service when enabled:**

- **MCP**: When `mcp.enabled = true`, at least one server must be configured in `mcp.servers`
- **LLM**: When `llm.enabled = true`, at least one provider must be configured in `llm.providers`
  - Each LLM provider MUST have at least one model explicitly configured
  - Models are configured under `[llm.providers.<name>.models.<model-id>]`
  - Model IDs containing dots must be quoted: `[llm.providers.google.models."gemini-1.5-flash"]`

### Environment Setup
1. Copy `.env.example` to `.env` and configure as needed
2. Configure OAuth2 settings for authentication
3. Set up MCP server connections and LLM provider API keys
4. Configure rate limiting and observability as needed

### Development Environment
- **Rust**: Latest stable release
- **Docker Compose**: For integration testing
- **Redis** (optional): For distributed rate limiting
- **OpenTelemetry Collector** (optional): For metrics export

## Directory Structure & File Organization

### Reports Directory
ALL project reports and documentation should be saved to the `reports/` directory:

```
nexus/
├── reports/              # All project reports and documentation
│   └── *.md             # Various report types
├── temp/                # Temporary files and debugging
└── [other directories]
```

### Report Generation Guidelines
**Important**: ALL reports should be saved to the `reports/` directory with descriptive names:

**Implementation Reports:**
- Phase validation: `PHASE_X_VALIDATION_REPORT.md`
- Implementation summaries: `IMPLEMENTATION_SUMMARY_[FEATURE].md`
- Feature completion: `FEATURE_[NAME]_REPORT.md`

**Testing & Analysis Reports:**
- Test results: `TEST_RESULTS_[DATE].md`
- Coverage reports: `COVERAGE_REPORT_[DATE].md`
- Performance analysis: `PERFORMANCE_ANALYSIS_[SCENARIO].md`
- Security scans: `SECURITY_SCAN_[DATE].md`

**Quality & Validation:**
- Code quality: `CODE_QUALITY_REPORT.md`
- Dependency analysis: `DEPENDENCY_REPORT.md`
- API compatibility: `API_COMPATIBILITY_REPORT.md`

### Temporary Files & Debugging
All temporary files, debugging scripts, and test artifacts should be organized in a `/temp` folder:

**Temporary File Organization:**
- **Debug scripts**: `temp/debug-*.js`, `temp/analyze-*.py`
- **Test artifacts**: `temp/test-results/`, `temp/coverage/`
- **Generated files**: `temp/generated/`, `temp/build-artifacts/`
- **Logs**: `temp/logs/debug.log`, `temp/logs/error.log`

**Guidelines:**
- Never commit files from `/temp` directory
- Use `/temp` for all debugging and analysis scripts created during development
- Clean up `/temp` directory regularly or use automated cleanup
- Include `/temp/` in `.gitignore` to prevent accidental commits

### Claude Code Settings (.claude Directory)

The `.claude` directory contains Claude Code configuration files with specific version control rules:

#### Version Controlled Files (commit these)
- `.claude/settings.json` - Shared team settings for hooks, tools, and environment
- `.claude/commands/*.md` - Custom slash commands available to all team members
- `.claude/hooks/*.sh` - Hook scripts for automated validations and actions

#### Ignored Files (do NOT commit)
- `.claude/settings.local.json` - Personal preferences and local overrides
- Any `*.local.json` files - Personal configuration not meant for sharing

## Dependency Management

### Workspace Dependencies
**All dependencies must be added to the workspace `Cargo.toml` (`./Cargo.toml`), not individual crate `Cargo.toml` files:**

```toml
# In Cargo.toml [workspace.dependencies]
new-crate = "1.0.0"

# In crates/*/Cargo.toml or nexus/Cargo.toml [dependencies]
new-crate.workspace = true
```

### Adding New Dependencies
1. Add the dependency to `[workspace.dependencies]` in root `Cargo.toml`
2. Reference it in individual crates using `dependency.workspace = true`
3. Enable specific features in individual crates as needed:

```toml
# Workspace defines base dependency
tokio = { version = "1.46.1", default-features = false }

# Individual crates enable needed features
tokio = { workspace = true, features = ["macros", "rt"] }
```

### Feature Management
- Keep `default-features = false` in workspace for minimal builds
- Enable only required features in individual crates
- Group related features logically (e.g., `["derive", "serde"]`)

## Agent Delegation & Tool Execution

### ⚠️ MANDATORY: Always Delegate to Specialists & Execute in Parallel

**When specialized agents are available, you MUST use them instead of attempting tasks yourself.**

**When performing multiple operations, send all tool calls (including Task calls for agent delegation) in a single message to execute them concurrently for optimal performance.**

### Available Specialized Agents for Nexus

This project has specialized Claude Code agents for specific domains:

#### Core Development Agents
- **`rust-code-reviewer`** - Code review for adherence to project-specific Rust coding standards and best practices. Use after writing or modifying Rust code.
- **`server-crate-specialist`** - Work on HTTP endpoints, Axum routing, middleware, authentication integration in `crates/server`
- **`mcp-crate-engineer`** - MCP router implementation, tool discovery, search functionality, execution routing in `crates/mcp`
- **`integration-test-engineer`** - Create, modify, debug integration tests in `crates/integration-tests` with Docker Compose

#### General Purpose Agents
- **`changelog-generator`** - Generate changelogs from git commit history (use immediately when bumping versions)

#### Example Agent Usage
```markdown
# When working on server endpoints:
Use server-crate-specialist for any modifications to crates/server/

# When working on MCP functionality:
Use mcp-crate-engineer for modifications to crates/mcp/

# When working on integration tests:
Use integration-test-engineer for crates/integration-tests/

# After significant code changes:
Use rust-code-reviewer to ensure compliance with coding standards
```

### Key Principles
- **Agent Delegation**: Always check if a specialized agent exists for your task domain
- **Complex Problems**: Delegate to domain experts, use diagnostic agents when scope is unclear
- **Multiple Agents**: Send multiple Task tool calls in a single message to delegate to specialists in parallel
- **DEFAULT TO PARALLEL**: Unless you have a specific reason why operations MUST be sequential (output of A required for input of B), always execute multiple tools simultaneously
- **Plan Upfront**: Think "What information do I need to fully answer this question?" Then execute all searches together

### Critical: Always Use Parallel Tool Calls

**Err on the side of maximizing parallel tool calls rather than running sequentially.**

**IMPORTANT: Send all tool calls in a single message to execute them in parallel.**

**These cases MUST use parallel tool calls:**
- Searching for different patterns (imports, usage, definitions)
- Multiple grep searches with different regex patterns
- Reading multiple files or searching different directories
- Combining Glob with Grep for comprehensive results
- Agent delegations with multiple Task calls to different specialists
- Any information gathering where you know upfront what you're looking for

**Sequential calls ONLY when:**
You genuinely REQUIRE the output of one tool to determine the usage of the next tool.

**Performance Impact:** Parallel tool execution is 3-5x faster than sequential calls, significantly improving user experience.

**Remember:** This is not just an optimization—it's the expected behavior. Both delegation and parallel execution are requirements, not suggestions.

## Key Technologies

### Core Stack
- **Rust 2024 Edition**: Primary programming language
- **Axum**: Web framework for HTTP routing and middleware
- **Tantivy**: Full-text search engine for tool indexing
- **Tokio**: Async runtime
- **Serde**: Serialization/deserialization for JSON/TOML
- **RMCP**: Rust MCP client/server implementation
- **Anyhow**: Error handling with context and backtrace

### Infrastructure & Operations
- **Reqwest**: HTTP client for external API calls
- **JWT-Compact**: JWT token handling for authentication
- **Tower/Tower-HTTP**: Middleware and service layers
- **Rustls**: TLS implementation for secure connections
- **OpenTelemetry**: Observability with metrics collection and OTLP export
- **Governor**: Rate limiting with token bucket algorithm
- **Mini-moka**: In-memory caching for rate limit buckets
- **Redis**: Redis support for distributed rate limiting
- **Deadpool**: Connection pooling

### Development & Testing
- **Insta**: Snapshot testing framework
- **Clap**: Command-line argument parsing
- **Logforth**: Structured logging with tracing support
- **Docker Compose**: For integration testing with Hydra OAuth2 server
- **cargo-nextest**: Required test runner for this project

## Architecture Patterns

### Error Propagation
Use the `?` operator liberally for clean error propagation:

```rust
// Good: Clean error chain
async fn handle_tool_search(query: String) -> anyhow::Result<Vec<Tool>> {
    let index = get_search_index().await?;
    let results = index.search(&query).await?;
    parse_results(results)
}
```

### Model Configuration Requirements
Every LLM provider MUST have explicit model configuration:

```rust
// In config parsing - models are required
#[derive(Deserialize)]
pub struct ProviderConfig {
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub api_key: Option<String>,
    #[serde(default, deserialize_with = "deserialize_non_empty_models_with_default")]
    pub models: BTreeMap<String, ModelConfig>,  // Must have at least one entry
}
```

### Important Project Notes

#### Tools Behavior
The Nexus server will always return two tools when listed: `search` and `execute`. If you want to find tools in tests you created, you have to search for them.

#### Telemetry Metrics
- Metrics are collected via middleware and only execute when `telemetry.exporters.otlp.enabled = true`
- Zero overhead when disabled
- MCP metrics are deterministic - exact counts must match test expectations
- HTTP-level metrics may vary due to batching

Remember: This codebase values **correctness and maintainability** over premature optimization. Write clear, safe code that properly handles errors and follows Rust best practices.