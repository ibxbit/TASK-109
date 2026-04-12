# =============================================================================
# VitalPath — Rust backend
# =============================================================================
# Multi-stage build:
#   builder  — compiles the release binary
#   runtime  — minimal Debian image with only the compiled binary
#
# Build:   docker build -t vitalpath .
# Run:     docker compose up
# =============================================================================

# ─── Build stage ─────────────────────────────────────────────────────────────
FROM rust:1.88-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        libpq-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependency compilation separately from application source.
# This layer is only invalidated when Cargo.toml or Cargo.lock change.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
 && cargo build --release \
 && rm -f target/release/vitalpath src/main.rs

# Build the actual application
COPY . .
# touch all rust files so their mtime is newer than the cache, forcing recompilation
RUN touch src/main.rs && cargo build --release

# ─── Runtime stage ───────────────────────────────────────────────────────────
# Use the same base as the builder stage to guarantee all runtime libraries
# (libgcc, libstdc++, glibc, etc.) are present and at the exact same versions.
# The OOM was caused by cargo compilation; running the pre-built binary here
# is safe and requires no compilation.
FROM rust:1.88-slim-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
        libpq5 \
        ca-certificates \
        curl \
    && rm -rf /var/lib/apt/lists/* \
 && useradd --system --no-create-home --shell /sbin/nologin vitalpath

WORKDIR /app

COPY --from=builder /app/target/release/vitalpath ./vitalpath
RUN chmod +x ./vitalpath && chown vitalpath:vitalpath ./vitalpath

# Exports directory — mount exports_data volume here at runtime
RUN mkdir -p /exports && chown vitalpath:vitalpath /exports
RUN chown -R vitalpath:vitalpath /app

USER vitalpath

VOLUME ["/exports"]

EXPOSE 8080

CMD ["./vitalpath"]
