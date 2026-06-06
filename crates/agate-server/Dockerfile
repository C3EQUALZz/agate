# syntax=docker/dockerfile:1
#
# Multi-stage build for the agate-server binary (the Docker entrypoint).
# cargo-chef caches the dependency compile as its own layer, so source-only
# changes rebuild in seconds. The runtime is a slim, non-root image with a
# health probe on /healthz.

# ---- chef: pin cargo-chef on the build toolchain (MSRV) ----
FROM rust:1.94-bookworm AS chef
WORKDIR /app
RUN cargo install cargo-chef --locked

# ---- planner: capture the dependency graph ----
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- builder: cook deps (cached layer), then build the server ----
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin agate-server

# ---- runtime: slim, non-root, with a health probe ----
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --user-group --no-create-home agate
COPY --from=builder /app/target/release/agate-server /usr/local/bin/agate-server
USER agate
EXPOSE 8080
# Probes the proxy's /healthz on the default bind port. Adjust if you change
# [proxy].bind to a non-8080 port.
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -fsS http://localhost:8080/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/agate-server"]
