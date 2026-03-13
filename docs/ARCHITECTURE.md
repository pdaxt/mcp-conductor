# Architecture

## Overview

MCP Conductor is a Rust gateway that manages MCP backend servers and exposes them through a single HTTP endpoint. It handles two transport types:

- **stdio** — spawns backends as child processes, communicates via stdin/stdout
- **http** — connects to already-running MCP servers over Streamable HTTP

```
AI Agent ──HTTP──▸ MCP Conductor (:9090)
                    ├── /mcp     → Streamable HTTP (MCP protocol)
                    └── /api/*   → REST API (JSON)
                         │
                    BackendPool
                    ├── spawn ──▸ search-mcp (stdio)
                    ├── spawn ──▸ crm-mcp (stdio)
                    ├── connect ▸ remote-api (http)
                    └── spawn ──▸ secrets-mcp (stdio)
```

## Components

### `main.rs` — Server Entrypoint (113 lines)

Sets up:
1. **Tracing** — structured logging with env filter
2. **BackendPool** — creates pool, connects all backends
3. **MCP Service** — `StreamableHttpService` with `ProxyServer` handler
4. **Axum Router** — mounts MCP at `/mcp`, REST at `/api/*`, health at `/health`
5. **Graceful Shutdown** — Ctrl+C → cancellation token → clean exit

### `config.rs` — Configuration (105 lines)

Parses `MCP Conductor.toml` into typed structs:

```rust
GatewayConfig
├── ServerConfig { host, port, timeout_secs, api_key }
├── HashMap<String, BackendConfig>    // stdio or http
└── HashMap<String, WebhookConfig>    // webhook → tool mapping
```

`BackendConfig` is a tagged enum:
- `Stdio { command, args, cwd, env }` — MCP Conductor spawns the process
- `Http { url }` — MCP Conductor connects to existing server

### `pool.rs` — Backend Pool (221 lines)

The core component. Manages the lifecycle of all backends:

**Connection:**
- `connect_all()` — spawns all backends concurrently with 30s timeout each
- `connect_one()` — spawns a single backend, runs MCP handshake, discovers tools
- `reconnect()` — drops old connection, re-connects (hot reconnect)

**Tool Routing:**
- `call_tool_any()` — searches all backends for a tool name, routes to correct one
- `call_tool()` — calls a tool on a specific named backend
- `list_all_tools()` — aggregates tools from all backends

**Data Structure:**
```rust
BackendPool {
    configs: HashMap<String, BackendConfig>,        // static config
    connections: DashMap<String, Arc<RwLock<LiveBackend>>>,  // live connections
}

LiveBackend {
    client: RunningService<RoleClient, ()>,  // MCP client connection
    tools: Vec<Tool>,                         // cached tool list
}
```

`DashMap` allows concurrent reads (tool lookups) without blocking other backends.

### `proxy.rs` — MCP Proxy (69 lines)

Implements `ServerHandler` from rmcp — this is what AI agent talks to:

- `get_info()` — returns server name "MCP Conductor" and capabilities
- `list_tools()` — returns all tools from all backends (flat list)
- `call_tool()` — routes to `pool.call_tool_any()`

Each MCP client (AI agent session) gets its own `ProxyServer` instance via `StreamableHttpService`, but all share the same `Arc<BackendPool>`.

### `routes.rs` — REST API (159 lines)

REST endpoints for scripts, webhooks, and debugging:

| Route | Handler | Purpose |
|-------|---------|---------|
| `GET /health` | `health()` | Backend status |
| `GET /api/tools` | `list_tools()` | All tools from all backends |
| `GET /api/backends/{name}/tools` | `list_backend_tools()` | Tools from one backend |
| `POST /api/tools/call` | `call_tool()` | Auto-routed tool call |
| `POST /api/backends/{name}/tools/{tool}` | `call_backend_tool()` | Direct backend call |
| `POST /api/backends/{name}/reconnect` | `reconnect_backend()` | Hot reconnect |
| `POST /api/webhooks/{hook}` | `webhook()` | Webhook → tool call |

### `middleware.rs` — Middleware (55 lines)

Request logging — logs method, path, status code, and duration for every request.

## Design Decisions

### Why stdio management matters

Most MCP gateways only proxy HTTP→HTTP. But the majority of MCP servers use **stdio** transport (they read from stdin, write to stdout). Someone needs to spawn and manage these processes. MCP Conductor does this natively:

```
Other gateways:  You start 20 processes → gateway proxies to them
MCP Conductor:           You write a TOML config → MCP Conductor starts everything
```

### Concurrent startup with isolation

`connect_all()` spawns all backends simultaneously using `tokio::spawn`. Each gets a 30-second timeout. If backend A hangs, backends B through Z still connect normally. This is critical when you have 10+ backends — sequential startup with one timeout would be unusable.

### Tool routing by name (no namespacing)

Tools appear with their original names — `search`, `vault_get`, `create_contact`. No `backend.tool` prefixing. This means:
- AI agents don't need to know about the gateway
- Existing tool calls work unchanged
- Trade-off: tool name collisions across backends aren't handled (last-write-wins)

The roadmap includes optional namespacing for users who need it.

### Hot reconnect

`POST /api/backends/{name}/reconnect` drops the old connection and re-spawns the backend. No gateway restart needed. Useful when:
- A backend crashes
- You rebuild a backend binary
- A backend gets into a bad state

## Tech Stack

| Crate | Version | Purpose |
|-------|---------|---------|
| [rmcp](https://github.com/anthropics/rust-mcp) | 1.1 | MCP protocol (client + server + Streamable HTTP) |
| [axum](https://github.com/tokio-rs/axum) | 0.8 | HTTP server |
| [tokio](https://github.com/tokio-rs/tokio) | 1.x | Async runtime |
| [dashmap](https://github.com/xacrimon/dashmap) | 6.x | Concurrent HashMap |
| [toml](https://github.com/toml-rs/toml) | 0.8 | Config parsing |
| tower-http | 0.6 | CORS, timeout, tracing middleware |

## Future Architecture

```
MCP Conductor (planned)
├── /mcp          — Streamable HTTP (current)
├── /api/*        — REST API (current)
├── /ws           — WebSocket for real-time events (planned)
├── /dashboard    — Web UI for monitoring (planned)
└── /metrics      — Prometheus metrics (planned)
     │
BackendPool (planned additions)
├── Health checks    — periodic ping, auto-reconnect on failure
├── Circuit breaker  — stop routing to failing backends
├── Metrics          — per-tool latency, error rate, call count
├── Config reload    — watch MCP Conductor.toml, hot-add/remove backends
└── SSE transport    — support for SSE-based MCP servers
```
