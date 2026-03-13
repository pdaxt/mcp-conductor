# Getting Started

Get MCP Conductor running in under 2 minutes.

## Install

```bash
# From source (requires Rust toolchain)
git clone https://github.com/pdaxt/mcp-conductor.git
cd mcp-conductor
cargo install --path crates/mcp-gateway
```

The binary `mcp-conductor` will be in `~/.cargo/bin/`.

## Create a Config

Create `conductor.toml` with your MCP backends:

```toml
[server]
host = "127.0.0.1"
port = 9090

# A stdio backend — Conductor spawns this process
[backends.my-tool]
transport = "stdio"
command = "/path/to/my-mcp-server"
args = ["--some-flag"]
[backends.my-tool.env]
API_KEY = "your-key"

# An HTTP backend — Conductor connects to this
[backends.remote]
transport = "http"
url = "http://10.0.0.5:8080/mcp"
```

See `mcpgw.example.toml` for a full example.

## Run

```bash
mcp-conductor conductor.toml
```

You'll see:
```
INFO config loaded from conductor.toml (backends=2)
INFO backend=my-tool connected (12 tools)
INFO backend=remote connected (8 tools)
INFO mcp-conductor listening on http://127.0.0.1:9090
```

## Connect Your AI Agent

Point any MCP-compatible client at `http://localhost:9090/mcp`:

### Claude Code / Claude Desktop

```json
{
  "mcpServers": {
    "conductor": {
      "type": "http",
      "url": "http://localhost:9090/mcp"
    }
  }
}
```

### Cursor / Windsurf

Add `http://localhost:9090/mcp` as an HTTP MCP server in settings.

### Python (any LLM)

```python
from mcp import ClientSession
from mcp.client.streamable_http import streamablehttp_client

async with streamablehttp_client("http://localhost:9090/mcp") as (r, w, _):
    async with ClientSession(r, w) as session:
        await session.initialize()
        tools = await session.list_tools()
        result = await session.call_tool("search", {"query": "hello"})
```

### curl / REST

```bash
curl -X POST http://localhost:9090/api/tools/call \
  -H 'Content-Type: application/json' \
  -d '{"tool": "my_tool_name", "arguments": {"key": "value"}}'
```

## Verify

```bash
# Health check
curl http://localhost:9090/health

# List all tools
curl http://localhost:9090/api/tools | jq '.count'
```

## What's Next

- [Architecture](ARCHITECTURE.md) — how it works internally
- [Configuration Reference](CONFIGURATION.md) — all config options
- [REST API](../README.md#rest-api) — full API documentation

## Troubleshooting

### Backend fails to connect

Check the logs — Conductor logs the error and backend name. Common causes:
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

No need to restart Conductor.
