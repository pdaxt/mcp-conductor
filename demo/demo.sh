#!/bin/bash
# Demo script for mcpgw — records with asciinema + converts to GIF
# Run: cd ~/Projects/mcp-runtime && bash demo/demo.sh

set -e

CAST_FILE="/tmp/mcpgw-demo.cast"
GIF_FILE="demo/mcpgw-demo.gif"

echo "Recording demo..."

# Create a script that types commands with realistic delays
cat > /tmp/mcpgw-demo-script.sh << 'SCRIPT'
#!/bin/bash

# Simulated typing function
type_cmd() {
    echo ""
    for ((i=0; i<${#1}; i++)); do
        printf '%s' "${1:$i:1}"
        sleep 0.04
    done
    echo ""
    sleep 0.3
}

clear
echo ""
echo "  ┌─────────────────────────────────────────────┐"
echo "  │  mcpgw — MCP Gateway for AI Agents          │"
echo "  │  One endpoint. Every tool. Single binary.    │"
echo "  └─────────────────────────────────────────────┘"
echo ""
sleep 1.5

# Show the config
echo "$ cat mcpgw.toml"
sleep 0.5
cat << 'TOML'
[server]
host = "127.0.0.1"
port = 9090

[backends.search]
transport = "stdio"
command = "search-mcp"

[backends.secrets]
transport = "stdio"
command = "pqvault-unified"

[backends.crm]
transport = "stdio"
command = "dataxlr8-crm-mcp"
TOML
sleep 2

# Start the gateway
echo ""
echo "$ mcpgw mcpgw.toml"
sleep 0.5
echo "2026-03-13T10:00:01Z  INFO config loaded from mcpgw.toml (backends=3)"
sleep 0.3
echo "2026-03-13T10:00:01Z  INFO backend=search connected (12 tools, 340ms)"
sleep 0.2
echo "2026-03-13T10:00:01Z  INFO backend=secrets connected (14 tools, 210ms)"
sleep 0.2
echo "2026-03-13T10:00:02Z  INFO backend=crm connected (35 tools, 890ms)"
sleep 0.3
echo "2026-03-13T10:00:02Z  INFO mcpgw listening on http://127.0.0.1:9090"
echo ""
echo "  MCP endpoint (Claude Code connects here):"
echo "    http://127.0.0.1:9090/mcp"
echo ""
echo "  REST API:"
echo "    GET  /health"
echo "    GET  /api/tools"
echo "    POST /api/tools/call"
sleep 2

# Health check
echo ""
echo '$ curl -s localhost:9090/health | jq .'
sleep 0.5
echo '{
  "status": "healthy",
  "backends_connected": 3,
  "backends_configured": 3,
  "connected": ["search", "secrets", "crm"]
}'
sleep 1.5

# List tools
echo ""
echo '$ curl -s localhost:9090/api/tools | jq .count'
sleep 0.5
echo "61"
sleep 1

# Call a tool
echo ""
echo '$ curl -s -X POST localhost:9090/api/tools/call \'
echo '    -d '"'"'{"tool":"search","arguments":{"query":"rust mcp"}}'"'"' | jq .results[0]'
sleep 0.5
echo '{
  "title": "Building MCP Servers in Rust",
  "url": "https://example.com/rust-mcp-tutorial",
  "score": 0.95,
  "source": "brave"
}'
sleep 1.5

# Claude Code config
echo ""
echo "# Add to ~/.claude.json — replaces ALL individual MCP configs:"
sleep 0.3
echo '{ "mcpServers": { "gateway": { "type": "http", "url": "http://localhost:9090/mcp" } } }'
sleep 2

echo ""
echo "# One line. Every tool. ✓"
sleep 2
SCRIPT

chmod +x /tmp/mcpgw-demo-script.sh

# Record
asciinema rec "$CAST_FILE" \
  --cols 90 \
  --rows 35 \
  --command "bash /tmp/mcpgw-demo-script.sh" \
  --overwrite \
  --title "mcpgw demo"

# Convert to GIF
echo "Converting to GIF..."
agg "$CAST_FILE" "$GIF_FILE" \
  --theme monokai \
  --font-size 16 \
  --speed 1.2 \
  --cols 90 \
  --rows 35

echo ""
echo "Demo saved to $GIF_FILE"
echo "Size: $(du -h "$GIF_FILE" | cut -f1)"
