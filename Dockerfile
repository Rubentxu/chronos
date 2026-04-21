# Stage 1: Builder
FROM rust:1.77-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
RUN cargo build -p chronos-mcp --release

# Stage 2: Runtime
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/chronos-mcp /usr/local/bin/chronos-mcp
ENV CHRONOS_DB_PATH=/data/chronos/sessions.redb
VOLUME ["/data/chronos"]
EXPOSE 3000
ENTRYPOINT ["chronos-mcp"]
CMD ["--stdio"]
