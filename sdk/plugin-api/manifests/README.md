# Plugin Manifests

This directory contains runtime manifests consumed by `services/mcp-runtime`.

## Included sample
- `mock.mcp.echo.json`: starts `mock-mcp-plugin` via:
  - `cargo run -p mock-mcp-plugin`

## Quick smoke test
From repository root:

```bash
cargo run -p mcp-runtime -- validate
cargo run -p mcp-runtime -- list
cargo run -p mcp-runtime -- run --autostart
```

When running, type `list`, `stop mock.mcp.echo`, `start mock.mcp.echo`, or `quit`.
