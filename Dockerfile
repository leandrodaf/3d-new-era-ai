# The service behind mcp.3dneweraai.com (newera-cloud): the relay that lets an
# AI reach the editor in someone's browser tab, plus accounts, OAuth for AI
# clients, the one fixed MCP address and the editor with no window. The one
# thing here that is deployed; it sits at the root because that is where the
# deploy workflow looks. Postgres is beside it on the VPS (DATABASE_URL).
#
# Two stages: one that has a Rust toolchain, one that has nothing. What ships
# is a single binary on a distroless base — no shell, no package manager,
# nothing to pivot to if the process is ever taken.
# Bookworm, like the runtime below: a newer build base links against a newer
# glibc than distroless debian12 carries, and the binary would not start.
FROM rust:1.95-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p newera-cloud --locked

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/newera-cloud /usr/local/bin/newera-cloud
# Accounts and cloud projects live in the Postgres beside it; what it writes
# to disk is a cache it can lose. Runs as nobody in particular.
USER nonroot:nonroot
ENV PORT=7979
EXPOSE 7979
HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD ["/usr/local/bin/newera-cloud", "--health"]
ENTRYPOINT ["/usr/local/bin/newera-cloud"]
