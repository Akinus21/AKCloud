# AKCloud - Build stage
FROM rust:latest AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy and build dependencies first (for dependency caching)
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf target/release/deps/aktags_cloud*

# Copy source
COPY src ./src

# Build the application
RUN cargo build --release

# AKCloud - Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1000 -s /bin/bash appuser

# Copy the binary
COPY --from=builder /app/target/release/aktags-cloud /usr/local/bin/

# Create directories
RUN mkdir -p /data/storage /config /logs /graveyard

# Set ownership
RUN chown -R appuser:appuser /data /config /logs /graveyard

# Switch to non-root user
USER appuser

# Environment variables
ENV RUST_LOG=info
ENV CONFIG_PATH=/config/config.toml

# Expose ports
EXPOSE 8080 22000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

# Start the server
CMD ["aktags-cloud", "--config", "/config/config.toml"]