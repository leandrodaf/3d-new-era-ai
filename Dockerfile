# The relay that lets an AI reach the editor in someone's browser tab — the one
# service in this repository, and the only thing here that is deployed.
# It sits at the root because that is where the deploy workflow looks.
#
# Two stages: one that has a Rust toolchain, one that has nothing. What ships
# is a single static-ish binary on a distroless base — no shell, no package
# manager, nothing to pivot to if the process is ever taken.
FROM rust:1.95-slim AS build
WORKDIR /src
# The workspace manifests first, so a change in the code does not throw away
# the dependency build.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p newera-relay --locked

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/newera-relay /usr/local/bin/newera-relay
# It holds rooms in memory and writes nothing: no volume, no database, no user
# data to lose. Runs as nobody in particular.
USER nonroot:nonroot
ENV PORT=7979
EXPOSE 7979
ENTRYPOINT ["/usr/local/bin/newera-relay"]
