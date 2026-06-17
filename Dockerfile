# --- Builder Stage ---
FROM rust:1.85-slim-bookworm AS builder
WORKDIR /usr/src/microscope-mem
COPY . .
RUN cargo build --release

# --- Runner Stage ---
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/microscope-mem/target/release/microscope-mem /usr/local/bin/microscope-mem
RUN mkdir -p layers output

EXPOSE 6060 8080
ENTRYPOINT ["microscope-mem"]
CMD ["--help"]
