# Stage 1: Build the containment binary
FROM rust:bookworm AS builder

WORKDIR /build

# Install the WASM compilation target (needed for `containment build`)
RUN rustup target add wasm32-wasip1

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy src to pre-build dependencies
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/containment target/release/deps/containment-*

# Copy actual source and build
COPY src/ src/
RUN cargo build --release && strip target/release/containment

# Stage 2: Minimal runtime image
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends bubblewrap ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/containment /usr/local/bin/containment

# Default data directory for images/containers
RUN mkdir -p /data
WORKDIR /data

ENTRYPOINT ["containment"]
