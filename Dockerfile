# Multi-stage build for Synapse CLI
FROM rust:1.88-bookworm AS builder

WORKDIR /build

# Copy workspace
COPY . .

# Build release binary with full features
RUN cargo build --release --features full

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/synapse /usr/local/bin/synapse

# Default config location
RUN mkdir -p /etc/synapse
VOLUME ["/etc/synapse"]

EXPOSE 3000

ENTRYPOINT ["synapse"]
CMD ["serve"]
