use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use dashmap::DashMap;
use rmcp::{
    model::{CallToolRequestParams, CallToolResult, Tool},
    service::RunningService,
    transport::TokioChildProcess,
    RoleClient, ServiceExt,
};
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::config::BackendConfig;

/// A connected MCP backend with cached tool list.
struct LiveBackend {
    client: RunningService<RoleClient, ()>,
    tools: Vec<Tool>,
}

/// Manages connections to all MCP backends.
/// Handles connect, reconnect, tool discovery, and tool invocation.
pub struct BackendPool {
    configs: HashMap<String, BackendConfig>,
    connections: DashMap<String, Arc<RwLock<LiveBackend>>>,
}

impl BackendPool {
    pub fn new(configs: HashMap<String, BackendConfig>) -> Self {
        Self {
            configs,
            connections: DashMap::new(),
        }
    }

    /// Connect to all configured backends concurrently with per-backend timeout.
    /// Logs errors but doesn't fail.
    pub async fn connect_all(&self) {
        let mut handles = Vec::new();
        for (name, config) in &self.configs {
            let name = name.clone();
            let config = config.clone();
            let pool = self;
            handles.push(async move {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    pool.connect_one(&name, &config),
                )
                .await
                {
                    Ok(Ok(_)) => tracing::info!(backend = %name, "connected"),
                    Ok(Err(e)) => tracing::error!(backend = %name, error = %e, "failed to connect"),
                    Err(_) => tracing::error!(backend = %name, "connection timed out after 30s"),
                }
            });
        }
        futures::future::join_all(handles).await;
    }

    /// Connect to a single backend by name.
    async fn connect_one(&self, name: &str, config: &BackendConfig) -> Result<()> {
        let client = match config {
            BackendConfig::Stdio {
                command,
                args,
                cwd,
                env,
            } => {
                let mut cmd = Command::new(command);
                cmd.args(args);
                if let Some(dir) = cwd {
                    cmd.current_dir(dir);
                }
                for (k, v) in env {
                    cmd.env(k, v);
                }
                let transport = TokioChildProcess::new(cmd)?;
                ().serve(transport)
                    .await
                    .with_context(|| format!("stdio connect to '{name}'"))?
            }
            BackendConfig::Http { url } => {
                use rmcp::transport::StreamableHttpClientTransport;
                let transport = StreamableHttpClientTransport::from_uri(url.as_str());
                ().serve(transport)
                    .await
                    .with_context(|| format!("http connect to '{name}' at {url}"))?
            }
        };

        // Discover tools
        let tools_result = client
            .list_all_tools()
            .await
            .with_context(|| format!("list tools from '{name}'"))?;

        let tool_count = tools_result.len();
        let backend = LiveBackend {
            client,
            tools: tools_result,
        };

        self.connections
            .insert(name.to_string(), Arc::new(RwLock::new(backend)));

        tracing::info!(backend = %name, tools = tool_count, "tools discovered");
        Ok(())
    }

    /// Call a tool on a specific backend.
    pub async fn call_tool(
        &self,
        backend_name: &str,
        tool_name: &str,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult> {
        let conn = self
            .connections
            .get(backend_name)
            .with_context(|| format!("backend '{backend_name}' not connected"))?;

        let backend = conn.read().await;
        let owned_name = tool_name.to_string();
        let mut params = CallToolRequestParams::new(owned_name);
        if let Some(args) = arguments {
            params = params.with_arguments(args);
        }

        let result = backend
            .client
            .call_tool(params)
            .await
            .with_context(|| format!("call_tool '{tool_name}' on '{backend_name}'"))?;

        Ok(result)
    }

    /// Call a tool by searching across ALL backends for the tool name.
    pub async fn call_tool_any(
        &self,
        tool_name: &str,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult> {
        // Find which backend has this tool
        for entry in self.connections.iter() {
            let name = entry.key().clone();
            let backend = entry.value().read().await;
            if backend.tools.iter().any(|t| t.name.as_ref() == tool_name) {
                drop(backend);
                return self.call_tool(&name, tool_name, arguments).await;
            }
        }

        anyhow::bail!("tool '{tool_name}' not found on any backend")
    }

    /// List all raw Tool objects from all backends (for MCP proxy).
    pub async fn list_all_tools_raw(&self) -> Vec<Tool> {
        let mut all = Vec::new();
        for entry in self.connections.iter() {
            let backend = entry.value().read().await;
            for tool in &backend.tools {
                all.push(tool.clone());
            }
        }
        all
    }

    /// List all tools across all backends.
    pub async fn list_all_tools(&self) -> Vec<ToolInfo> {
        let mut all = Vec::new();
        for entry in self.connections.iter() {
            let backend_name = entry.key().clone();
            let backend = entry.value().read().await;
            for tool in &backend.tools {
                all.push(ToolInfo {
                    backend: backend_name.clone(),
                    name: tool.name.to_string(),
                    description: tool.description.as_deref().unwrap_or_default().to_string(),
                });
            }
        }
        all.sort_by(|a, b| a.name.cmp(&b.name));
        all
    }

    /// List tools for a specific backend.
    pub async fn list_tools(&self, backend_name: &str) -> Result<Vec<ToolInfo>> {
        let conn = self
            .connections
            .get(backend_name)
            .with_context(|| format!("backend '{backend_name}' not connected"))?;

        let backend = conn.read().await;
        let tools = backend
            .tools
            .iter()
            .map(|t| ToolInfo {
                backend: backend_name.to_string(),
                name: t.name.to_string(),
                description: t.description.as_deref().unwrap_or_default().to_string(),
            })
            .collect();

        Ok(tools)
    }

    /// Get list of connected backend names.
    pub fn connected_backends(&self) -> Vec<String> {
        self.connections.iter().map(|e| e.key().clone()).collect()
    }

    /// Get list of configured backend names.
    pub fn configured_backends(&self) -> Vec<String> {
        self.configs.keys().cloned().collect()
    }

    /// Reconnect a specific backend.
    pub async fn reconnect(&self, name: &str) -> Result<()> {
        // Drop old connection — just remove it, the Arc will clean up
        self.connections.remove(name);

        let config = self
            .configs
            .get(name)
            .with_context(|| format!("no config for backend '{name}'"))?;

        self.connect_one(name, config).await
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ToolInfo {
    pub backend: String,
    pub name: String,
    pub description: String,
}
