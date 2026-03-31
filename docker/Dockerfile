# MEV Protocol — Rust Core Engine
#
# Multi-stage build: compile in rust:1.75 → copy binary to slim Debian runtime.
# Final image is ~80 MB with only the binary, TLS certs, and OpenSSL.
#
# Build:  docker build -f docker/Dockerfile -t mev-engine .
# Run:    docker run --env-file .env mev-engine

# ── Stage 1: Build ───────────────────────────────────────────────────
FROM rust:1.75-slim-bookworm as builder

WORKDIR /build

# Install build dependencies (OpenSSL for TLS, build-essential for cc crate)
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY core/ ./

# Release build with LTO and single codegen unit (matches Cargo.toml profile)
RUN cargo build --release

# ── Stage 2: Runtime ─────────────────────────────────────────────────
FROM debian:bookworm-slim

WORKDIR /app

# Minimal runtime dependencies: TLS certificates + OpenSSL shared lib
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/mev-engine /app/

# Run as non-root user (UID 1000) to limit blast radius if compromised
RUN useradd -m -u 1000 mev
USER mev

# Liveness check: verify the process is running
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
    CMD pgrep mev-engine || exit 1

ENTRYPOINT ["/app/mev-engine"]
