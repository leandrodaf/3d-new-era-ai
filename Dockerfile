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
#
# cargo-chef splits the build in two layers: the dependencies, rebuilt only
# when Cargo.lock or a manifest changes, and our crates on top. The deploy
# workflow keeps those layers in the registry, so a release compiles only
# what changed in this repository.
FROM rust:1.95-slim-bookworm AS chef
RUN cargo install cargo-chef --locked --version 0.1.78
WORKDIR /src

FROM chef AS plan
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=plan /src/recipe.json recipe.json
RUN cargo chef cook --release -p newera-cloud --locked --recipe-path recipe.json
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
