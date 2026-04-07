# --- Builder Stage ---
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /usr/src/microscope-mem
COPY . .

# Build the release binary
RUN cargo build --release

# --- Runner Stage ---
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies (OpenSSL might be needed if features expand)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /usr/src/microscope-mem/target/release/microscope-mem /usr/local/bin/microscope-mem

# Create default directories
RUN mkdir -p layers output

# Expose Binary Spine API port
EXPOSE 6060

# Default command: show help
ENTRYPOINT ["microscope-mem"]
CMD ["--help"]
