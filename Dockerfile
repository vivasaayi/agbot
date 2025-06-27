# Multi-stage Docker build for AgroDrone
FROM rust:1.75-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libudev-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml ./shared/
COPY mission_control/Cargo.toml ./mission_control/
COPY sensor_collector/Cargo.toml ./sensor_collector/
COPY ndvi_processor/Cargo.toml ./ndvi_processor/
COPY lidar_mapper/Cargo.toml ./lidar_mapper/
COPY ground_station_ui/Cargo.toml ./ground_station_ui/

# Create dummy source files to cache dependencies
RUN mkdir -p shared/src mission_control/src sensor_collector/src ndvi_processor/src lidar_mapper/src ground_station_ui/src
RUN echo "fn main() {}" > mission_control/src/main.rs
RUN echo "fn main() {}" > sensor_collector/src/main.rs
RUN echo "fn main() {}" > ndvi_processor/src/main.rs
RUN echo "fn main() {}" > lidar_mapper/src/main.rs
RUN echo "fn main() {}" > ground_station_ui/src/main.rs
RUN echo "pub fn dummy() {}" > shared/src/lib.rs

# Build dependencies
RUN cargo build --release
RUN rm -rf shared/src mission_control/src sensor_collector/src ndvi_processor/src lidar_mapper/src ground_station_ui/src

# Copy actual source code
COPY shared/src ./shared/src/
COPY mission_control/src ./mission_control/src/
COPY sensor_collector/src ./sensor_collector/src/
COPY ndvi_processor/src ./ndvi_processor/src/
COPY lidar_mapper/src ./lidar_mapper/src/
COPY ground_station_ui/src ./ground_station_ui/src/

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

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
COPY --from=builder /app/target/release/ndvi_processor /opt/agrodrone/bin/
COPY --from=builder /app/target/release/lidar_mapper /opt/agrodrone/bin/
COPY --from=builder /app/target/release/ground_station_ui /opt/agrodrone/bin/

# Copy environment file
COPY .env /opt/agrodrone/

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
