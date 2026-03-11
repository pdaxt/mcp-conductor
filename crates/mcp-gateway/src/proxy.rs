use std::sync::Arc;

use rmcp::{model::*, service::RequestContext, ErrorData as McpError, RoleServer, ServerHandler};

use crate::pool::BackendPool;

/// An MCP Server that proxies all tool calls to the BackendPool.
/// Claude Code connects to this via Streamable HTTP — it sees all tools
/// from all backends as if they were native tools on a single MCP server.
#[derive(Clone)]
pub struct ProxyServer {
    pool: Arc<BackendPool>,
}

impl ProxyServer {
    pub fn new(pool: Arc<BackendPool>) -> Self {
        Self { pool }
    }
}

impl ServerHandler for ProxyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("mcpgw", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "MCP Gateway: Unified proxy to all backend MCP servers. \
             All tools from all backends are available here."
                    .to_string(),
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        // Get real tool definitions from backends, not just names
        let tools = self.pool.list_all_tools_raw().await;

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let tool_name = request.name.as_ref();
        let arguments = request.arguments.clone();

        match self.pool.call_tool_any(tool_name, arguments).await {
            Ok(result) => Ok(result),
            Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Error: {e}"
            ))])),
        }
    }
}
