FROM rust:latest AS base
RUN rustup update
RUN cargo install trunk
RUN rustup target add wasm32-unknown-unknown

FROM base AS build-wasm

# Build WASM frontend
RUN mkdir /web
WORKDIR /web

COPY Cargo.toml Cargo.lock index.html Trunk.toml /web/
COPY src ./src/
COPY public ./public/
COPY style.css ./

ENV RUSTFLAGS='--cfg getrandom_backend="wasm_js"'

RUN trunk build --release

FROM base AS build-server

# Build native WebSocket server for musl (Alpine compatibility)
RUN rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
RUN apt-get update && apt-get install -y musl-tools musl-dev

RUN mkdir /server
WORKDIR /server

COPY Cargo.toml Cargo.lock /server/
COPY src ./src/

# Detect architecture and build for appropriate musl target
ARG TARGETARCH
RUN if [ "$TARGETARCH" = "arm64" ]; then \
      MUSL_TARGET="aarch64-unknown-linux-musl"; \
    else \
      MUSL_TARGET="x86_64-unknown-linux-musl"; \
    fi && \
    cargo build --release --bin server --features backend --target $MUSL_TARGET && \
    cp /server/target/$MUSL_TARGET/release/server /server/target/release/server

FROM nginx:1.27-alpine

# Install supervisor to run multiple processes
RUN apk add --no-cache supervisor

EXPOSE 80

# Copy nginx config
COPY nginx.conf /etc/nginx/nginx.conf

# Copy WASM frontend
COPY --from=build-wasm /web/dist/ /usr/share/nginx/html/

# Copy server binary
COPY --from=build-server /server/target/release/server /usr/local/bin/server

# Copy supervisor config
COPY supervisord.conf /etc/supervisord.conf

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
