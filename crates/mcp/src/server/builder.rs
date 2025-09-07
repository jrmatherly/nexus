//! Builder for the MCP server.

use super::{McpServer, handler::McpHandler, metrics::MetricsMiddleware};
use rate_limit::RateLimitManager;
use std::sync::Arc;

/// Builder for creating an MCP server handler.
pub(crate) struct McpServerBuilder {
    pub(crate) config: config::Config,
    pub(crate) rate_limit_manager: Option<Arc<RateLimitManager>>,
}

impl McpServerBuilder {
    /// Create a new MCP server builder.
    pub fn new(config: config::Config) -> Self {
        Self {
            config,
            rate_limit_manager: None,
        }
    }

    /// Set the rate limit manager.
    pub fn rate_limit_manager(mut self, manager: Arc<RateLimitManager>) -> Self {
        self.rate_limit_manager = Some(manager);
        self
    }

    /// Build the MCP handler with optional metrics middleware.
    pub(crate) async fn build(self) -> anyhow::Result<McpHandler> {
        // Check if metrics are enabled
        let metrics_enabled = self
            .config
            .telemetry
            .as_ref()
            .is_some_and(|t| t.metrics_otlp_config().is_some());

        let mcp_server = McpServer::new(self).await?;

        // Create handler with or without metrics
        let handler = if metrics_enabled {
            McpHandler::WithMetrics(MetricsMiddleware::new(mcp_server))
        } else {
            McpHandler::WithoutMetrics(mcp_server)
        };

        Ok(handler)
    }
}
