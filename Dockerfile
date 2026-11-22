# syntax=docker/dockerfile:1

FROM rust:1-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY web ./web
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked \
    && cp target/release/merkle-builder /usr/local/bin/merkle-builder

FROM alpine:3
RUN adduser -D -u 10001 app
COPY --from=builder /usr/local/bin/merkle-builder /usr/local/bin/merkle-builder
USER app
WORKDIR /data
ENV HOST=0.0.0.0 \
    PORT=8080
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
    CMD wget -qO- "http://127.0.0.1:${PORT}/healthz" || exit 1
ENTRYPOINT ["merkle-builder"]
CMD ["serve"]
