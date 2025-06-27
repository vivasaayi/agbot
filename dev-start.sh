#!/bin/bash
# Development startup script for AgroDrone

set -e

echo "🚁 Starting AgroDrone Development Environment"

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Cargo not found. Please install Rust: https://rustup.rs/"
    exit 1
fi

# Create data directories
mkdir -p data/{lidar,camera}
mkdir -p missions
mkdir -p processed/{ndvi,maps}

echo "📁 Created data directories"

# Build all projects
echo "🔨 Building all workspace members..."
cargo build

echo "✅ Build complete"

# Set simulation mode
export RUNTIME_MODE=SIMULATION

echo "🎮 Starting services in simulation mode..."

# Start services in background
echo "Starting Mission Control..."
cargo run --bin mission_control &
MISSION_PID=$!

sleep 2

echo "Starting Sensor Collector..."
cargo run --bin sensor_collector &
SENSOR_PID=$!

sleep 2

echo "Starting Ground Station Web UI..."
cargo run --bin ground_station_ui -- --web &
WEB_PID=$!

sleep 2

echo "🌐 Services started!"
echo "   - Mission Control API: http://localhost:3000"
echo "   - WebSocket Stream: ws://localhost:8080/ws"  
echo "   - Ground Station UI: http://localhost:8081"
echo ""
echo "🎯 Try these commands:"
echo "   curl http://localhost:3000/health"
echo "   curl http://localhost:3000/missions"
echo ""
echo "Press Ctrl+C to stop all services"

# Function to cleanup processes
cleanup() {
    echo ""
    echo "🛑 Stopping services..."
    kill $MISSION_PID $SENSOR_PID $WEB_PID 2>/dev/null || true
    wait $MISSION_PID $SENSOR_PID $WEB_PID 2>/dev/null || true
    echo "✅ All services stopped"
}

# Set trap to cleanup on exit
trap cleanup EXIT

# Wait for user input
read -p "Press Enter to stop all services..."
