# MCP Runtime Architecture

## Overview

MCP Runtime is a central gateway (`mcpgw`) that manages all MCP backend servers and exposes them as a single unified MCP endpoint. Claude Code connects to one HTTP URL and gets access to all tools from all backends.

```
Claude Code ──HTTP──> mcpgw (port 9090) ──stdio──> backend MCPs
                      /mcp (Streamable HTTP)         ├── mega (993 tools)
                      /api/* (REST)                   ├── dx-terminal (206 tools)
                      /health                         ├── forge (62 tools)
                                                      ├── mailforge (30 tools)
                                                      ├── ... (16 more)
                                                      └── router (6 tools)
```

## How It Works

1. **Startup**: `mcpgw` reads `mcpgw.toml`, spawns all backends concurrently (30s timeout each)
2. **Tool Discovery**: Each backend's tools are cached in `BackendPool`
3. **Routing**: When a tool is called, `call_tool_any()` searches all backends for the tool name and routes to the correct one
4. **Per-session MCP**: Each Claude Code client gets its own MCP Server instance (via `StreamableHttpService`), but all share the same `Arc<BackendPool>`

## Connection Details

```
MCP endpoint:  http://127.0.0.1:9090/mcp
REST API:      http://127.0.0.1:9090/api/*
Health check:  http://127.0.0.1:9090/health
```

Claude Code config:
```json
{ "type": "http", "url": "http://localhost:9090/mcp" }
```

## Backend Inventory (20 backends, 1,458 tools)

| Backend | Tools | Type | Description |
|---------|-------|------|-------------|
| mega | 993 | core | Unified MCP with 835+ tools across data, API, meta, content, subprocess, ML |
| dx-terminal | 206 | core | Agent orchestration, task management, issue tracking |
| forge | 62 | dev | Development tooling |
| mailforge | 30 | email | AI-native email infrastructure via SES |
| autocad | 18 | cad | Professional CAD drawing creation and DXF export |
| recon | 18 | security | Attack surface reconnaissance scanner |
| stripe | 18 | payment | Stripe payment API |
| marketing | 17 | content | Multi-platform social media posting |
| pqvault | 14 | secrets | Quantum-proof secrets management (unified from 4 shards) |
| bskiller-email | 10 | bskiller | Email campaigns via Resend |
| bskiller-research | 9 | bskiller | Source finding and verification |
| media | 9 | content | Video/audio download and processing |
| bskiller-admin | 8 | bskiller | Admin dashboard and system health |
| bskiller-scraper | 8 | bskiller | Newsletter scraping and story extraction |
| bskiller-score | 7 | bskiller | BSAF v1.0 scientific BS scoring |
| books | 7 | content | Book search and download |
| github-crawler | 7 | dev | GitHub repository crawling |
| bskiller-growth | 6 | bskiller | Subscriber management and Stripe checkout |
| router | 6 | core | Task routing to find the right MCP tool |
| bskiller-publish | 5 | bskiller | HTML analysis pages and LinkedIn posts |

## Key Design Decisions

### Why a Gateway (not monolith)

- **Independent failure**: One crashed MCP doesn't take down others
- **Independent deployment**: Rebuild one MCP without touching others
- **Memory isolation**: Each backend has its own process space
- **Hot reconnect**: `POST /api/backends/{name}/reconnect` reconnects one backend

### Concurrent Connection with Timeout

`connect_all()` spawns all backends concurrently with a 30s timeout per backend. One slow/hanging backend doesn't block startup. Previously this was sequential and a single hung backend (e.g., VPN) would prevent the HTTP server from starting.

### PQVault Consolidation (4 → 1)

Merged `pqvault-mcp` (7), `pqvault-env-mcp` (3), `pqvault-health-mcp` (3), `pqvault-proxy-mcp` (1) into a single `pqvault-unified` binary (14 tools). All 4 shared the same state (`VaultHolder` + `UsageTracker`), so merging gives better consistency and saves 3 processes.

### BSKiller Stays Separate (7 MCPs)

Each BSKiller MCP has its own SQLite database and domain logic (5,830 lines of tool code). Merging would require significant refactoring with minimal benefit — the gateway already presents them as a unified interface. Total overhead: ~21MB RAM for 7 tiny Rust processes.

### Mega (993 tools) — Future Decomposition Target

The `mega` backend is the real monolith. It should eventually be split by domain (cloudflare, google, video, etc.) into ~10 focused MCPs of 50-100 tools each. The gateway makes this transparent to clients.

## Auto-Start (launchd)

```
~/Library/LaunchAgents/com.mcpruntime.mcpgw.plist
```

- **RunAtLoad**: Starts on login
- **KeepAlive**: Restarts on crash
- **Logs**: `~/.mcpgw/mcpgw.log`

```bash
# Restart
launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.mcpruntime.mcpgw.plist
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.mcpruntime.mcpgw.plist

# Check health
curl http://localhost:9090/health
```

## Config Reference (mcpgw.toml)

```toml
[server]
host = "127.0.0.1"
port = 9090
timeout_secs = 120

[backends.name]
transport = "stdio"          # or "http"
command = "/path/to/binary"
args = ["--flag"]
cwd = "/optional/workdir"
[backends.name.env]
KEY = "value"
```

## Crate Structure

```
mcp-runtime/
├── Cargo.toml              # Workspace root
├── mcpgw.toml              # Production config (20 backends)
├── crates/
│   └── mcp-gateway/
│       └── src/
│           ├── main.rs     # Server entrypoint, Axum router
│           ├── config.rs   # TOML config parsing
│           ├── pool.rs     # BackendPool (DashMap, connect, route)
│           ├── proxy.rs    # MCP Streamable HTTP proxy (ServerHandler)
│           ├── routes.rs   # REST API routes
│           └── middleware.rs # Auth + logging middleware
└── docs/
    └── ARCHITECTURE.md     # This file
```

## Tech Stack

- **rmcp 1.1** — Official Rust MCP SDK (client + server + transport)
- **Axum 0.8** — HTTP server
- **DashMap 6** — Concurrent connection pool
- **tokio** — Async runtime
- **launchd** — macOS service management
