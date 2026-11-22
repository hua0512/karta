# syntax=docker/dockerfile:1

FROM lukemathwalker/cargo-chef:latest-rust-1-slim-bookworm AS chef
WORKDIR /src

# ── Stage 1: Dependency recipe ────────────────────────────────────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 2: Build ────────────────────────────────────────────────────────────
FROM chef AS builder

# BoringSSL (wreq) build dependencies — git included because the btls-sys
# build script runs `git init` in its vendored BoringSSL checkout
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        cmake perl clang libclang-dev build-essential pkg-config git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --locked --release --recipe-path recipe.json -p karta --features ytdlp

COPY . .
RUN cargo build --locked --release -p karta --features ytdlp

# ── Stage 3: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates ffmpeg \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /src/target/release/karta /app/karta
COPY --from=builder /src/target/release/karta-cli /app/karta-cli
COPY resources/ /app/resources/
# For reference only: the plugin reads/writes its real config at
# /app/data/karta/config.json (created with defaults on first run).
COPY config.example.json /app/config.example.json

RUN chmod +x /app/karta /app/karta-cli

# The bot writes config.json, state, and cache under its working directory.
# Keep UID/GID 1000 so mounted data retains the same ownership.
RUN groupadd --gid 1000 karta \
    && useradd --uid 1000 --gid 1000 --create-home karta \
    && chown -R karta:karta /app
USER karta

ENTRYPOINT ["/app/karta"]
