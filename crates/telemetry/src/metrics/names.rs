//! Standard metric names following OpenTelemetry semantic conventions
//! See: https://opentelemetry.io/docs/specs/semconv/http/http-metrics/

/// HTTP server request duration in milliseconds
/// Note: Histograms automatically provide count and sum, so a separate counter is not needed
pub const HTTP_SERVER_REQUEST_DURATION: &str = "http.server.request.duration";

/// MCP tool call duration in milliseconds
/// Tracks the duration of MCP tool invocations including both built-in and downstream tools
pub const MCP_TOOL_CALL_DURATION: &str = "mcp.tool.call.duration";

/// MCP tools listing duration in milliseconds
/// Tracks the duration of listing available tools
pub const MCP_TOOLS_LIST_DURATION: &str = "mcp.tools.list.duration";

/// MCP prompt request duration in milliseconds
/// Tracks the duration of prompt-related operations (list/get)
pub const MCP_PROMPT_REQUEST_DURATION: &str = "mcp.prompt.request.duration";

/// MCP resource request duration in milliseconds  
/// Tracks the duration of resource-related operations (list/read)
pub const MCP_RESOURCE_REQUEST_DURATION: &str = "mcp.resource.request.duration";
