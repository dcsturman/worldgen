FROM rust:latest AS base
RUN rustup update
RUN cargo install trunk
RUN rustup target add wasm32-unknown-unknown

FROM base AS build 

# Build dependencies
RUN mkdir /web
WORKDIR /web

ADD Cargo.toml Cargo.lock index.html Trunk.toml /web/

ENV RUSTFLAGS='--cfg getrandom_backend="wasm_js"'

RUN mkdir ./src && mkdir ./src/bin && echo 'fn main() {}' > ./src/bin/main.rs && touch ./src/lib.rs

RUN cargo build --release --target wasm32-unknown-unknown
RUN rm -rf ./src
COPY src /web/src/

FROM build AS release
RUN touch ./src/bin/main.rs
RUN touch ./src/lib.rs

COPY public /web/public
COPY style.css /web/style.css

ENV RUSTFLAGS='--cfg getrandom_backend="wasm_js"'

RUN trunk build --release

FROM nginx:1.21-alpine

EXPOSE 80
COPY nginx.conf /etc/nginx/nginx.conf
COPY --from=release /web/dist/ /usr/share/nginx/html/
