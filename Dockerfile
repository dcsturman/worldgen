FROM rust:latest AS base
RUN rustup update
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash && \
    cargo binstall -y trunk
RUN rustup target add wasm32-unknown-unknown

FROM base AS build-wasm

# Build WASM frontend
RUN mkdir /web
WORKDIR /web

COPY Cargo.toml Cargo.lock index.html Trunk.toml /web/
COPY public ./public/
COPY style.css ./

ENV RUSTFLAGS='--cfg getrandom_backend="wasm_js"'

# Copy the source code. assets/ holds DejaVuSans.ttf which
# src/worldmap/render/png.rs bakes in via include_bytes! at compile time;
# the macro resolves paths relative to the source file, so the font has to
# exist in the build context for the WASM crate to compile.
COPY src ./src/
COPY assets ./assets/

# Release build is required: Cloud Run caps responses at 32 MiB per request,
# and a debug-mode wasm easily exceeds that with debug symbols (~36 MB →
# request truncated → browser sees an empty BufferSource and refuses to
# instantiate). Release builds with LTO drop to ~5–8 MB.
#
# Cache mounts persist cargo's downloaded crate index, source registry,
# and the per-stage target/ directory in the BuildKit daemon across
# rebuilds. With these in place, a source-only change skips dep
# recompilation entirely (cargo's incremental cache lives inside
# target/). The mounts don't export with --cache-to=type=registry, so
# the *first* build on a fresh CI runner is no faster — the win is for
# subsequent local builds.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/web/target \
    trunk build --release

FROM base AS build-server

ARG TARGETARCH

# Determine target architecture once and save it for reuse
RUN if [ "$TARGETARCH" = "arm64" ]; then \
    echo "aarch64-unknown-linux-musl" > /tmp/musl_target; \
    else \
    echo "x86_64-unknown-linux-musl" > /tmp/musl_target; \
    fi && \
    echo "Building for target: $(cat /tmp/musl_target)"

# Build native WebSocket server for musl (Alpine compatibility)
RUN rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl && \
    apt-get update && apt-get install -y musl-tools musl-dev

RUN mkdir /server
WORKDIR /server

COPY Cargo.toml Cargo.lock ./

# Copy source code. assets/ is included for the same reason as the WASM
# stage: the lib's PNG renderer bakes in the bundled DejaVu Sans via
# include_bytes! at compile time.
COPY src ./src/
COPY assets ./assets/

# Build the server binary. Same cache-mount story as the wasm stage —
# /server/target holds cargo's incremental cache; the cargo registry +
# git mounts skip re-downloading the dep tree on every build. The final
# binary is copied OUT of the cache mount (to /server/server) before
# the layer commits — anything still inside /server/target after the
# RUN finishes is mount-only and not visible to later stages.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/server/target \
    MUSL_TARGET=$(cat /tmp/musl_target) && \
    cargo build --release --bin server --features backend --target "$MUSL_TARGET" && \
    cp "/server/target/$MUSL_TARGET/release/server" /server/server

FROM nginx:1.27-alpine

# Install supervisor to run multiple processes
RUN apk add --no-cache supervisor

EXPOSE 80

# Copy nginx config
COPY nginx.conf /etc/nginx/nginx.conf

# Copy WASM frontend
COPY --from=build-wasm /web/dist/ /usr/share/nginx/html/

# Copy server binary
COPY --from=build-server /server/server /usr/local/bin/server

# Copy supervisor config
COPY supervisord.conf /etc/supervisord.conf

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
