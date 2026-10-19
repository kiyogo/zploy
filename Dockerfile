# Multi-stage Dockerfile for Zploy
FROM rust:1.80-alpine AS builder

WORKDIR /app
RUN apk add --no-cache musl-dev mysql-client

# Cache dependencies
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release || true

# Copy real source code
COPY . .
RUN cargo build --release

# Minimal runtime container
FROM alpine:3.20

WORKDIR /app
RUN apk add --no-cache ca-certificates tzdata

COPY --from=builder /app/target/release/zploy /app/zploy
COPY --from=builder /app/target/release/reset_admin /app/reset_admin
COPY public/ /app/public/
COPY migrations/ /app/migrations/
COPY config/config.example.toml /app/config/config.toml

RUN mkdir -p /app/storage/sites /app/storage/staging /app/storage/temp

EXPOSE 3000

ENV ZPLOY_PORT=3000
ENV ZPLOY_HOST=0.0.0.0

CMD ["/app/zploy"]
