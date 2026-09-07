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
RUN cargo build --locked --release \
    --bin mission_control \
    --bin sensor_collector \
    --bin imagery_processor \
    --bin lidar_mapper \
    --bin ground_station_ui \
    --bin geo_hub

# Edge runtime stage: the on-vehicle / ground-station binaries.
FROM debian:12-slim AS runtime-edge

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

# ---------------------------------------------------------------------------
# Appliance runtime stage: the geo_hub server (farmer portal + web
# workspace/browse + APIs + satellite ingestion). This is the primary
# release artifact deployed to the headless Linux box. It runs the single
# `geo_hub` binary as a non-root user with all state under /opt/agbot,
# addressable through GEO_HUB__* environment overrides so nothing depends on
# the current working directory.
# ---------------------------------------------------------------------------
FROM debian:12-slim AS runtime-appliance

ARG AGRODRONE_COMMIT=unknown
ARG AGRODRONE_IMAGE_DIGEST=unbuilt

LABEL org.opencontainers.image.title="AGBot geo_hub appliance" \
      org.opencontainers.image.description="Farmer portal, web workspace/browse, APIs, and satellite ingestion pipeline" \
      org.opencontainers.image.revision="${AGRODRONE_COMMIT}"

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Non-root service account
RUN useradd -r -s /bin/false -d /opt/agbot agbot

# Appliance layout: bin (binary), db (sqlite), data (scenes/products),
# web (static portal + workspace assets served via ServeDir).
RUN mkdir -p /opt/agbot/bin /opt/agbot/db /opt/agbot/data /opt/agbot/web

# Server binary and the on-disk web assets (/workspace + /portal are served
# from disk; /browse and the mobile client are compiled in).
COPY --from=builder /app/target/release/geo_hub /opt/agbot/bin/
COPY --chown=agbot:agbot geo_hub/web /opt/agbot/web

RUN chown -R agbot:agbot /opt/agbot

RUN printf '%s\n' \
    '{' \
    '  "schema_version": 1,' \
    '  "component": "geo_hub-appliance",' \
    '  "toolchain": "rust 1.93.1",' \
    '  "builder_base": "rust:1.93.1-slim-bookworm",' \
    '  "runtime_base": "debian:12-slim",' \
    "  \"source_commit\": \"${AGRODRONE_COMMIT}\"," \
    "  \"image_digest\": \"${AGRODRONE_IMAGE_DIGEST}\"," \
    '  "binaries": ["geo_hub"]' \
    '}' \
    > /opt/agbot/build-manifest.json

# GEO_HUB__* overrides pin every CWD-relative default to an absolute
# appliance path so the container is self-contained and relocatable.
ENV PATH="/opt/agbot/bin:$PATH" \
    GEO_HUB__BIND_ADDRESS="0.0.0.0:8080" \
    GEO_HUB__DATABASE_URL="sqlite:///opt/agbot/db/geo_hub.db" \
    GEO_HUB__DATA_ROOT="/opt/agbot/data" \
    GEO_HUB__WORKSPACE_WEB_ROOT="/opt/agbot/web"

USER agbot
WORKDIR /opt/agbot

# db, data are the stateful volumes to persist across upgrades.
VOLUME ["/opt/agbot/db", "/opt/agbot/data"]

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/health || exit 1

CMD ["geo_hub"]
