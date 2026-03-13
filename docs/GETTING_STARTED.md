# Getting Started

Get mcpgw running in under 2 minutes.

## Install

```bash
# From source (requires Rust toolchain)
git clone https://github.com/pdaxt/mcpgw.git
cd mcpgw
cargo install --path crates/mcp-gateway
```

The binary is called `mcpgw` and will be in `~/.cargo/bin/`.

## Create a Config

Create `mcpgw.toml` with your MCP backends:

```toml
[server]
host = "127.0.0.1"
port = 9090

# A stdio backend — mcpgw spawns this process
[backends.my-tool]
transport = "stdio"
command = "/path/to/my-mcp-server"
args = ["--some-flag"]
[backends.my-tool.env]
API_KEY = "your-key"

# An HTTP backend — mcpgw connects to this
[backends.remote]
transport = "http"
url = "http://10.0.0.5:8080/mcp"
```

See `mcpgw.example.toml` for a full example.

## Run

```bash
mcpgw mcpgw.toml
```

You'll see:
```
INFO config loaded from mcpgw.toml (backends=2)
INFO backend=my-tool connected (12 tools)
INFO backend=remote connected (8 tools)
INFO mcpgw listening on http://127.0.0.1:9090
```

## Connect Claude Code

Add one entry to `~/.claude.json`:

```json
{
  "mcpServers": {
    "gateway": {
      "type": "http",
      "url": "http://localhost:9090/mcp"
    }
  }
}
```

Restart Claude Code. All tools from all backends are now available.

## Verify

```bash
# Health check
curl http://localhost:9090/health

# List all tools
curl http://localhost:9090/api/tools | jq '.count'

# Call a tool
curl -X POST http://localhost:9090/api/tools/call \
  -H 'Content-Type: application/json' \
  -d '{"tool": "my_tool_name", "arguments": {"key": "value"}}'
```

## What's Next

- [Architecture](ARCHITECTURE.md) — how it works internally
- [Configuration Reference](CONFIGURATION.md) — all config options
- [REST API](../README.md#rest-api) — full API documentation

## Troubleshooting

### Backend fails to connect

Check the logs — mcpgw logs the error and backend name. Common causes:
- Binary not found at the specified `command` path
- Missing environment variables
- Backend crashes on startup

### Tool not found

The tool might be on a backend that failed to connect. Check `/health` to see which backends are connected:

```bash
curl http://localhost:9090/health | jq .connected
```

### Reconnect a crashed backend

```bash
curl -X POST http://localhost:9090/api/backends/my-tool/reconnect
```

No need to restart the gateway.
