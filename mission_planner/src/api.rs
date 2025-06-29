use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::{Mission, MissionPlannerService, MissionStats, Waypoint};

/// REST API for mission planning
pub struct MissionApi {
    service: Arc<MissionPlannerService>,
}

impl MissionApi {
    pub fn new(service: Arc<MissionPlannerService>) -> Self {
        Self { service }
    }

    /// Create router with all mission endpoints
    pub fn router(service: Arc<MissionPlannerService>) -> Router {
        let api = Self::new(service.clone());
        
        Router::new()
            .route("/missions", post(create_mission))
            .route("/missions", get(list_missions))
            .route("/missions/search", get(search_missions))
            .route("/missions/stats", get(get_mission_stats))
            .route("/missions/:id", get(get_mission))
            .route("/missions/:id", put(update_mission))
            .route("/missions/:id", delete(delete_mission))
            .route("/missions/:id/optimize", post(optimize_mission))
            .with_state(service)
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateMissionRequest {
    pub name: String,
    pub description: String,
    pub area_of_interest: geo::Polygon<f64>,
    pub waypoints: Option<Vec<Waypoint>>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMissionRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub area_of_interest: Option<geo::Polygon<f64>>,
    pub waypoints: Option<Vec<Waypoint>>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct ListMissionsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Debug, Serialize)]
pub struct MissionResponse {
    pub mission: Mission,
}

#[derive(Debug, Serialize)]
pub struct MissionListResponse {
    pub missions: Vec<Mission>,
    pub total: usize,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CreateMissionResponse {
    pub id: Uuid,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

/// Create a new mission
async fn create_mission(
    State(service): State<Arc<MissionPlannerService>>,
    Json(request): Json<CreateMissionRequest>,
) -> Result<Json<CreateMissionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut mission = Mission::new(
        request.name,
        request.description,
        request.area_of_interest,
    );

    // Add waypoints if provided
    if let Some(waypoints) = request.waypoints {
        for waypoint in waypoints {
            mission.add_waypoint(waypoint);
        }
    }

    // Add metadata if provided
    if let Some(metadata) = request.metadata {
        mission.metadata = metadata;
    }

    match service.create_mission(mission).await {
        Ok(id) => Ok(Json(CreateMissionResponse {
            id,
            message: "Mission created successfully".to_string(),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "CREATE_FAILED".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// Get a mission by ID
async fn get_mission(
    State(service): State<Arc<MissionPlannerService>>,
    Path(id): Path<Uuid>,
) -> Result<Json<MissionResponse>, (StatusCode, Json<ErrorResponse>)> {
    match service.get_mission(&id).await {
        Ok(Some(mission)) => Ok(Json(MissionResponse { mission })),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "NOT_FOUND".to_string(),
                message: "Mission not found".to_string(),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "GET_FAILED".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// Update a mission
async fn update_mission(
    State(service): State<Arc<MissionPlannerService>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateMissionRequest>,
) -> Result<Json<MissionResponse>, (StatusCode, Json<ErrorResponse>)> {
    // First get the existing mission
    let mut mission = match service.get_mission(&id).await {
        Ok(Some(mission)) => mission,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "NOT_FOUND".to_string(),
                    message: "Mission not found".to_string(),
                }),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "GET_FAILED".to_string(),
                    message: e.to_string(),
                }),
            ));
        }
    };

    // Update fields if provided
    if let Some(name) = request.name {
        mission.name = name;
    }
    if let Some(description) = request.description {
        mission.description = description;
    }
    if let Some(area) = request.area_of_interest {
        mission.area_of_interest = area;
    }
    if let Some(waypoints) = request.waypoints {
        mission.waypoints = waypoints;
    }
    if let Some(metadata) = request.metadata {
        mission.metadata = metadata;
    }

    mission.updated_at = Utc::now();

    match service.update_mission(mission.clone()).await {
        Ok(_) => Ok(Json(MissionResponse { mission })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "UPDATE_FAILED".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// List missions with pagination
async fn list_missions(
    State(service): State<Arc<MissionPlannerService>>,
    Query(query): Query<ListMissionsQuery>,
) -> Result<Json<MissionListResponse>, (StatusCode, Json<ErrorResponse>)> {
    match service.list_missions(query.limit, query.offset).await {
        Ok(missions) => {
            let total = missions.len();
            Ok(Json(MissionListResponse {
                missions,
                total,
                limit: query.limit,
                offset: query.offset,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "LIST_FAILED".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// Search missions
async fn search_missions(
    State(service): State<Arc<MissionPlannerService>>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<MissionListResponse>, (StatusCode, Json<ErrorResponse>)> {
    match service.search_missions(&query.q).await {
        Ok(missions) => {
            let total = missions.len();
            Ok(Json(MissionListResponse {
                missions,
                total,
                limit: None,
                offset: None,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "SEARCH_FAILED".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// Delete a mission
async fn delete_mission(
    State(service): State<Arc<MissionPlannerService>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match service.delete_mission(&id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            if e.to_string().contains("not found") {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "NOT_FOUND".to_string(),
                        message: "Mission not found".to_string(),
                    }),
                ))
            } else {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "DELETE_FAILED".to_string(),
                        message: e.to_string(),
                    }),
                ))
            }
        }
    }
}

/// Optimize a mission
async fn optimize_mission(
    State(service): State<Arc<MissionPlannerService>>,
    Path(id): Path<Uuid>,
) -> Result<Json<MissionResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Get the existing mission
    let mut mission = match service.get_mission(&id).await {
        Ok(Some(mission)) => mission,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "NOT_FOUND".to_string(),
                    message: "Mission not found".to_string(),
                }),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "GET_FAILED".to_string(),
                    message: e.to_string(),
                }),
            ));
        }
    };

