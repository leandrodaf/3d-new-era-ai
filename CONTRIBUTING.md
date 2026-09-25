# Contributing

Thanks for helping build 3D New Era AI!

## Setup

```sh
make setup   # rustfmt, clippy, cargo-watch, cargo-deny
make run     # editor + HTTP + MCP with a sample house
make dev     # same, rebuilding on every change
```

Run `make` to list every command. [docs/README.md](docs/README.md) maps the
documentation and [scripts/README.md](scripts/README.md) the helper scripts.

Builds report to Sentry only with `NEWERA_SENTRY_DSN` set at build time. Released
binaries get it from the repository secret; to test reporting locally, put
`NEWERA_SENTRY_DSN=…` in a `.env.local` at the root — git ignores it and the
Makefile reads it — and run `newera telemetry test`.

## Before opening a PR

```sh
make check   # fmt, clippy (pedantic, -D warnings), tests, MCP smoke test
make deny    # license and advisory audit
```

CI runs the same checks on Linux, macOS and Windows.

## Guidelines

- **Every change to a home goes through a `Command`.** Don't mutate `Home` from UI
  or MCP code. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
- **A feature ships with its MCP tool.** Keep tool payloads compact: short ids,
  `[x, y]` points, omitted defaults, one-line write replies. A tool goes in the
  domain module it belongs to under `crates/newera-mcp/src/tools/` (the table in
  `tools/mod.rs` says which), and its router joins `parts` in `NewEraMcp::new`.
- **Units are centimeters.** Name fields after what they measure, and document units.
- **Tests**: core logic gets unit tests; MCP tools get tests for their wire format;
  end-to-end behavior goes in `scripts/mcp-smoke.sh` when it crosses processes.
- **No code from Sweet Home 3D** (or other GPL projects): it's a functional reference
  only. Every asset needs a known, redistributable license.
- Commit messages: [Conventional Commits](https://www.conventionalcommits.org)
  (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `ci:`, `chore:`).

## Community

Be kind and constructive: this project follows its [code of conduct](CODE_OF_CONDUCT.md).
Report security issues privately, as described in [SECURITY.md](SECURITY.md), not in a
public issue.

## License

By contributing, you agree that your contributions are dual-licensed under
MIT OR Apache-2.0, without additional terms.
