FROM rust:1-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends jq iproute2 procps \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY data ./data
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo test --locked && cargo build --release --locked && cp target/release/sre-trainer /build/sre-trainer

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates coreutils findutils procps iproute2 dnsutils curl jq util-linux grep sed gawk \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/sre-trainer /usr/local/bin/sre-trainer
COPY data ./data

ENV SRE_DATA_DIR=/data \
    SRE_LESSONS=/app/data/lessons.json \
    OPENROUTER_MODEL=inception/mercury-2.5

RUN useradd --create-home --uid 10001 trainee && mkdir -p /data && chown trainee:trainee /data
USER trainee

ENTRYPOINT ["sre-trainer"]
