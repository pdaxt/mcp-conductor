use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level gateway configuration (loaded from TOML).
#[derive(Debug, Deserialize)]
pub struct GatewayConfig {
    /// HTTP server settings
    #[serde(default)]
    pub server: ServerConfig,

    /// MCP backend servers to connect to
    #[serde(default)]
    pub backends: HashMap<String, BackendConfig>,

    /// Webhook route mappings
    #[serde(default)]
    pub webhooks: HashMap<String, WebhookConfig>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ServerConfig {
    /// Listen address (default: 0.0.0.0)
    #[serde(default = "default_host")]
    pub host: String,

    /// Listen port (default: 8080)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Request timeout in seconds (default: 30)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Optional API key for auth (header: X-API-Key)
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "transport")]
pub enum BackendConfig {
    /// Connect to MCP server via stdio (spawn child process)
    #[serde(rename = "stdio")]
    Stdio {
        /// Command to run
        command: String,
        /// Arguments
        #[serde(default)]
        args: Vec<String>,
        /// Working directory
        cwd: Option<PathBuf>,
        /// Environment variables
        #[serde(default)]
        env: HashMap<String, String>,
    },

    /// Connect to MCP server via Streamable HTTP
    #[serde(rename = "http")]
    Http {
        /// URL of the remote MCP server
        url: String,
    },
}

/// Maps an incoming webhook path to a tool call pipeline.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct WebhookConfig {
    /// Which backend to route to
    pub backend: String,
    /// Tool to call
    pub tool: String,
    /// Optional: JSONPath mapping from webhook body to tool arguments
    #[serde(default)]
    pub arg_map: HashMap<String, String>,
}

impl GatewayConfig {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            timeout_secs: default_timeout(),
            api_key: None,
        }
    }
}

fn default_host() -> String {
    "0.0.0.0".into()
}
fn default_port() -> u16 {
    8080
}
fn default_timeout() -> u64 {
    30
}
