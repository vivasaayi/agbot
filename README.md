# AgroDrone - Agricultural Drone Control System

A comprehensive Rust monorepo for autonomous agricultural drone operations, featuring real-time flight control, multi-sensor data collection, AI-powered image analysis, and web-based ground station monitoring.

## 🚁 System Architecture

### Core Services

- **`mission_control`** - Async MAVLink flight controller interface with WebSocket telemetry
- **`sensor_collector`** - Real-time LiDAR (RPLIDAR A3) and multispectral camera data acquisition
- **`ndvi_processor`** - NDVI calculation from Red/NIR bands with GeoTIFF and PNG output
- **`lidar_mapper`** - Point cloud processing, occupancy grids, and 3D mapping
- **`ground_station_ui`** - Web dashboard and CLI for telemetry, NDVI visualization, and LiDAR overlays
- **`shared`** - Common configuration, schemas, error handling, and logging utilities

### Technology Stack

- **Runtime**: Tokio async runtime with tracing observability
- **Communication**: MAVLink protocol, WebSockets, JSON/gRPC APIs
- **Image Processing**: NDVI calculation, multispectral analysis, GeoTIFF support
- **Mapping**: LiDAR point clouds, occupancy grids, heatmap generation
- **Hardware**: Cross-compiled for ARM (Jetson Nano/Xavier, Raspberry Pi)

## 🛠️ Prerequisites

### Development Environment

