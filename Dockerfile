# The hosted service behind mcp.3dneweraai.com: the relay that lets an AI reach
# the editor in someone's browser tab, plus accounts, OAuth for AI clients and
# the one fixed MCP address (newera-cloud). The one thing here that is
# deployed. It sits at the root because that is where the deploy workflow looks.
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
# The database is Postgres beside it (DATABASE_URL); projects stay in the
# browser tab. Runs as nobody in particular.
USER nonroot:nonroot
ENV PORT=7979
EXPOSE 7979
HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD ["/usr/local/bin/newera-cloud", "--health"]
ENTRYPOINT ["/usr/local/bin/newera-cloud"]
