FROM rust:latest AS base
RUN rustup update
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash && \
    cargo binstall -y trunk cargo-chef
RUN rustup target add wasm32-unknown-unknown

FROM base AS planner
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN cargo install cargo-chef
RUN cargo chef prepare --recipe-path recipe.json

FROM base AS build-wasm

# Build WASM frontend
RUN mkdir /web
WORKDIR /web

COPY Cargo.toml Cargo.lock index.html Trunk.toml /web/
COPY public ./public/
COPY style.css ./

ENV RUSTFLAGS='--cfg getrandom_backend="wasm_js"'

# Copy the source code
COPY src ./src/

# Build with debug symbols for better error messages (not minified)
RUN trunk build

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

# Copy the recipe from planner
COPY --from=planner /app/recipe.json recipe.json

# Cook dependencies with cache mount - speeds up dependency compilation
RUN --mount=type=cache,target=/server/target \
    MUSL_TARGET=$(cat /tmp/musl_target) && \
    cargo chef cook --release --recipe-path recipe.json --target "$MUSL_TARGET"

# Now copy the actual source code
COPY src ./src/

# Build the server binary - uses cached dependencies from cook step
RUN MUSL_TARGET=$(cat /tmp/musl_target) && \
    cargo build --release --bin server --features backend --target "$MUSL_TARGET" && \
    cp "/server/target/$MUSL_TARGET/release/server" /server/target/release/server

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
