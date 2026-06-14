# syntax=docker/dockerfile:1.7
# Multi-stage Docker build for AgroDrone
FROM rust:1.93.1-slim-bookworm AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libudev-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy source and build the runtime binaries.
COPY . .
RUN cargo build --release \
    --bin mission_control \
    --bin sensor_collector \
    --bin imagery_processor \
    --bin lidar_mapper \
    --bin ground_station_ui

# Runtime stage
FROM debian:12-slim AS runtime

ARG AGRODRONE_COMMIT=unknown
ARG AGRODRONE_IMAGE_DIGEST=unbuilt

LABEL org.opencontainers.image.title="AGBot AgroDrone runtime" \
      org.opencontainers.image.revision="${AGRODRONE_COMMIT}"

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false agrodrone

# Create directories
RUN mkdir -p /opt/agrodrone/data /opt/agrodrone/missions /opt/agrodrone/bin
RUN chown -R agrodrone:agrodrone /opt/agrodrone

# Copy binaries
COPY --from=builder /app/target/release/mission_control /opt/agrodrone/bin/
COPY --from=builder /app/target/release/sensor_collector /opt/agrodrone/bin/
COPY --from=builder /app/target/release/imagery_processor /opt/agrodrone/bin/
COPY --from=builder /app/target/release/lidar_mapper /opt/agrodrone/bin/
COPY --from=builder /app/target/release/ground_station_ui /opt/agrodrone/bin/

RUN printf '%s\n' \
    '{' \
    '  "schema_version": 1,' \
    '  "toolchain": "rust 1.93.1",' \
    '  "builder_base": "rust:1.93.1-slim-bookworm",' \
    '  "runtime_base": "debian:12-slim",' \
    "  \"source_commit\": \"${AGRODRONE_COMMIT}\"," \
    "  \"image_digest\": \"${AGRODRONE_IMAGE_DIGEST}\"," \
    '  "binaries": ["mission_control", "sensor_collector", "imagery_processor", "lidar_mapper", "ground_station_ui"]' \
    '}' \
    > /opt/agrodrone/build-manifest.json

# Set environment
ENV PATH="/opt/agrodrone/bin:$PATH"
ENV DATA_ROOT_PATH="/opt/agrodrone/data"
ENV MISSION_DATA_PATH="/opt/agrodrone/missions"

# Switch to app user
USER agrodrone

# Set working directory
WORKDIR /opt/agrodrone

# Default command (can be overridden)
CMD ["mission_control"]

# Expose ports
EXPOSE 3000 8080 8081
