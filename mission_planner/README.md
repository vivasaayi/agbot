# Mission Planner with PostgreSQL

A comprehensive mission planning system for agricultural drones with PostgreSQL database backend and REST API.

## Features

- **PostgreSQL Database**: Persistent storage for missions, waypoints, and flight paths
- **REST API**: Complete CRUD operations for missions
- **CLI Tool**: Command-line interface for mission management
- **Mission Optimization**: Automatic flight path optimization
- **Search & Filtering**: Search missions by name or description
- **Statistics**: Mission analytics and reporting

## Database Schema

The system uses three main tables:
- `missions`: Core mission information and metadata
- `waypoints`: Individual waypoints with positions and actions
- `flight_paths`: Optimized flight paths with segments

## Setup

### Prerequisites

1. **PostgreSQL**: Install and run PostgreSQL server
2. **Rust**: Install Rust toolchain

### Database Setup

1. Start PostgreSQL server
2. Run the setup script:
```bash
cd mission_planner
./scripts/setup_database.sh
```

Or manually configure:
```bash
# Create database
createdb agbot

# Set environment variable
export DATABASE_URL="postgres://postgres:password@localhost:5432/agbot"
```

### Build and Run

```bash
# Build the project
cargo build --release

# Run the API server
cargo run --bin server

# Or use the CLI tool
cargo run --bin cli -- --help
```

## API Usage

### Start the Server

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5432/agbot"
cargo run --bin server
```

The API will be available at `http://localhost:3000`

### API Endpoints

#### Create Mission
```bash
curl -X POST http://localhost:3000/api/v1/missions \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Farm Survey",
    "description": "Corn field NDVI survey",
    "area_of_interest": {
      "type": "Polygon",
      "coordinates": [[[0,0],[1,0],[1,1],[0,1],[0,0]]]
    }
  }'
```

#### List Missions
```bash
curl http://localhost:3000/api/v1/missions?limit=10&offset=0
```

#### Get Mission
```bash
curl http://localhost:3000/api/v1/missions/{mission-id}
```

#### Update Mission
```bash
curl -X PUT http://localhost:3000/api/v1/missions/{mission-id} \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Updated Mission Name",
    "description": "Updated description"
  }'
```

#### Delete Mission
```bash
curl -X DELETE http://localhost:3000/api/v1/missions/{mission-id}
```

#### Search Missions
```bash
curl "http://localhost:3000/api/v1/missions/search?q=corn"
```

#### Optimize Mission
```bash
curl -X POST http://localhost:3000/api/v1/missions/{mission-id}/optimize
```

#### Get Statistics
```bash
curl http://localhost:3000/api/v1/missions/stats
```

## CLI Usage

### Basic Commands

```bash
# Create a mission
cargo run --bin cli create --name "Test Mission" --description "Test description"

# List missions
cargo run --bin cli list --limit 5

# Get mission details
cargo run --bin cli get {mission-id}

# Update mission
cargo run --bin cli update {mission-id} --name "New Name"

# Delete mission
cargo run --bin cli delete {mission-id}

# Search missions
cargo run --bin cli search "corn"

# Add waypoint to mission
cargo run --bin cli add-waypoint {mission-id} --lat 40.7128 --lon -74.0060 --altitude 100

# Get statistics
cargo run --bin cli stats
```

### Advanced Usage

```bash
# Create mission with custom area
cargo run --bin cli create \
  --name "Complex Survey" \
  --description "Multi-field survey" \
  --area '{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}'

# List with pagination
cargo run --bin cli list --limit 20 --offset 10
```

## Environment Variables

- `DATABASE_URL`: PostgreSQL connection string
- `PORT`: API server port (default: 3000)
- `RUST_LOG`: Logging level (default: info)

## Development

### Running Tests

```bash
# Unit tests (requires test database)
export TEST_DATABASE_URL="postgres://postgres:password@localhost:5432/agbot_test"
cargo test

# Integration tests
cargo test --test integration_tests -- --ignored
```

### Database Migrations

The database schema is automatically created when the service starts. For manual setup:

```sql
-- Connect to PostgreSQL
psql postgres://postgres:password@localhost:5432/agbot

-- Tables are created automatically, but you can also run:
\i scripts/schema.sql
```

## Data Models

### Mission
```json
{
  "id": "uuid",
  "name": "string",
  "description": "string",
  "created_at": "timestamp",
  "updated_at": "timestamp",
  "area_of_interest": "GeoJSON Polygon",
  "waypoints": ["Waypoint"],
  "flight_paths": ["FlightPath"],
  "estimated_duration_minutes": "integer",
  "estimated_battery_usage": "float",
  "weather_constraints": "WeatherConstraints",
  "metadata": "object"
}
```

### Waypoint
```json
{
  "id": "uuid",
  "position": "GeoJSON Point",
  "altitude_m": "float",
  "waypoint_type": "enum",
  "actions": ["Action"],
  "arrival_time": "timestamp?",
  "speed_ms": "float?",
  "heading_degrees": "float?"
}
```

### Flight Path
```json
{
  "id": "uuid",
  "name": "string",
  "segments": ["PathSegment"],
  "total_distance_m": "float",
  "estimated_duration_minutes": "integer"
}
```

## Error Handling

The API returns structured error responses:

```json
{
  "error": "ERROR_CODE",
  "message": "Human readable error message"
}
```

Common error codes:
- `NOT_FOUND`: Resource not found
- `CREATE_FAILED`: Failed to create resource
- `UPDATE_FAILED`: Failed to update resource
- `DELETE_FAILED`: Failed to delete resource
- `VALIDATION_ERROR`: Invalid input data

## Performance Notes

- Database indexes are created for common query patterns
- Use pagination (`limit`/`offset`) for large result sets
- JSONB columns support efficient querying of complex data
- Consider connection pooling for high-traffic deployments

## Security Considerations

- Use environment variables for database credentials
- Consider adding authentication middleware for production
- Validate all input data
- Use prepared statements (handled by sqlx)
- Enable SSL for database connections in production
