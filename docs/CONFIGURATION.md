# Configuration Reference

mcpgw is configured via a TOML file (default: `mcpgw.toml`).

## Server

```toml
[server]
host = "127.0.0.1"      # Listen address (default: "0.0.0.0")
port = 9090              # Listen port (default: 8080)
timeout_secs = 120       # Request timeout in seconds (default: 30)
api_key = "secret"       # Optional: require X-API-Key header on /api/* routes
```

## Backends

### Stdio Transport

mcpgw spawns the process and communicates via stdin/stdout.

```toml
[backends.my-mcp]
transport = "stdio"
command = "/usr/local/bin/my-mcp-server"   # Required: path to binary
args = ["--verbose", "--port", "0"]         # Optional: command arguments
cwd = "/var/data"                           # Optional: working directory

[backends.my-mcp.env]                       # Optional: environment variables
DATABASE_URL = "sqlite:data.db"
API_KEY = "your-key"
```

### HTTP Transport

mcpgw connects to an already-running MCP server over Streamable HTTP.

```toml
[backends.remote-api]
transport = "http"
url = "http://10.0.0.5:8080/mcp"           # Required: MCP endpoint URL
```

## Webhooks

Map incoming webhook POSTs to MCP tool calls.

```toml
[webhooks.stripe_payment]
backend = "billing"          # Which backend to route to
tool = "handle_payment"      # Tool to call
```

When a POST hits `/api/webhooks/stripe_payment`, the request body is passed as arguments to the `handle_payment` tool on the `billing` backend.

## Full Example

```toml
[server]
host = "127.0.0.1"
port = 9090
timeout_secs = 120

# Search engine
[backends.search]
transport = "stdio"
command = "search-mcp"
[backends.search.env]
BRAVE_API_KEY = "BSA..."

# Secrets management
[backends.secrets]
transport = "stdio"
command = "pqvault-unified"

# CRM
[backends.crm]
transport = "stdio"
command = "crm-mcp"
args = ["--db", "postgres://localhost/crm"]

# Remote API
[backends.analytics]
transport = "http"
url = "http://analytics-server:8080/mcp"

# Webhook routing
[webhooks.payment_received]
backend = "crm"
tool = "record_payment"
```

## Environment Variables

mcpgw itself reads:

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info,mcp_gateway=debug` | Log level filter |

Backend environment variables are set per-backend in the config file.

## Tips

- **Keep secrets out of git.** Add `mcpgw.toml` to `.gitignore` and use `mcpgw.example.toml` as a template.
- **Use absolute paths** for `command` to avoid PATH issues.
- **Set `RUST_LOG=debug`** to see detailed MCP handshake and tool discovery logs.
- **Start small.** Add one backend, verify it works, then add more.
