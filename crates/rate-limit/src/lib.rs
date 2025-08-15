//! Rate limiting functionality for Nexus.
//!
//! This crate provides rate limiting capabilities with support for:
//! - Global rate limits
//! - Per-IP rate limits
//! - Per-MCP-server and per-tool rate limits
//!
//! Currently supports in-memory storage using the governor crate.
//! Redis support will be added in future versions.

#![deny(missing_docs)]

mod error;
mod manager;
mod request;
mod storage;
mod token;

pub use error::RateLimitError;
pub use manager::RateLimitManager;
pub use request::{RateLimitRequest, RateLimitRequestBuilder};
pub use storage::{InMemoryStorage, RateLimitStorage, StorageError};
pub use token::{TokenRateLimitManager, TokenRateLimitRequest, resolve_token_rate_limit};
