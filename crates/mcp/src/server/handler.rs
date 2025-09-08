//! MCP handler wrapper for the middleware pipeline.

use super::{McpServer, metrics::MetricsMiddleware, tracing::TracingMiddleware};
use rmcp::{
    RoleServer, ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, ErrorData, GetPromptRequestParam, GetPromptResult, ListPromptsResult,
        ListResourcesResult, ListToolsResult, PaginatedRequestParam, ReadResourceRequestParam, ReadResourceResult,
    },
    service::RequestContext,
};

/// Wrapper enum to handle different middleware combinations
#[derive(Clone)]
pub(crate) enum McpHandler {
    /// Both tracing and metrics enabled
    WithFullTelemetry(TracingMiddleware<MetricsMiddleware<McpServer>>),
    /// Only metrics enabled
    WithMetricsOnly(MetricsMiddleware<McpServer>),
    /// Only tracing enabled
    WithTracingOnly(TracingMiddleware<McpServer>),
    /// No telemetry
    WithoutTelemetry(McpServer),
}

impl ServerHandler for McpHandler {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        match self {
            McpHandler::WithFullTelemetry(handler) => handler.get_info(),
            McpHandler::WithMetricsOnly(handler) => handler.get_info(),
            McpHandler::WithTracingOnly(handler) => handler.get_info(),
            McpHandler::WithoutTelemetry(handler) => handler.get_info(),
        }
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        match self {
            McpHandler::WithFullTelemetry(handler) => handler.call_tool(params, context).await,
            McpHandler::WithMetricsOnly(handler) => handler.call_tool(params, context).await,
            McpHandler::WithTracingOnly(handler) => handler.call_tool(params, context).await,
            McpHandler::WithoutTelemetry(handler) => handler.call_tool(params, context).await,
        }
    }

    async fn list_tools(
        &self,
        params: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        match self {
            McpHandler::WithFullTelemetry(handler) => handler.list_tools(params, context).await,
            McpHandler::WithMetricsOnly(handler) => handler.list_tools(params, context).await,
            McpHandler::WithTracingOnly(handler) => handler.list_tools(params, context).await,
            McpHandler::WithoutTelemetry(handler) => handler.list_tools(params, context).await,
        }
    }

    async fn list_prompts(
        &self,
        params: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        match self {
            McpHandler::WithFullTelemetry(handler) => handler.list_prompts(params, context).await,
            McpHandler::WithMetricsOnly(handler) => handler.list_prompts(params, context).await,
            McpHandler::WithTracingOnly(handler) => handler.list_prompts(params, context).await,
            McpHandler::WithoutTelemetry(handler) => handler.list_prompts(params, context).await,
        }
    }

    async fn get_prompt(
        &self,
        params: GetPromptRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, ErrorData> {
        match self {
            McpHandler::WithFullTelemetry(handler) => handler.get_prompt(params, context).await,
            McpHandler::WithMetricsOnly(handler) => handler.get_prompt(params, context).await,
            McpHandler::WithTracingOnly(handler) => handler.get_prompt(params, context).await,
            McpHandler::WithoutTelemetry(handler) => handler.get_prompt(params, context).await,
        }
    }

    async fn list_resources(
        &self,
        params: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        match self {
            McpHandler::WithFullTelemetry(handler) => handler.list_resources(params, context).await,
            McpHandler::WithMetricsOnly(handler) => handler.list_resources(params, context).await,
            McpHandler::WithTracingOnly(handler) => handler.list_resources(params, context).await,
            McpHandler::WithoutTelemetry(handler) => handler.list_resources(params, context).await,
        }
    }

    async fn read_resource(
        &self,
        params: ReadResourceRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        match self {
            McpHandler::WithFullTelemetry(handler) => handler.read_resource(params, context).await,
            McpHandler::WithMetricsOnly(handler) => handler.read_resource(params, context).await,
            McpHandler::WithTracingOnly(handler) => handler.read_resource(params, context).await,
            McpHandler::WithoutTelemetry(handler) => handler.read_resource(params, context).await,
        }
    }
}
