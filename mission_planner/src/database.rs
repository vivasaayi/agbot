use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{Mission, Waypoint, FlightPath};

/// Database service for mission storage using PostgreSQL
pub struct DatabaseService {
    pool: PgPool,
}

impl DatabaseService {
    /// Create a new database service with the given connection pool
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Connect to PostgreSQL database
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self::new(pool))
    }

    /// Initialize database tables
    pub async fn initialize(&self) -> Result<()> {
        // Create missions table
        sqlx::query(
            r#"
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
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create waypoints table
        sqlx::query(
            r#"
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
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create flight_paths table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS flight_paths (
                id UUID PRIMARY KEY,
                mission_id UUID NOT NULL REFERENCES missions(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                segments JSONB NOT NULL DEFAULT '[]'::jsonb,
                total_distance_m REAL NOT NULL DEFAULT 0.0,
                estimated_duration_seconds INTEGER NOT NULL DEFAULT 0,
                path_type JSONB NOT NULL DEFAULT '"Direct"'::jsonb,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for better performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_waypoints_mission_id ON waypoints(mission_id);")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_flight_paths_mission_id ON flight_paths(mission_id);")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_missions_created_at ON missions(created_at);")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Create a new mission in the database
    pub async fn create_mission(&self, mission: &Mission) -> Result<Uuid> {
        let mut tx = self.pool.begin().await?;

        // Insert mission
        sqlx::query(
            r#"
            INSERT INTO missions (
                id, name, description, created_at, updated_at, 
                area_of_interest, estimated_duration_minutes, 
                estimated_battery_usage, weather_constraints, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(mission.id)
        .bind(&mission.name)
        .bind(&mission.description)
        .bind(mission.created_at)
        .bind(mission.updated_at)
        .bind(serde_json::to_value(&mission.area_of_interest)?)
        .bind(mission.estimated_duration_minutes as i32)
        .bind(mission.estimated_battery_usage)
        .bind(serde_json::to_value(&mission.weather_constraints)?)
        .bind(serde_json::to_value(&mission.metadata)?)
        .execute(&mut *tx)
        .await?;

        // Insert waypoints
        for (index, waypoint) in mission.waypoints.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO waypoints (
                    id, mission_id, position, altitude_m, waypoint_type,
                    actions, arrival_time, speed_ms, heading_degrees, sequence_order
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(waypoint.id)
            .bind(mission.id)
            .bind(serde_json::to_value(&waypoint.position)?)
            .bind(waypoint.altitude_m)
            .bind(serde_json::to_string(&waypoint.waypoint_type)?)
            .bind(serde_json::to_value(&waypoint.actions)?)
            .bind(waypoint.arrival_time)
            .bind(waypoint.speed_ms)
            .bind(waypoint.heading_degrees)
            .bind(index as i32)
            .execute(&mut *tx)
            .await?;
        }

        // Insert flight paths
        for flight_path in &mission.flight_paths {
            sqlx::query(
                r#"
                INSERT INTO flight_paths (
                    id, mission_id, name, segments, 
                    total_distance_m, estimated_duration_seconds, path_type
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(flight_path.id)
            .bind(mission.id)
            .bind(&flight_path.name)
            .bind(serde_json::to_value(&flight_path.segments)?)
            .bind(flight_path.total_distance_m)
            .bind(flight_path.estimated_duration_seconds as i32)
            .bind(serde_json::to_value(&flight_path.path_type)?)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(mission.id)
    }

    /// Get a mission by ID
    pub async fn get_mission(&self, id: &Uuid) -> Result<Option<Mission>> {
        // Get mission basic info
        let mission_row = sqlx::query(
            "SELECT * FROM missions WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        let mission_row = match mission_row {
            Some(row) => row,
            None => return Ok(None),
        };

        // Get waypoints
        let waypoint_rows = sqlx::query(
            "SELECT * FROM waypoints WHERE mission_id = $1 ORDER BY sequence_order"
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;

        let waypoints: Result<Vec<Waypoint>, anyhow::Error> = waypoint_rows
            .iter()
            .map(|row| {
                Ok(Waypoint {
                    id: row.get("id"),
                    position: serde_json::from_value(row.get("position"))?,
                    altitude_m: row.get("altitude_m"),
                    waypoint_type: serde_json::from_str(&row.get::<String, _>("waypoint_type"))?,
                    actions: serde_json::from_value(row.get("actions"))?,
                    arrival_time: row.get("arrival_time"),
                    speed_ms: row.get("speed_ms"),
                    heading_degrees: row.get("heading_degrees"),
                })
            })
            .collect();

        // Get flight paths
        let flight_path_rows = sqlx::query(
            "SELECT * FROM flight_paths WHERE mission_id = $1"
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;

        let flight_paths: Result<Vec<FlightPath>, anyhow::Error> = flight_path_rows
            .iter()
            .map(|row| {
                Ok(FlightPath {
                    id: row.get("id"),
                    name: row.get("name"),
                    segments: serde_json::from_value(row.get("segments"))?,
                    total_distance_m: row.get("total_distance_m"),
                    estimated_duration_seconds: row.get::<i32, _>("estimated_duration_seconds") as u32,
                    path_type: serde_json::from_value(row.get("path_type"))?,
                })
            })
            .collect();

        let mission = Mission {
            id: mission_row.get("id"),
            name: mission_row.get("name"),
            description: mission_row.get("description"),
            created_at: mission_row.get("created_at"),
            updated_at: mission_row.get("updated_at"),
            area_of_interest: serde_json::from_value(mission_row.get("area_of_interest"))?,
            waypoints: waypoints?,
            flight_paths: flight_paths?,
            estimated_duration_minutes: mission_row.get::<i32, _>("estimated_duration_minutes") as u32,
            estimated_battery_usage: mission_row.get("estimated_battery_usage"),
            weather_constraints: serde_json::from_value(mission_row.get("weather_constraints"))?,
            metadata: serde_json::from_value(mission_row.get("metadata"))?,
        };

        Ok(Some(mission))
    }

    /// Update a mission
    pub async fn update_mission(&self, mission: &Mission) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Update mission
        let rows_affected = sqlx::query(
            r#"
            UPDATE missions SET 
                name = $2, description = $3, updated_at = $4,
                area_of_interest = $5, estimated_duration_minutes = $6,
                estimated_battery_usage = $7, weather_constraints = $8, metadata = $9
            WHERE id = $1
            "#,
        )
        .bind(mission.id)
        .bind(&mission.name)
        .bind(&mission.description)
        .bind(mission.updated_at)
        .bind(serde_json::to_value(&mission.area_of_interest)?)
        .bind(mission.estimated_duration_minutes as i32)
        .bind(mission.estimated_battery_usage)
        .bind(serde_json::to_value(&mission.weather_constraints)?)
        .bind(serde_json::to_value(&mission.metadata)?)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(anyhow::anyhow!("Mission not found"));
        }

        // Delete existing waypoints and flight paths
        sqlx::query("DELETE FROM waypoints WHERE mission_id = $1")
            .bind(mission.id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM flight_paths WHERE mission_id = $1")
            .bind(mission.id)
            .execute(&mut *tx)
            .await?;

        // Insert updated waypoints
        for (index, waypoint) in mission.waypoints.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO waypoints (
                    id, mission_id, position, altitude_m, waypoint_type,
                    actions, arrival_time, speed_ms, heading_degrees, sequence_order
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(waypoint.id)
            .bind(mission.id)
            .bind(serde_json::to_value(&waypoint.position)?)
            .bind(waypoint.altitude_m)
            .bind(serde_json::to_string(&waypoint.waypoint_type)?)
            .bind(serde_json::to_value(&waypoint.actions)?)
            .bind(waypoint.arrival_time)
            .bind(waypoint.speed_ms)
            .bind(waypoint.heading_degrees)
            .bind(index as i32)
            .execute(&mut *tx)
            .await?;
        }

        // Insert updated flight paths
        for flight_path in &mission.flight_paths {
            sqlx::query(
                r#"
                INSERT INTO flight_paths (
                    id, mission_id, name, segments, 
                    total_distance_m, estimated_duration_seconds, path_type
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(flight_path.id)
            .bind(mission.id)
            .bind(&flight_path.name)
            .bind(serde_json::to_value(&flight_path.segments)?)
            .bind(flight_path.total_distance_m)
            .bind(flight_path.estimated_duration_seconds as i32)
            .bind(serde_json::to_value(&flight_path.path_type)?)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// List all missions with optional filtering
    pub async fn list_missions(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<Mission>> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);

        let mission_rows = sqlx::query(
            "SELECT * FROM missions ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut missions = Vec::new();

        for row in mission_rows {
            let mission_id: Uuid = row.get("id");
            
            // Get waypoints for this mission
            let waypoint_rows = sqlx::query(
                "SELECT * FROM waypoints WHERE mission_id = $1 ORDER BY sequence_order"
            )
            .bind(mission_id)
            .fetch_all(&self.pool)
            .await?;

            let waypoints: Result<Vec<Waypoint>, anyhow::Error> = waypoint_rows
                .iter()
                .map(|row| {
                    Ok(Waypoint {
                        id: row.get("id"),
                        position: serde_json::from_value(row.get("position"))?,
                        altitude_m: row.get("altitude_m"),
                        waypoint_type: serde_json::from_str(&row.get::<String, _>("waypoint_type"))?,
                        actions: serde_json::from_value(row.get("actions"))?,
                        arrival_time: row.get("arrival_time"),
                        speed_ms: row.get("speed_ms"),
                        heading_degrees: row.get("heading_degrees"),
                    })
                })
                .collect();

            // Get flight paths for this mission
            let flight_path_rows = sqlx::query(
                "SELECT * FROM flight_paths WHERE mission_id = $1"
            )
            .bind(mission_id)
            .fetch_all(&self.pool)
            .await?;

            let flight_paths: Result<Vec<FlightPath>, anyhow::Error> = flight_path_rows
                .iter()
                .map(|row| {
                    Ok(FlightPath {
                        id: row.get("id"),
                        name: row.get("name"),
                        segments: serde_json::from_value(row.get("segments"))?,
                        total_distance_m: row.get("total_distance_m"),
                        estimated_duration_seconds: row.get::<i32, _>("estimated_duration_seconds") as u32,
                        path_type: serde_json::from_value(row.get("path_type"))?,
                    })
                })
                .collect();

            let mission = Mission {
                id: mission_id,
                name: row.get("name"),
                description: row.get("description"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                area_of_interest: serde_json::from_value(row.get("area_of_interest"))?,
                waypoints: waypoints?,
                flight_paths: flight_paths?,
                estimated_duration_minutes: row.get::<i32, _>("estimated_duration_minutes") as u32,
                estimated_battery_usage: row.get("estimated_battery_usage"),
                weather_constraints: serde_json::from_value(row.get("weather_constraints"))?,
                metadata: serde_json::from_value(row.get("metadata"))?,
            };

            missions.push(mission);
        }

        Ok(missions)
    }

    /// Delete a mission
    pub async fn delete_mission(&self, id: &Uuid) -> Result<()> {
        let rows_affected = sqlx::query("DELETE FROM missions WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected();

        if rows_affected == 0 {
            return Err(anyhow::anyhow!("Mission not found"));
        }

        Ok(())
    }

    /// Search missions by name or description
    pub async fn search_missions(&self, query: &str) -> Result<Vec<Mission>> {
        let pattern = format!("%{}%", query);
        
        let mission_rows = sqlx::query(
            r#"
            SELECT * FROM missions 
            WHERE name ILIKE $1 OR description ILIKE $1
            ORDER BY created_at DESC
            "#
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;

        let mut missions = Vec::new();

        for row in mission_rows {
            let mission_id: Uuid = row.get("id");
            
            // Get waypoints for this mission
            let waypoint_rows = sqlx::query(
                "SELECT * FROM waypoints WHERE mission_id = $1 ORDER BY sequence_order"
            )
            .bind(mission_id)
            .fetch_all(&self.pool)
            .await?;

            let waypoints: Result<Vec<Waypoint>, anyhow::Error> = waypoint_rows
                .iter()
                .map(|row| {
                    Ok(Waypoint {
                        id: row.get("id"),
                        position: serde_json::from_value(row.get("position"))?,
                        altitude_m: row.get("altitude_m"),
                        waypoint_type: serde_json::from_str(&row.get::<String, _>("waypoint_type"))?,
                        actions: serde_json::from_value(row.get("actions"))?,
                        arrival_time: row.get("arrival_time"),
                        speed_ms: row.get("speed_ms"),
                        heading_degrees: row.get("heading_degrees"),
                    })
                })
                .collect();

            // Get flight paths for this mission
            let flight_path_rows = sqlx::query(
                "SELECT * FROM flight_paths WHERE mission_id = $1"
            )
            .bind(mission_id)
            .fetch_all(&self.pool)
            .await?;

            let flight_paths: Result<Vec<FlightPath>, anyhow::Error> = flight_path_rows
                .iter()
                .map(|row| {
                    Ok(FlightPath {
                        id: row.get("id"),
                        name: row.get("name"),
                        segments: serde_json::from_value(row.get("segments"))?,
                        total_distance_m: row.get("total_distance_m"),
                        estimated_duration_seconds: row.get::<i32, _>("estimated_duration_seconds") as u32,
                        path_type: serde_json::from_value(row.get("path_type"))?,
                    })
                })
                .collect();

            let mission = Mission {
                id: mission_id,
                name: row.get("name"),
                description: row.get("description"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                area_of_interest: serde_json::from_value(row.get("area_of_interest"))?,
                waypoints: waypoints?,
                flight_paths: flight_paths?,
                estimated_duration_minutes: row.get::<i32, _>("estimated_duration_minutes") as u32,
                estimated_battery_usage: row.get("estimated_battery_usage"),
                weather_constraints: serde_json::from_value(row.get("weather_constraints"))?,
                metadata: serde_json::from_value(row.get("metadata"))?,
            };

            missions.push(mission);
        }

        Ok(missions)
    }

    /// Get mission statistics
    pub async fn get_mission_stats(&self) -> Result<MissionStats> {
        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_missions,
                AVG(estimated_duration_minutes) as avg_duration,
                AVG(estimated_battery_usage) as avg_battery_usage,
                MIN(created_at) as oldest_mission,
                MAX(created_at) as newest_mission
            FROM missions
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(MissionStats {
            total_missions: row.get::<i64, _>("total_missions") as u64,
            average_duration_minutes: row.get::<Option<f64>, _>("avg_duration").unwrap_or(0.0) as f32,
            average_battery_usage: row.get::<Option<f64>, _>("avg_battery_usage").unwrap_or(0.0) as f32,
            oldest_mission: row.get("oldest_mission"),
            newest_mission: row.get("newest_mission"),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MissionStats {
    pub total_missions: u64,
    pub average_duration_minutes: f32,
    pub average_battery_usage: f32,
    pub oldest_mission: Option<DateTime<Utc>>,
    pub newest_mission: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{coord, polygon};

    async fn setup_test_db() -> DatabaseService {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/agbot_test".to_string());
        
        let service = DatabaseService::connect(&database_url).await.unwrap();
        service.initialize().await.unwrap();
        service
    }

    #[tokio::test]
    #[ignore] // Requires PostgreSQL database
    async fn test_mission_crud() {
        let db = setup_test_db().await;

        let area = polygon![
            (x: 0.0, y: 0.0),
            (x: 1.0, y: 0.0),
            (x: 1.0, y: 1.0),
            (x: 0.0, y: 1.0),
            (x: 0.0, y: 0.0),
        ];

        let mut mission = Mission::new(
            "Test Mission".to_string(),
            "A test mission".to_string(),
            area,
        );

        // Create mission
        let id = db.create_mission(&mission).await.unwrap();
        assert_eq!(id, mission.id);

        // Get mission
        let retrieved = db.get_mission(&id).await.unwrap().unwrap();
        assert_eq!(retrieved.name, mission.name);
        assert_eq!(retrieved.description, mission.description);

        // Update mission
        mission.name = "Updated Mission".to_string();
        mission.updated_at = Utc::now();
        db.update_mission(&mission).await.unwrap();

        let updated = db.get_mission(&id).await.unwrap().unwrap();
        assert_eq!(updated.name, "Updated Mission");

        // List missions
        let missions = db.list_missions(None, None).await.unwrap();
        assert!(!missions.is_empty());

        // Delete mission
        db.delete_mission(&id).await.unwrap();
        let deleted = db.get_mission(&id).await.unwrap();
        assert!(deleted.is_none());
    }
}
