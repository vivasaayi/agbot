-- Mission Planner Database Initialization Script
-- This script creates the initial database schema for the agbot mission planner

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create missions table
CREATE TABLE IF NOT EXISTS missions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    area_of_interest JSONB NOT NULL,
    estimated_duration_minutes INTEGER NOT NULL DEFAULT 0,
    estimated_battery_usage REAL NOT NULL DEFAULT 0.0,
    weather_constraints JSONB NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

-- Create waypoints table
CREATE TABLE IF NOT EXISTS waypoints (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
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
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mission_id UUID NOT NULL REFERENCES missions(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    segments JSONB NOT NULL DEFAULT '[]'::jsonb,
    total_distance_m REAL NOT NULL DEFAULT 0.0,
    estimated_duration_seconds INTEGER NOT NULL DEFAULT 0,
    path_type JSONB NOT NULL DEFAULT '"Direct"'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for better performance
CREATE INDEX IF NOT EXISTS idx_waypoints_mission_id ON waypoints(mission_id);
CREATE INDEX IF NOT EXISTS idx_flight_paths_mission_id ON flight_paths(mission_id);
CREATE INDEX IF NOT EXISTS idx_missions_created_at ON missions(created_at);
CREATE INDEX IF NOT EXISTS idx_missions_name ON missions(name);
CREATE INDEX IF NOT EXISTS idx_missions_updated_at ON missions(updated_at);
CREATE INDEX IF NOT EXISTS idx_waypoints_sequence_order ON waypoints(mission_id, sequence_order);

-- Create a function to automatically update the updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create trigger to automatically update updated_at on missions table
DROP TRIGGER IF EXISTS update_missions_updated_at ON missions;
CREATE TRIGGER update_missions_updated_at
    BEFORE UPDATE ON missions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Insert sample data for testing (optional)
INSERT INTO missions (
    id, name, description, created_at, updated_at, 
    area_of_interest, estimated_duration_minutes, 
    estimated_battery_usage, weather_constraints, metadata
) VALUES (
    'f47ac10b-58cc-4372-a567-0e02b2c3d479',
    'Sample Survey Mission',
    'A sample agricultural survey mission for testing the agbot system',
    NOW(),
    NOW(),
    '{"type":"Polygon","coordinates":[[[0,0],[0.01,0],[0.01,0.01],[0,0.01],[0,0]]]}',
    45,
    75.5,
    '{"max_wind_speed_ms":15.0,"max_precipitation_mm":2.0,"min_visibility_m":1000.0,"temperature_range_celsius":[-10.0,45.0]}',
    '{"farm_id":"farm_001","crop_type":"corn","season":"spring_2025","notes":"Initial test mission"}'
) ON CONFLICT (id) DO NOTHING;

-- Insert sample waypoints for the test mission
INSERT INTO waypoints (
    id, mission_id, position, altitude_m, waypoint_type,
    actions, sequence_order
) VALUES 
(
    uuid_generate_v4(),
    'f47ac10b-58cc-4372-a567-0e02b2c3d479',
    '{"type":"Point","coordinates":[0,0]}',
    100.0,
    'Takeoff',
    '[]',
    0
),
(
    uuid_generate_v4(),
    'f47ac10b-58cc-4372-a567-0e02b2c3d479',
    '{"type":"Point","coordinates":[0.005,0.005]}',
    120.0,
    'Survey',
    '[{"TakePhoto":{"camera_id":"main_camera","settings":{"iso":100,"shutter_speed_ms":125}}}]',
    1
),
(
    uuid_generate_v4(),
    'f47ac10b-58cc-4372-a567-0e02b2c3d479',
    '{"type":"Point","coordinates":[0.01,0.01]}',
    100.0,
    'Landing',
    '[]',
    2
);

-- Insert sample flight path
INSERT INTO flight_paths (
    id, mission_id, name, segments, 
    total_distance_m, estimated_duration_seconds, path_type
) VALUES (
    uuid_generate_v4(),
    'f47ac10b-58cc-4372-a567-0e02b2c3d479',
    'Survey Pattern Alpha',
    '[{"start_waypoint_id":"00000000-0000-0000-0000-000000000000","end_waypoint_id":"00000000-0000-0000-0000-000000000001","distance_m":707.1,"bearing_degrees":45.0,"estimated_time_seconds":120}]',
    1500.0,
    2700,
    '"Survey"'
);

-- Grant permissions (if needed for specific users)
-- GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO postgres;
-- GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO postgres;

-- Display creation summary
DO $$
BEGIN
    RAISE NOTICE 'AgBot Mission Planner database initialized successfully!';
    RAISE NOTICE 'Tables created: missions, waypoints, flight_paths';
    RAISE NOTICE 'Sample data inserted for testing';
    RAISE NOTICE 'Ready for mission planning operations';
END $$;
