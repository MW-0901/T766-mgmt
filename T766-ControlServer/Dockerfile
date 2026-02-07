# syntax=docker/dockerfile:1

FROM rust:slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    clang \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add aarch64-unknown-linux-gnu
RUN cargo install dioxus-cli

WORKDIR /app

# Copy project
COPY Cargo.toml ./
COPY src ./src
COPY assets ./assets
COPY Dioxus.toml ./

RUN dx bundle --release --platform server

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/dx ./dx

EXPOSE 8080

CMD ["./dx/T766-ControlServer"]
