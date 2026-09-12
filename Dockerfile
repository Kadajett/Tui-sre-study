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

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates coreutils findutils procps iproute2 dnsutils curl jq util-linux grep sed gawk \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/sre-trainer /usr/local/bin/sre-trainer
COPY data ./data

ENV SRE_DATA_DIR=/data \
    SRE_LESSONS=/app/data/lessons.json \
    OPENROUTER_MODEL=inception/mercury-2

RUN useradd --create-home --uid 10001 trainee && mkdir -p /data && chown trainee:trainee /data
USER trainee

ENTRYPOINT ["sre-trainer"]


FROM docker:29.1.3-cli AS docker-cli

FROM runtime AS lab-runner
USER root
COPY --from=docker-cli /usr/local/bin/docker /usr/local/bin/docker
ARG KUBECTL_VERSION=v1.31.14
RUN curl -fsSLo /tmp/kubectl "https://dl.k8s.io/release/${KUBECTL_VERSION}/bin/linux/amd64/kubectl" \
    && curl -fsSLo /tmp/kubectl.sha256 "https://dl.k8s.io/release/${KUBECTL_VERSION}/bin/linux/amd64/kubectl.sha256" \
    && cd /tmp && echo "$(cat kubectl.sha256)  kubectl" | sha256sum --check \
    && install -m 0755 kubectl /usr/local/bin/kubectl && rm kubectl kubectl.sha256
ENV KUBECONFIG=/credentials/config SRE_LAB_DATA=/data
CMD ["--lab-server"]

FROM runtime AS trainer
