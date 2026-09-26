# Contributing

Thanks for helping build 3D New Era AI!

## Setup

```sh
make setup   # rustfmt, clippy, cargo-watch, cargo-deny
make run     # editor + HTTP + MCP with a sample house
make dev     # same, rebuilding on every change
```

### The short loop

A change in `newera-core` rebuilds every crate above it and relinks the editor —
about 40 seconds (15 for a change in the UI alone), and `--release` minutes,
because of the single codegen unit and the LTO it links with. For anything you
have to *look* at, the picture is the answer and the window is not: `make shot`
stops at `newera-render` and writes a PNG (about 15 seconds from a change in the
core), and `make watch` redraws it on every save.

```sh
make shot                                    # aerial view of the sample house
make shot SHOT_VIEW='cam=2'                  # a stored point of view
make shot SHOT_VIEW='top' SHOT_OUT=/tmp/p.png
make shot SHOT_FILE=my-plan.newera SHOT_VIEW='aerial yaw=45 pitch=35 walls=down'
```

When the editor has to be fast to *use* while you work on it — a photo render,
a big plan, a smooth 3D view — `make run-quick` is the same optimized code
without LTO. Keep `--release` for what you ship; it is not the build to iterate
on.

`FILE=` opens the plan you are chasing a bug in, and `make dev` reopens it after
every rebuild:

```sh
make dev FILE=plans/kitchen.newera
make shot FILE=plans/kitchen.newera SHOT_VIEW='cam=0'
```

Run `make` to list every command. [docs/README.md](docs/README.md) maps the
documentation and [scripts/README.md](scripts/README.md) the helper scripts.

Builds report to Sentry only with `NEWERA_SENTRY_DSN` set at build time. Released
binaries get it from the repository secret; to test reporting locally, put
`NEWERA_SENTRY_DSN=…` in a `.env.local` at the root — git ignores it and the
Makefile reads it — and run `newera telemetry test`.

## Before opening a PR

```sh
make check       # fmt, clippy (pedantic, -D warnings), tests, MCP smoke test
make deny        # license and advisory audit
make docs-lint   # Markdown, or what checks it: rules, spelling, links
```

CI runs the same checks on Linux, macOS and Windows.

### How a change lands

`main` takes no direct pushes: a pull request with green checks is the way in, and it
arrives as a single squashed commit. Force pushes to `main` and deleting it are refused,
and a review thread has to be resolved before the merge button turns green. No approval
from anyone else is required.

A change that touches only prose skips the seven Rust jobs and runs three quick ones
instead — Markdown, spelling and links. Nothing is needed from you to get that: a job
that has nothing to do reports itself as skipped. `scripts/changed-kind.sh` decides which
half a change belongs to, and you can ask it directly:

```sh
scripts/changed-kind.sh README.md   # code=false docs=true
```

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
