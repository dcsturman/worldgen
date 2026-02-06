# Multi-stage Dockerfile for Worldgen with Axum server and Firestore
# This builds both the WASM frontend and the Rust backend server
# Optimized for layer caching to avoid rebuilding trunk and dependencies

# Stage 1: Base image with tools (cached unless Rust version changes)
FROM rust:latest AS base
RUN rustup update && \
    rustup target add wasm32-unknown-unknown

# Stage 2: Install trunk (cached separately, rarely changes)
FROM base AS trunk-installer
RUN cargo install trunk --locked

# Stage 3: Build Rust dependencies (cached unless Cargo.toml changes)
FROM base AS dependencies
WORKDIR /web

# Copy only dependency manifests and cargo config
COPY Cargo.toml Cargo.lock /web/
COPY .cargo ./.cargo/

# Create dummy source files to build dependencies
RUN mkdir -p src/bin src/components src/server src/systems src/trade && \
    echo "fn main() {}" > src/bin/main.rs && \
    echo "fn main() {}" > src/bin/server.rs && \
    echo "fn main() {}" > src/bin/trade.rs && \
    echo "fn main() {}" > src/bin/world.rs && \
    echo "pub mod components; pub mod logging; pub mod server; pub mod systems; pub mod trade; pub mod util;" > src/lib.rs && \
    echo "pub fn init_from_url() {}" > src/logging.rs && \
    echo "pub fn parse_uwp(_: &str) -> Result<(), String> { Ok(()) }" > src/util.rs && \
    mkdir -p src/components && echo "" > src/components/mod.rs && \
    mkdir -p src/server && echo "pub mod state;" > src/server/mod.rs && \
    echo "use serde::{Deserialize, Serialize}; #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)] pub struct TradeState { pub version: u32 }" > src/server/state.rs && \
    mkdir -p src/systems && echo "" > src/systems/mod.rs && \
    mkdir -p src/trade && echo "" > src/trade/mod.rs

# Build dependencies for both WASM and server
# This layer is cached unless Cargo.toml or .cargo/config.toml changes
# Set CFLAGS to disable reference-types for C dependencies (fixes wasm-bindgen clone_ref error)
ENV CFLAGS_wasm32_unknown_unknown="-mno-reference-types"
RUN cargo build --release --target wasm32-unknown-unknown --bin main && \
    cargo build --release --features ssr --bin server

# Stage 4: Build actual application
FROM base AS builder
WORKDIR /web

# Copy trunk from installer stage
COPY --from=trunk-installer /usr/local/cargo/bin/trunk /usr/local/cargo/bin/trunk

# Copy dependency build artifacts from previous stage
# We copy the cargo registry (downloaded crates) and build cache
COPY --from=dependencies /usr/local/cargo /usr/local/cargo
COPY --from=dependencies /web/target /web/target

# Copy dependency manifests (needed for cargo)
COPY Cargo.toml Cargo.lock /web/

# Copy cargo config for target-specific rustflags
COPY .cargo ./.cargo/

# Copy actual source code (this layer changes frequently)
COPY src ./src/
COPY public ./public/
COPY index.html Trunk.toml style.css ./

# Build the WASM frontend
# Trunk will rebuild the WASM binary with real source code
# The .cargo/config.toml will apply the correct rustflags for wasm32-unknown-unknown
# Set CFLAGS to disable reference-types for C dependencies (fixes wasm-bindgen clone_ref error)
ENV CFLAGS_wasm32_unknown_unknown="-mno-reference-types"
RUN trunk build --release

# Build the actual server binary
# Cargo will reuse cached dependencies from the dependencies stage
# The .cargo/config.toml ensures rustflags only apply to WASM target
RUN cargo build --release --features ssr --bin server

# Stage 5: Final runtime image (minimal)
FROM debian:bookworm-slim

# Install CA certificates for HTTPS and gRPC
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the server binary
COPY --from=builder /web/target/release/server /app/server

# Copy the built WASM frontend
COPY --from=builder /web/dist /app/dist

# Set environment variables
ENV STATIC_DIR=/app/dist
ENV RUST_LOG=info

# Cloud Run sets PORT environment variable
EXPOSE 8080

# Run the server
CMD ["/app/server"]

