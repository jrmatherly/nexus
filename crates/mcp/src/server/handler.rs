//! MCP handler that conditionally applies metrics middleware.

use super::{McpServer, metrics::MetricsMiddleware};
use rmcp::{
    RoleServer, ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, ErrorData, GetPromptRequestParam, GetPromptResult, ListPromptsResult,
        ListResourcesResult, ListToolsResult, PaginatedRequestParam, ReadResourceRequestParam, ReadResourceResult,
    },
    service::RequestContext,
};

/// Wrapper enum to handle conditional metrics middleware
#[derive(Clone)]
pub(crate) enum McpHandler {
    WithMetrics(MetricsMiddleware<McpServer>),
    WithoutMetrics(McpServer),
}

impl ServerHandler for McpHandler {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        match self {
            McpHandler::WithMetrics(handler) => handler.get_info(),
            McpHandler::WithoutMetrics(handler) => handler.get_info(),
        }
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        match self {
            McpHandler::WithMetrics(handler) => handler.call_tool(params, context).await,
            McpHandler::WithoutMetrics(handler) => handler.call_tool(params, context).await,
        }
    }

    async fn list_tools(
        &self,
        params: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        match self {
            McpHandler::WithMetrics(handler) => handler.list_tools(params, context).await,
            McpHandler::WithoutMetrics(handler) => handler.list_tools(params, context).await,
        }
    }

    async fn list_prompts(
        &self,
        params: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        match self {
            McpHandler::WithMetrics(handler) => handler.list_prompts(params, context).await,
            McpHandler::WithoutMetrics(handler) => handler.list_prompts(params, context).await,
        }
    }

    async fn get_prompt(
        &self,
        params: GetPromptRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, ErrorData> {
        match self {
            McpHandler::WithMetrics(handler) => handler.get_prompt(params, context).await,
            McpHandler::WithoutMetrics(handler) => handler.get_prompt(params, context).await,
        }
    }

    async fn list_resources(
        &self,
        params: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        match self {
            McpHandler::WithMetrics(handler) => handler.list_resources(params, context).await,
            McpHandler::WithoutMetrics(handler) => handler.list_resources(params, context).await,
        }
    }

    async fn read_resource(
        &self,
        params: ReadResourceRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        match self {
            McpHandler::WithMetrics(handler) => handler.read_resource(params, context).await,
            McpHandler::WithoutMetrics(handler) => handler.read_resource(params, context).await,
        }
    }
}
