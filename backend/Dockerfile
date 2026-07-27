# ── Build stage ───────────────────────────────────────────────────────────────
FROM rust:1.94.1-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY crates ./crates
COPY migrations ./migrations

# Build the API binary
RUN cargo build --release --bin hive-api

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/hive-api .
COPY migrations ./migrations

RUN mkdir -p /app/projects

EXPOSE 3000
CMD ["./hive-api"]
