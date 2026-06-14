use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use sqlx::{postgres::PgRow, PgPool, Postgres, QueryBuilder, Row};
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    FlightPath, Mission, MissionListFilter, MissionListPage, MissionRevision, MissionStatus,
    Waypoint,
};

fn mission_from_row(
    row: &PgRow,
    waypoints: Vec<Waypoint>,
    flight_paths: Vec<FlightPath>,
) -> Result<Mission> {
    let status = MissionStatus::from_str(&row.get::<String, _>("status"))?;
    Ok(Mission {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get::<i32, _>("version").max(1) as u32,
        field_id: row.get("field_id"),
        season_id: row.get("season_id"),
        session_id: row.get("session_id"),
        owner_id: row.get("owner_id"),
        status,
        area_of_interest: serde_json::from_value(row.get("area_of_interest"))?,
        waypoints,
        flight_paths,
        estimated_duration_minutes: row.get::<i32, _>("estimated_duration_minutes") as u32,
        estimated_battery_usage: row.get("estimated_battery_usage"),
        weather_constraints: serde_json::from_value(row.get("weather_constraints"))?,
        metadata: serde_json::from_value(row.get("metadata"))?,
    })
}

fn mission_page_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(100).clamp(1, 500)
}

fn mission_page_offset(offset: Option<i64>) -> i64 {
    offset.unwrap_or(0).max(0)
}

fn push_filter_separator(builder: &mut QueryBuilder<'_, Postgres>, has_where: &mut bool) {
    if *has_where {
        builder.push(" AND ");
    } else {
        builder.push(" WHERE ");
        *has_where = true;
    }
}

