# MCP Runtime Module

Purpose: lifecycle manager for MCP servers with isolation controls.

## Responsibilities
- Install/start/stop MCP servers
- Apply sandbox and capability policy
- Emit runtime health and audit events

## Local usage
From repository root:

```bash
cargo run -p mcp-runtime -- validate
cargo run -p mcp-runtime -- list
cargo run -p mcp-runtime -- run --autostart
```

Default manifest directory: `sdk/plugin-api/manifests`

During `run`, use stdin commands:
- `list`
- `start <plugin-id>`
- `stop <plugin-id>`
- `restart <plugin-id>`
- `quit`
