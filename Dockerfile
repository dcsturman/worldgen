FROM rust:latest AS base
RUN rustup update
RUN cargo install trunk
RUN rustup target add wasm32-unknown-unknown

FROM base AS build 

# Build dependencies
RUN mkdir /web
WORKDIR /web

COPY Cargo.toml Cargo.lock index.html Trunk.toml /web/
COPY src ./src/
COPY public ./public/
COPY style.css ./

ENV RUSTFLAGS='--cfg getrandom_backend="wasm_js"'

RUN trunk build --release

FROM nginx:1.21-alpine

EXPOSE 80
COPY nginx.conf /etc/nginx/nginx.conf
COPY --from=build /web/dist/ /usr/share/nginx/html/
