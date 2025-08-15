//! Storage backends for rate limiting.

use std::net::IpAddr;
use std::time::Duration;

pub mod memory;
pub mod redis;
pub mod redis_pool;

pub use memory::InMemoryStorage;

/// Result type for rate limit checks.
pub struct RateLimitResult {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Time to wait before retrying if not allowed.
    pub retry_after: Option<Duration>,
}

/// Context for token rate limiting to help with cache key generation.
#[derive(Debug, Clone)]
pub struct TokenRateLimitContext<'a> {
    /// Client identifier.
    pub client_id: &'a str,
    /// Group identifier (optional).
    pub group: Option<&'a str>,
    /// Provider name (e.g., "openai", "anthropic").
    pub provider: &'a str,
    /// Model name (e.g., "gpt-4", "claude-3").
    pub model: Option<&'a str>,
}

/// Context for general rate limiting to help with cache key generation.
#[derive(Debug, Clone)]
pub enum RateLimitContext<'a> {
    /// Global rate limit (applies to all requests).
    Global,
    /// Per-IP rate limit.
    PerIp { ip: IpAddr },
    /// Per-MCP server rate limit.
    PerServer { server: &'a str },
    /// Per-MCP tool rate limit within a server.
    PerTool { server: &'a str, tool: &'a str },
}

/// Trait for rate limit storage backends.
#[allow(async_fn_in_trait)]
pub trait RateLimitStorage: Send + Sync {
    /// Check and potentially consume a token for general rate limiting.
    async fn check_and_consume(
        &self,
        context: &RateLimitContext<'_>,
        limit: u32,
        interval: Duration,
    ) -> Result<RateLimitResult, StorageError>;

    /// Check and potentially consume multiple tokens for token rate limiting.
    async fn check_and_consume_tokens(
        &self,
        context: &TokenRateLimitContext<'_>,
        tokens: u32,
        limit: u32,
        interval: Duration,
    ) -> Result<RateLimitResult, StorageError>;
}

/// Errors that can occur in storage backends.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// Internal storage error.
    #[error("Storage error: {0}")]
    Internal(String),
    /// Connection error.
    #[error("Connection error: {0}")]
    Connection(String),
    /// Query error.
    #[error("Query error: {0}")]
    Query(String),
}
