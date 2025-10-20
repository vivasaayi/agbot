#!/bin/bash

# Database setup script for Mission Planner PostgreSQL

set -e

# Default database configuration
DB_HOST=${DB_HOST:-localhost}
DB_PORT=${DB_PORT:-5432}
DB_USER=${DB_USER:-postgres}
DB_PASSWORD=${DB_PASSWORD:-password}
DB_NAME=${DB_NAME:-agbot}

# Create database URL
DATABASE_URL="postgres://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}"

echo "Setting up Mission Planner database..."
echo "Database URL: postgres://${DB_USER}:***@${DB_HOST}:${DB_PORT}/${DB_NAME}"

# Check if PostgreSQL is running
if ! pg_isready -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" > /dev/null 2>&1; then
    echo "Error: PostgreSQL is not running or not accessible"
    echo "Please ensure PostgreSQL is running on ${DB_HOST}:${DB_PORT}"
    exit 1
fi

# Create database if it doesn't exist
echo "Creating database if it doesn't exist..."
createdb -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" "$DB_NAME" 2>/dev/null || echo "Database already exists or could not be created"

# Run the mission planner with database initialization
echo "Initializing database tables..."
export DATABASE_URL="$DATABASE_URL"

# Create a simple initialization script
cat > /tmp/init_db.sql << EOF
-- Mission Planner Database Schema

-- Create missions table
CREATE TABLE IF NOT EXISTS missions (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    area_of_interest JSONB NOT NULL,
    estimated_duration_minutes INTEGER NOT NULL DEFAULT 0,
    estimated_battery_usage REAL NOT NULL DEFAULT 0.0,
    weather_constraints JSONB NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

-- Create waypoints table
CREATE TABLE IF NOT EXISTS waypoints (
    id UUID PRIMARY KEY,
    mission_id UUID NOT NULL REFERENCES missions(id) ON DELETE CASCADE,
    position JSONB NOT NULL,
    altitude_m REAL NOT NULL,
    waypoint_type TEXT NOT NULL,
    actions JSONB NOT NULL DEFAULT '[]'::jsonb,
    arrival_time TIMESTAMPTZ,
    speed_ms REAL,
    heading_degrees REAL,
    sequence_order INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create flight_paths table
CREATE TABLE IF NOT EXISTS flight_paths (
    id UUID PRIMARY KEY,
    mission_id UUID NOT NULL REFERENCES missions(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    segments JSONB NOT NULL DEFAULT '[]'::jsonb,
    total_distance_m REAL NOT NULL DEFAULT 0.0,
    estimated_duration_minutes INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for better performance
CREATE INDEX IF NOT EXISTS idx_waypoints_mission_id ON waypoints(mission_id);
CREATE INDEX IF NOT EXISTS idx_flight_paths_mission_id ON flight_paths(mission_id);
CREATE INDEX IF NOT EXISTS idx_missions_created_at ON missions(created_at);
CREATE INDEX IF NOT EXISTS idx_missions_name ON missions(name);
CREATE INDEX IF NOT EXISTS idx_missions_updated_at ON missions(updated_at);

-- Add some sample data for testing (optional)
INSERT INTO missions (
    id, name, description, created_at, updated_at, 
    area_of_interest, estimated_duration_minutes, 
    estimated_battery_usage, weather_constraints, metadata
) VALUES (
    'f47ac10b-58cc-4372-a567-0e02b2c3d479',
    'Sample Survey Mission',
    'A sample agricultural survey mission for testing',
    NOW(),
    NOW(),
    '{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}',
    45,
    75.5,
    '{"max_wind_speed_ms":15.0,"max_precipitation_mm":2.0,"min_visibility_m":1000.0,"temperature_range_celsius":[-10.0,45.0]}',
    '{"farm_id":"farm_001","crop_type":"corn","season":"spring_2025"}'
) ON CONFLICT (id) DO NOTHING;

EOF

# Execute the SQL script
echo "Executing database schema..."
psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -f /tmp/init_db.sql

# Clean up
rm /tmp/init_db.sql

echo "Database setup complete!"
echo ""
echo "To connect to your database:"
echo "  psql '$DATABASE_URL'"
echo ""
echo "To start the mission planner server:"
echo "  export DATABASE_URL='$DATABASE_URL'"
echo "  cargo run --bin server"
echo ""
echo "API will be available at:"
echo "  http://localhost:3000/health"
echo "  http://localhost:3000/api/v1/missions"
