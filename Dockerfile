# Stage 1: Build the codejail binary
FROM rust:bookworm AS builder

WORKDIR /build

# Install the WASM compilation target (needed for `codejail build`)
RUN rustup target add wasm32-wasip1

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy src to pre-build dependencies
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf src target/release/codejail target/release/deps/codejail-*

# Copy actual source and build
COPY src/ src/
RUN cargo build --release && strip target/release/codejail

# Stage 2: Minimal runtime image
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends bubblewrap ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/codejail /usr/local/bin/codejail

# Copy example policy
COPY examples/policy.toml /etc/codejail/policy.toml

# Default data directory for images/containers
RUN mkdir -p /data
WORKDIR /data

ENTRYPOINT ["codejail"]
