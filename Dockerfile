# Multi-stage Dockerfile for Worldgen with Axum server and Firestore
# This builds both the WASM frontend and the Rust backend server
# Optimized for layer caching to avoid rebuilding trunk and dependencies

# Stage 1: Base image with tools (cached unless Rust version changes)
FROM rust:bookworm AS base
RUN --mount=type=cache,target=/usr/local/rustup \
    rustup update && \
    rustup target add wasm32-unknown-unknown

# Stage 2: Install trunk (cached separately, rarely changes)
FROM base AS cargo-tools-installer
# Install cargo-binstall
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

# Now use it to grab cargo-leptos (takes ~5 seconds instead of 5 minutes)
RUN cargo binstall --locked cargo-leptos -y

FROM cargo-tools-installer AS builder
WORKDIR /app

# Now copy the real source and build the app
COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo leptos build && \
    mkdir -p /app/dist && \
    cp -r /app/target/site /app/dist/site && \
    cp /app/target/debug/worldgen-server /app/dist/server

# Stage 5: Final runtime image (minimal)
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Install CA certificates for HTTPS and gRPC
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*
# Copy the server binary
COPY --from=builder /app/dist/server /app/server
COPY --from=builder /app/dist/site /app/site

# Cloud Run sets PORT environment variable
EXPOSE 8080

# Run the server
CMD ["/app/server"]
