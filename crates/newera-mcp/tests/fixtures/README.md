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

Normal tests and CI never rewrite the snapshot. What the browser tab and the
hosted service offer and how they word it is `src/surface.rs`, with its own
tests; the browser MCP E2E test runs it in a real tab.

## Budget and search

`tool-budget.json` records what each tool costs in bytes; `tool-search.json` holds the
requests a tool search must answer and the pairs of tools allowed to read alike. Both
are read by `src/surface_rules.rs`, and [docs/MCP-TOOLS.md](../../../../docs/MCP-TOOLS.md)
says how to change them.
