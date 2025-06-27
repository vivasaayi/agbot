# Justfile for AgroDrone project
# Run `just --list` to see all available tasks

# Set default recipe
default: help

# Show help
help:
    @echo "🚁 AgroDrone Development Tasks"
    @echo ""
    @echo "Setup:"
    @echo "  setup     - Install dependencies and setup project"
    @echo "  clean     - Clean build artifacts"
    @echo ""
    @echo "Build:"
    @echo "  build     - Build all workspace members"
    @echo "  check     - Check code without building"
    @echo "  test      - Run tests"
    @echo "  fmt       - Format code"
    @echo "  clippy    - Run clippy linter"
    @echo ""
    @echo "Development:"
    @echo "  dev       - Start development environment"
    @echo "  mission   - Start mission control only"
    @echo "  sensors   - Start sensor collector only"
    @echo "  ui        - Start ground station UI only"
    @echo ""
    @echo "Processing:"
    @echo "  ndvi      - Process NDVI from sample data"
    @echo "  lidar     - Process LiDAR from sample data"
    @echo ""
    @echo "Deployment:"
    @echo "  docker    - Build Docker image"
    @echo "  arm       - Cross-compile for ARM (Jetson/Pi)"

# Setup project
setup:
    @echo "🔧 Setting up AgroDrone development environment..."
    mkdir -p data/{lidar,camera}
    mkdir -p missions
    mkdir -p processed/{ndvi,maps}
    @echo "✅ Directories created"
    cargo fetch
    @echo "✅ Dependencies fetched"

# Build all components
build:
    @echo "🔨 Building all workspace members..."
    cargo build

# Build release version
build-release:
    @echo "🔨 Building release version..."
    cargo build --release

# Check code
check:
    @echo "🔍 Checking code..."
    cargo check

# Run tests
test:
    @echo "🧪 Running tests..."
    cargo test

# Format code
fmt:
    @echo "📝 Formatting code..."
    cargo fmt

# Run clippy
clippy:
    @echo "🔍 Running clippy..."
    cargo clippy -- -D warnings

# Clean build artifacts
clean:
    @echo "🧹 Cleaning build artifacts..."
    cargo clean
    rm -rf data/* missions/* processed/*

# Start development environment
dev:
    @echo "🚀 Starting development environment..."
    ./dev-start.sh

# Start mission control only
mission:
    @echo "🎮 Starting Mission Control..."
    RUST_LOG=debug cargo run --bin mission_control

# Start sensor collector only
sensors:
    @echo "📡 Starting Sensor Collector..."
    RUST_LOG=debug cargo run --bin sensor_collector

# Start ground station UI only (web)
ui:
    @echo "🖥️ Starting Ground Station UI..."
    RUST_LOG=debug cargo run --bin ground_station_ui -- --web

# Start ground station UI (CLI)
ui-cli:
    @echo "💻 Starting Ground Station CLI..."
    RUST_LOG=debug cargo run --bin ground_station_ui

# Process NDVI from sample data
ndvi:
    @echo "🌱 Processing NDVI..."
    cargo run --bin ndvi_processor -- --input-dir data/camera --output-dir processed/ndvi

# Process LiDAR from sample data
lidar:
    @echo "📊 Processing LiDAR..."
    cargo run --bin lidar_mapper -- --input-dir data/lidar --output-dir processed/maps

# Build Docker image
docker:
    @echo "🐳 Building Docker image..."
    docker build -t agrodrone:latest .

# Cross-compile for ARM64 (Jetson)
arm64:
    @echo "🔧 Cross-compiling for ARM64..."
    cross build --target aarch64-unknown-linux-gnu --release

# Cross-compile for ARM (Raspberry Pi)
arm:
    @echo "🔧 Cross-compiling for ARM..."
    cross build --target armv7-unknown-linux-gnueabihf --release

# Install development tools
install-tools:
    @echo "🛠️ Installing development tools..."
    cargo install cross
    cargo install cargo-watch
    @echo "✅ Tools installed"

# Watch for changes and rebuild
watch:
    @echo "👀 Watching for changes..."
    cargo watch -x check

# Generate sample mission
sample-mission:
    @echo "📋 Generating sample mission..."
    @mkdir -p missions
    @cat > missions/sample_mission.json << 'EOF'
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "Sample Survey Mission",
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
    },
    {
      "sequence": 1,
      "position": {"latitude": 37.7751, "longitude": -122.4195, "altitude": 100.0},
      "command": 16,
      "auto_continue": true,
      "param1": 0.0, "param2": 0.0, "param3": 0.0, "param4": 0.0
    }
  ]
}
EOF
    @echo "✅ Sample mission created at missions/sample_mission.json"

# Show project status
status:
    @echo "📊 Project Status"
    @echo "=================="
    @echo "Workspace members:"
    @cargo metadata --format-version 1 | jq -r '.workspace_members[]' | sed 's/.*\//  - /'
    @echo ""
    @echo "Build targets:"
    @ls -la target/ 2>/dev/null | grep ^d || echo "  No build artifacts"
    @echo ""
    @echo "Data directories:"
    @find data -type d 2>/dev/null | sed 's/^/  /' || echo "  No data directories"