    // Optimize the mission
    if let Err(e) = mission.optimize() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "OPTIMIZE_FAILED".to_string(),
                message: e.to_string(),
            }),
        ));
    }

    // Update the mission in the database
    match service.update_mission(mission.clone()).await {
        Ok(_) => Ok(Json(MissionResponse { mission })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "UPDATE_FAILED".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// Get mission statistics
async fn get_mission_stats(
    State(service): State<Arc<MissionPlannerService>>,
) -> Result<Json<MissionStats>, (StatusCode, Json<ErrorResponse>)> {
    match service.get_mission_stats().await {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "STATS_FAILED".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Method;
    use axum_test::TestServer;
    use geo::{coord, polygon};

    async fn setup_test_service() -> Arc<MissionPlannerService> {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/agbot_test".to_string());
        
        let service = MissionPlannerService::new(&database_url).await.unwrap();
        Arc::new(service)
    }

    #[tokio::test]
    #[ignore] // Requires PostgreSQL database
    async fn test_mission_api() {
        let service = setup_test_service().await;
        let app = MissionApi::router(service);
        let server = TestServer::new(app).unwrap();

        // Test create mission
        let area = polygon![
            (x: 0.0, y: 0.0),
            (x: 1.0, y: 0.0),
            (x: 1.0, y: 1.0),
            (x: 0.0, y: 1.0),
            (x: 0.0, y: 0.0),
        ];

        let create_request = CreateMissionRequest {
            name: "Test API Mission".to_string(),
            description: "A test mission via API".to_string(),
            area_of_interest: area,
            waypoints: None,
            metadata: None,
        };

        let response = server
            .post("/missions")
            .json(&create_request)
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let create_response: CreateMissionResponse = response.json();
        let mission_id = create_response.id;

        // Test get mission
        let response = server
            .get(&format!("/missions/{}", mission_id))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let mission_response: MissionResponse = response.json();
        assert_eq!(mission_response.mission.name, "Test API Mission");

        // Test list missions
        let response = server.get("/missions").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        // Test delete mission
        let response = server
            .method(Method::DELETE)
            .uri(&format!("/missions/{}", mission_id))
            .await;

        assert_eq!(response.status_code(), StatusCode::NO_CONTENT);
    }
}