fn append_mission_filters<'args>(
    builder: &mut QueryBuilder<'args, Postgres>,
    filter: &'args MissionListFilter,
) {
    let mut has_where = false;
    if let Some(field_id) = filter
        .field_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        push_filter_separator(builder, &mut has_where);
        builder.push("field_id = ");
        builder.push_bind(field_id);
    }
    if let Some(season_id) = filter
        .season_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        push_filter_separator(builder, &mut has_where);
        builder.push("season_id = ");
        builder.push_bind(season_id);
    }
    if let Some(status) = filter.status {
        push_filter_separator(builder, &mut has_where);
        builder.push("status = ");
        builder.push_bind(status.as_str());
    }
    if let Some(created_after) = filter.created_after {
        push_filter_separator(builder, &mut has_where);
        builder.push("created_at >= ");
        builder.push_bind(created_after);
    }
    if let Some(created_before) = filter.created_before {
        push_filter_separator(builder, &mut has_where);
        builder.push("created_at <= ");
        builder.push_bind(created_before);
    }
}

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
                version INTEGER NOT NULL DEFAULT 1,
                field_id TEXT NOT NULL DEFAULT 'unassigned',
                season_id TEXT NOT NULL DEFAULT 'unassigned',
                session_id TEXT,
                owner_id TEXT NOT NULL DEFAULT 'unassigned',
                status TEXT NOT NULL DEFAULT 'Draft',
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

        for statement in [
            "ALTER TABLE missions ADD COLUMN IF NOT EXISTS field_id TEXT NOT NULL DEFAULT 'unassigned';",
            "ALTER TABLE missions ADD COLUMN IF NOT EXISTS season_id TEXT NOT NULL DEFAULT 'unassigned';",
            "ALTER TABLE missions ADD COLUMN IF NOT EXISTS session_id TEXT;",
            "ALTER TABLE missions ADD COLUMN IF NOT EXISTS owner_id TEXT NOT NULL DEFAULT 'unassigned';",
            "ALTER TABLE missions ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'Draft';",
            "ALTER TABLE missions ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 1;",
        ] {
            sqlx::query(statement).execute(&self.pool).await?;
        }

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS mission_revisions (
                mission_id UUID NOT NULL REFERENCES missions(id) ON DELETE CASCADE,
                version INTEGER NOT NULL,
                archived_at TIMESTAMPTZ NOT NULL,
                mission_snapshot JSONB NOT NULL,
                PRIMARY KEY (mission_id, version)
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
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_waypoints_mission_id ON waypoints(mission_id);",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_flight_paths_mission_id ON flight_paths(mission_id);",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_missions_created_at ON missions(created_at);")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_missions_field_id ON missions(field_id);")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_missions_season_id ON missions(season_id);")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_missions_status ON missions(status);")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_mission_revisions_mission_version ON mission_revisions(mission_id, version);",
        )
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
                version, field_id, season_id, session_id, owner_id, status,
                area_of_interest, estimated_duration_minutes, 
                estimated_battery_usage, weather_constraints, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
        )
        .bind(mission.id)
        .bind(&mission.name)
        .bind(&mission.description)
        .bind(mission.created_at)
        .bind(mission.updated_at)
        .bind(mission.version.max(1) as i32)
        .bind(&mission.field_id)
        .bind(&mission.season_id)
        .bind(&mission.session_id)
        .bind(&mission.owner_id)
        .bind(mission.status.as_str())
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

    async fn hydrate_mission_row(&self, row: PgRow) -> Result<Mission> {
        let mission_id: Uuid = row.get("id");

        let waypoint_rows =
            sqlx::query("SELECT * FROM waypoints WHERE mission_id = $1 ORDER BY sequence_order")
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

        let flight_path_rows = sqlx::query("SELECT * FROM flight_paths WHERE mission_id = $1")
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
                    estimated_duration_seconds: row.get::<i32, _>("estimated_duration_seconds")
                        as u32,
                    path_type: serde_json::from_value(row.get("path_type"))?,
                })
            })
            .collect();

        mission_from_row(&row, waypoints?, flight_paths?)
    }

    /// Get a mission by ID
    pub async fn get_mission(&self, id: &Uuid) -> Result<Option<Mission>> {
        let mission_row = sqlx::query("SELECT * FROM missions WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        match mission_row {
            Some(row) => Ok(Some(self.hydrate_mission_row(row).await?)),
            None => Ok(None),
        }
    }

    /// Update a mission
    pub async fn update_mission(&self, mission: &Mission) -> Result<Mission> {
        let existing = self
            .get_mission(&mission.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Mission not found"))?;
        let mut updated = mission.clone();
        updated.version = existing.version;
        updated.bump_version();

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO mission_revisions (
                mission_id, version, archived_at, mission_snapshot
            ) VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(existing.id)
        .bind(existing.version as i32)
        .bind(Utc::now())
        .bind(serde_json::to_value(&existing)?)
        .execute(&mut *tx)
        .await?;

        let rows_affected = sqlx::query(
            r#"
            UPDATE missions SET 
                name = $2, description = $3, updated_at = $4, version = $5,
                field_id = $6, season_id = $7, session_id = $8, owner_id = $9, status = $10,
                area_of_interest = $11, estimated_duration_minutes = $12,
                estimated_battery_usage = $13, weather_constraints = $14, metadata = $15
            WHERE id = $1
            "#,
        )
        .bind(updated.id)
        .bind(&updated.name)
        .bind(&updated.description)
        .bind(updated.updated_at)
        .bind(updated.version as i32)
        .bind(&updated.field_id)
        .bind(&updated.season_id)
        .bind(&updated.session_id)
        .bind(&updated.owner_id)
        .bind(updated.status.as_str())
        .bind(serde_json::to_value(&updated.area_of_interest)?)
        .bind(updated.estimated_duration_minutes as i32)
        .bind(updated.estimated_battery_usage)
        .bind(serde_json::to_value(&updated.weather_constraints)?)
        .bind(serde_json::to_value(&updated.metadata)?)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(anyhow::anyhow!("Mission not found"));
        }

        // Delete existing waypoints and flight paths
        sqlx::query("DELETE FROM waypoints WHERE mission_id = $1")
            .bind(updated.id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM flight_paths WHERE mission_id = $1")
            .bind(updated.id)
            .execute(&mut *tx)
            .await?;

        // Insert updated waypoints
        for (index, waypoint) in updated.waypoints.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO waypoints (
                    id, mission_id, position, altitude_m, waypoint_type,
                    actions, arrival_time, speed_ms, heading_degrees, sequence_order
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(waypoint.id)
            .bind(updated.id)
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
        for flight_path in &updated.flight_paths {
            sqlx::query(
                r#"
                INSERT INTO flight_paths (
                    id, mission_id, name, segments, 
                    total_distance_m, estimated_duration_seconds, path_type
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(flight_path.id)
            .bind(updated.id)
            .bind(&flight_path.name)
            .bind(serde_json::to_value(&flight_path.segments)?)
            .bind(flight_path.total_distance_m)
            .bind(flight_path.estimated_duration_seconds as i32)
            .bind(serde_json::to_value(&flight_path.path_type)?)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(updated)
    }

    /// List all missions with optional filtering
    pub async fn list_missions(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<Mission>> {
        Ok(self
            .list_missions_page(MissionListFilter {
                limit,
                offset,
                ..MissionListFilter::default()
            })
            .await?
            .missions)
    }

    /// List missions with pagination and optional field, season, status, and creation-date filters.
    pub async fn list_missions_page(&self, filter: MissionListFilter) -> Result<MissionListPage> {
        let limit = mission_page_limit(filter.limit);
        let offset = mission_page_offset(filter.offset);

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) AS total FROM missions");
        append_mission_filters(&mut count_builder, &filter);
        let count_row = count_builder.build().fetch_one(&self.pool).await?;
        let total = count_row.get::<i64, _>("total") as usize;

        let mut row_builder: QueryBuilder<Postgres> = QueryBuilder::new("SELECT * FROM missions");
        append_mission_filters(&mut row_builder, &filter);
        row_builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        row_builder.push_bind(limit);
        row_builder.push(" OFFSET ");
        row_builder.push_bind(offset);
        let mission_rows = row_builder.build().fetch_all(&self.pool).await?;

        let mut missions = Vec::with_capacity(mission_rows.len());
        for row in mission_rows {
            missions.push(self.hydrate_mission_row(row).await?);
        }

        Ok(MissionListPage {
            missions,
            total,
            limit,
            offset,
        })
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
            "#,
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;

        let mut missions = Vec::new();

        for row in mission_rows {
            missions.push(self.hydrate_mission_row(row).await?);
        }

        Ok(missions)
    }

    /// Read retained prior mission revisions.
    pub async fn get_mission_history(&self, id: &Uuid) -> Result<Vec<MissionRevision>> {
        let revision_rows = sqlx::query(
            r#"
            SELECT mission_id, version, archived_at, mission_snapshot
            FROM mission_revisions
            WHERE mission_id = $1
            ORDER BY version ASC
            "#,
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;

        let mut revisions = Vec::with_capacity(revision_rows.len());
        for row in revision_rows {
            let mission: Mission = serde_json::from_value(row.get("mission_snapshot"))?;
            revisions.push(MissionRevision {
                mission_id: row.get("mission_id"),
                version: row.get::<i32, _>("version").max(1) as u32,
                archived_at: row.get("archived_at"),
                mission,
            });
        }

        Ok(revisions)
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
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(MissionStats {
            total_missions: row.get::<i64, _>("total_missions") as u64,
            average_duration_minutes: row.get::<Option<f64>, _>("avg_duration").unwrap_or(0.0)
                as f32,
            average_battery_usage: row
                .get::<Option<f64>, _>("avg_battery_usage")
                .unwrap_or(0.0) as f32,
            oldest_mission: row.get("oldest_mission"),
            newest_mission: row.get("newest_mission"),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        let database_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:password@localhost:5432/agbot_test".to_string()
        });

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
