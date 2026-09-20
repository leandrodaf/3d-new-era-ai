# MCP tool contract

`tool-surface.json` contains the complete native tool definitions, keyed by name.
The contract test compares the set of tools and each definition, including
schemas and descriptions. JSON object formatting/order is irrelevant; field,
array, type and description changes are checked even when byte counts match.

For an intentional API change, regenerate the snapshot explicitly:

```sh
cargo test -p newera-mcp update_tool_surface_snapshot -- --ignored
```

Review the JSON diff alongside the implementation, then run:

```sh
cargo test -p newera-mcp
```

Normal tests and CI never rewrite the snapshot. Browser-specific descriptions
and transport behavior remain covered separately by the browser MCP E2E test.
