# MEV Protocol - Rust Core Image
FROM rust:1.75-slim-bookworm as builder

WORKDIR /build

# Install dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source
COPY core/ ./

# Build release
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary
COPY --from=builder /build/target/release/mev-engine /app/

# Create non-root user
RUN useradd -m -u 1000 mev
USER mev

# Health check
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
    CMD pgrep mev-engine || exit 1

ENTRYPOINT ["/app/mev-engine"]
