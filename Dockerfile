# Multi-stage Docker build for gerrit-mcp

# Stage 1: Build
FROM rust:1.97.1-alpine3.24 AS builder
RUN apk add --no-cache musl-dev pkgconf
WORKDIR /build

ARG GIT_HASH
ARG BUILD_DATE
ENV GIT_HASH=${GIT_HASH}
ENV BUILD_DATE=${BUILD_DATE}

COPY Cargo.toml Cargo.lock ./
COPY crates/gerrit-core/Cargo.toml crates/gerrit-core/
COPY crates/gerrit-mcp/Cargo.toml crates/gerrit-mcp/
COPY crates/gerrit-mcp/build.rs crates/gerrit-mcp/
RUN mkdir -p crates/gerrit-core/src crates/gerrit-mcp/src && \
    echo 'fn main() {}' > crates/gerrit-mcp/src/main.rs && \
    echo '' > crates/gerrit-mcp/src/lib.rs && \
    echo '' > crates/gerrit-core/src/lib.rs && \
    cargo build --release && \
    rm -rf target/release/.fingerprint/gerrit-*
COPY crates/gerrit-core/src crates/gerrit-core/src
COPY crates/gerrit-mcp/src crates/gerrit-mcp/src
RUN cargo build --release

# Stage 2: Runtime
FROM alpine:3.24

ARG HEALTHCHECK_PORT=8080
ARG HEALTHCHECK_PATH=/healthz
ARG CONFIG_DIR=/config

RUN apk add --no-cache ca-certificates
COPY --from=builder /build/target/release/gerrit-mcp /usr/local/bin/gerrit-mcp
RUN addgroup -S appgroup && adduser -S appuser -G appgroup
USER appuser

VOLUME ["${CONFIG_DIR}"]

HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
    CMD wget -qO- http://localhost:${HEALTHCHECK_PORT}${HEALTHCHECK_PATH} || exit 1

LABEL org.opencontainers.image.title="gerrit-mcp"
LABEL org.opencontainers.image.description="MCP server for Gerrit code review (AOSP-scale codebases)"
LABEL org.opencontainers.image.vendor="RD2W"
LABEL org.opencontainers.image.authors="Maxim Krutovercev (mkrutovercev@yandex.ru)"
LABEL org.opencontainers.image.documentation="https://github.com/RD2W/gerrit-mcp-server"
LABEL org.opencontainers.image.url="https://github.com/RD2W/gerrit-mcp-server"
LABEL org.opencontainers.image.source="https://github.com/RD2W/gerrit-mcp-server"
LABEL org.opencontainers.image.licenses="GPL-3.0-or-later"

ENTRYPOINT ["gerrit-mcp"]