- **Rust** 1.70+ (install via [rustup.rs](https://rustup.rs/))
- **Linux/macOS** (Ubuntu 20.04+ recommended for production)
- **Git** and **build-essential** packages

### Production Hardware

- **Flight Controller**: Pixhawk/Cube Orange with MAVLink support
- **LiDAR Sensor**: RPLIDAR A3 (serial/USB connection)
- **Camera**: USB multispectral camera (or simulated for development)
- **Compute Platform**: Jetson Nano/Xavier, Raspberry Pi 4+, or x86_64 Linux
- **Storage**: 32GB+ SD card or SSD for data logging

## 🚀 Quick Start

### 1. Clone and Build

```bash
git clone <repository-url>
cd agrodrone
cargo build --release
```

### 2. Environment Setup

```bash
# Copy environment template
cp .env .env.local
# Edit configuration for your setup
vim .env.local
```

Key configuration variables:

```env
# Operating mode
RUNTIME_MODE=SIMULATION  # or FLIGHT for production

# Hardware interfaces (FLIGHT mode only)
MAVLINK_SERIAL_PORT=/dev/ttyUSB0
LIDAR_SERIAL_PORT=/dev/ttyUSB1

# Data storage
DATA_ROOT_PATH=/opt/agrodrone/data
MISSION_DATA_PATH=/opt/agrodrone/missions

# Network services
WS_BIND_ADDRESS=0.0.0.0:8080    # WebSocket telemetry
API_BIND_ADDRESS=0.0.0.0:3000   # REST API
WEB_BIND_ADDRESS=0.0.0.0:8081   # Web dashboard
```

### 3. Development Mode (Simulation)

Use the provided development script:

```bash
# Start all services in simulation mode
./dev-start.sh
```

Or manually start individual services:

```bash
# Terminal 1: Mission Control (MAVLink + WebSocket)
cargo run --bin mission_control

# Terminal 2: Sensor Collection (LiDAR + Camera)
cargo run --bin sensor_collector

# Terminal 3: Web Dashboard
cargo run --bin ground_station_ui -- --web

# Terminal 4: CLI Monitor
cargo run --bin ground_station_ui
```

**Access Points:**
- Web Dashboard: http://localhost:8081
- API Endpoint: http://localhost:3000
- WebSocket: ws://localhost:8080

### 4. Production Deployment

```bash
# Set flight mode
export RUNTIME_MODE=FLIGHT

# Configure hardware permissions
sudo usermod -a -G dialout $USER
sudo chmod 666 /dev/ttyUSB*

# Deploy with Docker
docker build -t agrodrone .
docker run -d --privileged --network host agrodrone

# Or run directly
cargo run --release --bin mission_control &
cargo run --release --bin sensor_collector &
cargo run --release --bin ground_station_ui -- --web &
```

## 📊 Data Processing Workflows

### NDVI Analysis Pipeline

Process captured multispectral imagery:

```bash
# Single mission processing
cargo run --bin ndvi_processor -- \
    --input-dir /opt/agrodrone/data/camera/mission_001 \
    --output-dir /opt/agrodrone/processed/ndvi/mission_001

# Batch processing with statistics
cargo run --bin ndvi_processor -- \
    --input-dir /opt/agrodrone/data/camera \
    --output-dir /opt/agrodrone/processed/ndvi \
    --generate-stats
```

**Output formats:**
- PNG images with NDVI visualization
- GeoTIFF files with geospatial metadata
- JSON statistics (mean, std, percentiles)

### LiDAR Mapping Pipeline

Generate 3D maps and occupancy grids:

```bash
# Point cloud processing
cargo run --bin lidar_mapper -- \
    --input-dir /opt/agrodrone/data/lidar/mission_001 \
    --output-dir /opt/agrodrone/processed/maps/mission_001 \
    --resolution 0.1

# Heatmap generation
cargo run --bin lidar_mapper -- \
    --input-dir /opt/agrodrone/data/lidar \
    --output-dir /opt/agrodrone/processed/heatmaps \
    --mode heatmap
```

**Output formats:**
- PCD point cloud files
- PNG occupancy grids
- JSON metadata and statistics

## 🔧 Cross-Compilation for ARM

### Setup Cross-Compilation

```bash
# Install cross-compilation tool
cargo install cross

# Install ARM target
rustup target add aarch64-unknown-linux-gnu
rustup target add armv7-unknown-linux-gnueabihf
```

### Build for Jetson (ARM64)

```bash
cross build --target aarch64-unknown-linux-gnu --release
```

### Build for Raspberry Pi (ARM)

```bash
cross build --target armv7-unknown-linux-gnueabihf --release
```

### Deploy to Target

```bash
# Copy binaries to target device
scp target/aarch64-unknown-linux-gnu/release/* user@jetson:/opt/agrodrone/bin/

# Setup systemd services (see deployment/systemd/)
sudo systemctl enable agrodrone-mission-control
sudo systemctl start agrodrone-mission-control
```

## 🧪 Testing

### Unit Tests

```bash
# Run all tests
cargo test

# Test specific workspace member
cargo test -p mission_control

# Run with output
cargo test -- --nocapture
```

### Integration Tests

```bash
# Test hardware simulation
RUNTIME_MODE=SIMULATION cargo test --release

# Test API endpoints
curl http://localhost:3000/api/health
curl http://localhost:3000/api/telemetry
```

### Hardware-in-the-Loop Testing

```bash
# Connect actual hardware in test mode
RUNTIME_MODE=FLIGHT TEST_MODE=true cargo run --bin mission_control
```

## 📡 API Reference

### Mission Control REST API

**Base URL**: `http://localhost:3000/api`

#### Health Check
```bash
curl http://localhost:3000/api/health
```

#### Get Telemetry
```bash
curl http://localhost:3000/api/telemetry
```

#### Upload Mission
```bash
curl -X POST http://localhost:3000/api/missions \
  -H "Content-Type: application/json" \
  -d '{
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "Survey Mission 1",
    "created_at": "2024-01-01T12:00:00Z",
    "home_position": {
      "latitude": 37.7749,
      "longitude": -122.4194,
      "altitude": 100.0
    },
    "waypoints": [
      {
        "sequence": 0,
        "position": {"latitude": 37.7750, "longitude": -122.4195, "altitude": 100.0},
        "command": 16,
        "auto_continue": true,
        "param1": 0.0, "param2": 0.0, "param3": 0.0, "param4": 0.0
      }
    ]
  }'
```

### WebSocket Events

Connect to `ws://localhost:8080/ws` for real-time telemetry:

```json
{
  "type": "Telemetry",
  "data": {
    "timestamp": "2024-01-01T12:00:00Z",
    "position": {"latitude": 37.7749, "longitude": -122.4194, "altitude": 100.0},
    "battery_voltage": 12.6,
    "battery_percentage": 85,
    "armed": false,
    "mode": "STABILIZE"
  }
}
```

## 🐳 Docker Deployment

### Build Production Image

```bash
# Build with multi-stage optimization
docker build -t agrodrone:latest .

# Build for specific architecture
docker buildx build --platform linux/arm64 -t agrodrone:arm64 .
```

### Run in Simulation Mode

```bash
docker run -d \
  --name agrodrone-sim \
  -e RUNTIME_MODE=SIMULATION \
  -p 8080:8080 -p 3000:3000 -p 8081:8081 \
  agrodrone:latest
```

### Run with Hardware Access

```bash
docker run -d \
  --name agrodrone-flight \
  --privileged \
  --restart unless-stopped \
  -e RUNTIME_MODE=FLIGHT \
  -v /dev:/dev \
  -v /opt/agrodrone/data:/opt/agrodrone/data \
  -p 8080:8080 -p 3000:3000 -p 8081:8081 \
  agrodrone:latest
```

## � Monitoring and Observability

### Structured Logging

Configure log levels for different components:

```bash
# Debug all components
export RUST_LOG=debug

# Production logging
export RUST_LOG=info,agrodrone=debug

# Per-module configuration
export RUST_LOG="mission_control=debug,sensor_collector=info,tokio=warn"
```

### Performance Monitoring

```bash
# Monitor with htop
htop

# Check CPU/memory usage per service
ps aux | grep agrodrone

# Monitor serial port activity
sudo iotop
```

### Log Analysis

```bash
# Filter logs by component
journalctl -u agrodrone-mission-control -f

# Search for errors
grep "ERROR" /var/log/agrodrone/*.log

# Performance metrics
grep "Processing time" /var/log/agrodrone/*.log
```

## ⚡ Performance Optimization

### Build Optimizations

```bash
# Profile-guided optimization
cargo build --release
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Link-time optimization
RUSTFLAGS="-C lto=fat" cargo build --release
```

### Runtime Tuning

```bash
# Increase thread pool size
export TOKIO_WORKER_THREADS=8

# Tune for embedded systems
export RUST_MIN_STACK=2097152
```

### ARM-Specific Optimizations

```bash
# For Jetson Nano
export CUDA_VISIBLE_DEVICES=0

# For Raspberry Pi
echo 'gpu_mem=128' | sudo tee -a /boot/config.txt
```

## 🔧 Troubleshooting

### Common Issues

#### Serial Port Access
```bash
# Check permissions
ls -la /dev/ttyUSB*

# Add user to dialout group
sudo usermod -a -G dialout $USER
sudo reboot

# Set port permissions
sudo chmod 666 /dev/ttyUSB0
```

#### Camera Detection
```bash
# List video devices
v4l2-ctl --list-devices

# Test camera stream
ffmpeg -f v4l2 -i /dev/video0 -t 5 test.mp4
```

#### Build Failures
```bash
# Clear cache and rebuild
cargo clean
cargo build --release

# Update dependencies
cargo update
```

### Memory Issues (ARM Devices)

```bash
# Increase swap space
sudo fallocate -l 2G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile

# Make permanent
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

### Network Connectivity

```bash
# Test WebSocket connection
websocat ws://localhost:8080/ws

# Test API endpoints
curl -v http://localhost:3000/api/health

# Check port bindings
netstat -tlnp | grep :8080
```

## �️ Development Tools

### Just Commands

```bash
# List available commands
just --list

# Development workflow
just dev          # Start all services in development mode
just test          # Run all tests
just build-arm     # Cross-compile for ARM
just docker-build  # Build Docker image
just clean         # Clean build artifacts
```

### VS Code Integration

Install recommended extensions:
- Rust Analyzer
- CodeLLDB (debugging)
- Better TOML
- Docker

### Debugging

```bash
# Run with debugger
cargo run --bin mission_control
# Then attach VS Code debugger

# Enable backtrace
RUST_BACKTRACE=1 cargo run --bin mission_control

# GDB debugging (Linux)
gdb target/debug/mission_control
```

## �🚀 Production Deployment

### Systemd Services

Create service files in `/etc/systemd/system/`:

```ini
# agrodrone-mission-control.service
[Unit]
Description=AgroDrone Mission Control
After=network.target

[Service]
Type=simple
User=agrodrone
WorkingDirectory=/opt/agrodrone
ExecStart=/opt/agrodrone/bin/mission_control
Restart=always
RestartSec=5
Environment=RUNTIME_MODE=FLIGHT
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

```bash
# Enable and start services
sudo systemctl enable agrodrone-mission-control
sudo systemctl start agrodrone-mission-control
sudo systemctl status agrodrone-mission-control
```

### Container Orchestration

Use Docker Compose for multi-service deployment:

```yaml
# docker-compose.yml
version: '3.8'
services:
  mission-control:
    build: .
    command: mission_control
    environment:
      - RUNTIME_MODE=FLIGHT
    devices:
      - "/dev/ttyUSB0:/dev/ttyUSB0"
    ports:
      - "8080:8080"
      - "3000:3000"
    restart: unless-stopped

  sensor-collector:
    build: .
    command: sensor_collector
    environment:
      - RUNTIME_MODE=FLIGHT
    devices:
      - "/dev/ttyUSB1:/dev/ttyUSB1"
      - "/dev/video0:/dev/video0"
    volumes:
      - "./data:/opt/agrodrone/data"
    restart: unless-stopped

  ground-station:
    build: .
    command: ["ground_station_ui", "--web"]
    ports:
      - "8081:8081"
    restart: unless-stopped
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Guidelines

- Follow Rust best practices and idioms
- Write comprehensive tests for new features
- Update documentation for API changes
- Ensure cross-platform compatibility
- Test on target hardware when possible

## 📞 Support

- **Documentation**: See individual crate README files
- **Issues**: GitHub Issues for bug reports and feature requests
- **Discussions**: GitHub Discussions for questions and community support

## 🏆 Acknowledgments

- **MAVLink**: Micro Air Vehicle communication protocol
- **Tokio**: Asynchronous runtime for Rust
- **RPLIDAR**: Slamtec RPLIDAR sensor family
- **Pixhawk**: Open-source flight controller platform
- **OpenCV**: Computer vision library (future integration)

---

**Built with ❤️ for sustainable agriculture and autonomous systems**

### Release Builds

Always use release builds for production:

```bash
cargo build --release
```

### Memory Usage

Monitor memory usage on embedded systems:

```bash
# Check system memory
free -h

# Monitor process memory
htop

# Rust-specific profiling
cargo install cargo-profiling
cargo profiling --release
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Code Style

```bash
# Format code
cargo fmt

# Lint code
cargo clippy -- -D warnings
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🆘 Support

- **Issues**: [GitHub Issues](https://github.com/your-org/agrodrone/issues)
- **Discussions**: [GitHub Discussions](https://github.com/your-org/agrodrone/discussions)
- **Documentation**: [Wiki](https://github.com/your-org/agrodrone/wiki)

## 🏗️ Project Structure

```
agrodrone/
├── Cargo.toml                 # Workspace configuration
├── .env                       # Environment variables
├── README.md                  # This file
├── shared/                    # Common utilities and types
│   ├── src/
│   │   ├── lib.rs
│   │   ├── config.rs         # Configuration management
│   │   ├── error.rs          # Error types
│   │   └── schemas.rs        # Data structures
├── mission_control/           # Flight controller interface
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── mavlink_client.rs # MAVLink communication
│   │   ├── websocket_server.rs # WebSocket server
│   │   └── api_server.rs     # REST API
├── sensor_collector/          # Sensor data acquisition
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── lidar_reader.rs   # LiDAR interface
│   │   └── camera_reader.rs  # Camera interface
├── ndvi_processor/            # Image processing
│   ├── src/
│   │   ├── main.rs
│   │   └── lib.rs            # NDVI calculation
├── lidar_mapper/              # Point cloud processing
│   ├── src/
│   │   ├── main.rs
│   │   └── lib.rs            # Occupancy mapping
└── ground_station_ui/         # User interface
    ├── src/
    │   ├── main.rs
    │   ├── lib.rs
    │   ├── web_server.rs     # Web dashboard
    │   └── cli_interface.rs  # CLI interface
```

---

**Happy flying with AgroDrone! 🌾🚁**
# agbot
