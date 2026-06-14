use anyhow::Result;
use axum::http::StatusCode;
use axum_test::TestServer;
use geo::{point, polygon};
use mission_planner::{
    api::{CreateMissionRequest, UpdateMissionRequest},
    DatabaseService, Mission, MissionApi, MissionListFilter, MissionPlannerService, MissionStatus,
    Waypoint, WaypointType,
};
use std::{collections::HashMap, env, sync::Arc};

const DEFAULT_TEST_DATABASE_URL: &str = "postgres://postgres:password@localhost:5432/agbot_test";

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL and a running PostgreSQL instance"]
async fn db_service_crud_search_and_stats_roundtrip() -> Result<()> {
    let db_url = test_database_url();
    let service = MissionPlannerService::new(&db_url).await?;

    let run_id = uuid::Uuid::new_v4().to_string();
    let field_id = format!("field-{run_id}");
    let season_id = format!("season-{run_id}");
    let mut mission = sample_mission("Coverage Alpha", "Initial mission");
    mission.field_id = field_id.clone();
    mission.season_id = season_id.clone();
    mission.estimated_duration_minutes = 18;
    mission.estimated_battery_usage = 0.42;
    let id = service.create_mission(mission.clone()).await?;

    let mut matching_mission = sample_mission("Coverage Gamma", "Second matching mission");
    matching_mission.field_id = field_id.clone();
    matching_mission.season_id = season_id.clone();
    let matching_id = service.create_mission(matching_mission).await?;

    let mut other_mission = sample_mission("Coverage Delta", "Different field mission");
    other_mission.field_id = format!("other-field-{run_id}");
    other_mission.season_id = season_id.clone();
    let other_id = service.create_mission(other_mission).await?;

    let fetched = service
        .get_mission(&id)
        .await?
        .expect("mission should exist");
    assert_eq!(fetched.name, "Coverage Alpha");
    assert_eq!(fetched.waypoints.len(), 2);
    assert_eq!(fetched.metadata.get("owner"), Some(&"qa".to_string()));

    mission.name = "Coverage Beta".to_string();
    mission.description = "Updated mission".to_string();
    mission.updated_at = chrono::Utc::now();
    mission
        .metadata
        .insert("priority".to_string(), "high".to_string());
    service.update_mission(mission.clone()).await?;

    let updated = service
        .get_mission(&mission.id)
        .await?
        .expect("updated mission should exist");
    assert_eq!(updated.name, "Coverage Beta");
    assert_eq!(updated.description, "Updated mission");
    assert_eq!(updated.metadata.get("priority"), Some(&"high".to_string()));
    assert_eq!(updated.version, 2);

    let listed = service
        .list_missions_page(MissionListFilter {
            limit: Some(1),
            offset: Some(0),
            field_id: Some(field_id.clone()),
            season_id: Some(season_id.clone()),
            status: Some(MissionStatus::Draft),
            ..MissionListFilter::default()
        })
        .await?;
    assert_eq!(listed.total, 2);
    assert_eq!(listed.missions.len(), 1);

    let second_page = service
        .list_missions_page(MissionListFilter {
            limit: Some(1),
            offset: Some(1),
            field_id: Some(field_id.clone()),
            season_id: Some(season_id.clone()),
            status: Some(MissionStatus::Draft),
            created_after: Some(chrono::Utc::now() - chrono::Duration::minutes(5)),
            created_before: Some(chrono::Utc::now() + chrono::Duration::minutes(5)),
        })
        .await?;
    assert_eq!(second_page.total, 2);
    assert_eq!(second_page.missions.len(), 1);
    let paged_ids = listed
        .missions
        .iter()
        .chain(second_page.missions.iter())
        .map(|mission| mission.id)
        .collect::<Vec<_>>();
    assert!(paged_ids.contains(&id));
    assert!(paged_ids.contains(&matching_id));
    assert!(!paged_ids.contains(&other_id));

    mission.description = "Second update".to_string();
    mission.updated_at = chrono::Utc::now();
    service.update_mission(mission.clone()).await?;
    let history = service.get_mission_history(&id).await?;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].version, 1);
    assert_eq!(history[0].mission.name, "Coverage Alpha");
    assert_eq!(history[1].version, 2);
    assert_eq!(history[1].mission.name, "Coverage Beta");

    let found = service.search_missions("beta").await?;
    assert!(found.iter().any(|m| m.id == id));

    let stats = service.get_mission_stats().await?;
    assert!(stats.total_missions >= 1);
    assert!(stats.average_duration_minutes > 0.0);
    assert!(stats.average_battery_usage >= 0.0);

    service.delete_mission(&matching_id).await?;
    service.delete_mission(&other_id).await?;
    service.delete_mission(&id).await?;
    assert!(service.get_mission(&id).await?.is_none());

    Ok(())
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL and a running PostgreSQL instance"]
async fn api_crud_search_stats_flow() -> Result<()> {
    let db_url = test_database_url();
    let db = DatabaseService::connect(&db_url).await?;
    db.initialize().await?;
    let service = Arc::new(MissionPlannerService::with_database(db));
    let server = TestServer::new(MissionApi::router(service))?;

    let run_id = uuid::Uuid::new_v4().to_string();
    let field_id = format!("field-api-{run_id}");
    let season_id = format!("season-api-{run_id}");
    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), "integration-test".to_string());
    let create_request = CreateMissionRequest {
        name: "API Mission".to_string(),
        description: "Created via API".to_string(),
        area_of_interest: polygon![
            (x: 0.0, y: 0.0),
            (x: 0.2, y: 0.0),
            (x: 0.2, y: 0.2),
            (x: 0.0, y: 0.2),
            (x: 0.0, y: 0.0),
        ],
        field_id: Some(field_id.clone()),
        season_id: Some(season_id.clone()),
        session_id: Some("session-integration".to_string()),
        owner_id: Some("owner-integration".to_string()),
        waypoints: Some(vec![
            Waypoint::new(point!(x: 0.0, y: 0.0), 100.0, WaypointType::Takeoff),
            Waypoint::new(point!(x: 0.1, y: 0.1), 120.0, WaypointType::Navigation),
        ]),
        metadata: Some(metadata),
    };

    let create_resp = server.post("/missions").json(&create_request).await;
    assert_eq!(create_resp.status_code(), StatusCode::OK);
    let create_json: serde_json::Value = create_resp.json();
    let mission_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing id in create response"))?
        .to_string();

    let get_resp = server.get(&format!("/missions/{mission_id}")).await;
    assert_eq!(get_resp.status_code(), StatusCode::OK);
    let get_json: serde_json::Value = get_resp.json();
    assert_eq!(
        get_json.pointer("/mission/name").and_then(|v| v.as_str()),
        Some("API Mission")
    );

    let update_request = UpdateMissionRequest {
        name: Some("API Mission Updated".to_string()),
        description: Some("Updated via API".to_string()),
        area_of_interest: None,
        field_id: None,
        season_id: None,
        session_id: None,
        owner_id: None,
        waypoints: None,
        metadata: Some(HashMap::from([(
            "source".to_string(),
            "api-update".to_string(),
        )])),
    };
    let update_resp = server
        .put(&format!("/missions/{mission_id}"))
        .json(&update_request)
        .await;
    assert_eq!(update_resp.status_code(), StatusCode::OK);
    let update_json: serde_json::Value = update_resp.json();
    assert_eq!(
        update_json
            .pointer("/mission/name")
            .and_then(|v| v.as_str()),
        Some("API Mission Updated")
    );
    assert_eq!(
        update_json
            .pointer("/mission/version")
            .and_then(|v| v.as_u64()),
        Some(2)
    );

    let second_update_request = UpdateMissionRequest {
        name: None,
        description: Some("Updated twice via API".to_string()),
        area_of_interest: None,
        field_id: None,
        season_id: None,
        session_id: None,
        owner_id: None,
        waypoints: None,
        metadata: None,
    };
    let second_update_resp = server
        .put(&format!("/missions/{mission_id}"))
        .json(&second_update_request)
        .await;
    assert_eq!(second_update_resp.status_code(), StatusCode::OK);
    let second_update_json: serde_json::Value = second_update_resp.json();
    assert_eq!(
        second_update_json
            .pointer("/mission/version")
            .and_then(|v| v.as_u64()),
        Some(3)
    );

    let list_resp = server.get("/missions?limit=10&offset=0").await;
    assert_eq!(list_resp.status_code(), StatusCode::OK);
    let list_json: serde_json::Value = list_resp.json();
    let listed = list_json
        .get("missions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("missions array missing in list response"))?;
    assert!(listed.iter().any(|m| {
        m.get("id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| id == mission_id)
    }));

    let created_after = (chrono::Utc::now() - chrono::Duration::minutes(5))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let created_before = (chrono::Utc::now() + chrono::Duration::minutes(5))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let filtered_resp = server
        .get(&format!(
            "/missions?field_id={field_id}&season_id={season_id}&status=Draft&created_after={created_after}&created_before={created_before}&limit=10&offset=0"
        ))
        .await;
    assert_eq!(filtered_resp.status_code(), StatusCode::OK);
    let filtered_json: serde_json::Value = filtered_resp.json();
    assert!(filtered_json
        .get("missions")
        .and_then(|v| v.as_array())
        .unwrap_or(&Vec::new())
        .iter()
        .any(|m| m
            .get("id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| id == mission_id)));

    let history_resp = server.get(&format!("/missions/{mission_id}/history")).await;
    assert_eq!(history_resp.status_code(), StatusCode::OK);
    let history_json: serde_json::Value = history_resp.json();
    let revisions = history_json
        .get("revisions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("revisions array missing in history response"))?;
    assert_eq!(revisions.len(), 2);
    assert_eq!(
        revisions[0].get("version").and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        revisions[1].get("version").and_then(|v| v.as_u64()),
        Some(2)
    );

    let search_resp = server.get("/missions/search?q=updated").await;
    assert_eq!(search_resp.status_code(), StatusCode::OK);
    let search_json: serde_json::Value = search_resp.json();
    let searched = search_json
        .get("missions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("missions array missing in search response"))?;
    assert!(searched.iter().any(|m| {
        m.get("id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| id == mission_id)
    }));

    let stats_resp = server.get("/missions/stats").await;
    assert_eq!(stats_resp.status_code(), StatusCode::OK);
    let stats_json: serde_json::Value = stats_resp.json();
    assert!(
        stats_json
            .get("total_missions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            >= 1
    );

    let delete_resp = server.delete(&format!("/missions/{mission_id}")).await;
    assert_eq!(delete_resp.status_code(), StatusCode::NO_CONTENT);

    let missing_resp = server.get(&format!("/missions/{mission_id}")).await;
    assert_eq!(missing_resp.status_code(), StatusCode::NOT_FOUND);

    Ok(())
}

fn sample_mission(name: &str, description: &str) -> Mission {
    let mut mission = Mission::new(
        name.to_string(),
        description.to_string(),
        polygon![
            (x: 0.0, y: 0.0),
            (x: 1.0, y: 0.0),
            (x: 1.0, y: 1.0),
            (x: 0.0, y: 1.0),
            (x: 0.0, y: 0.0),
        ],
    );
    mission.add_waypoint(Waypoint::new(
        point!(x: 0.0, y: 0.0),
        100.0,
        WaypointType::Takeoff,
    ));
    mission.add_waypoint(Waypoint::new(
        point!(x: 0.5, y: 0.5),
        120.0,
        WaypointType::Survey,
    ));
    mission
        .metadata
        .insert("owner".to_string(), "qa".to_string());
    mission
}

fn test_database_url() -> String {
    env::var("TEST_DATABASE_URL").unwrap_or_else(|_| DEFAULT_TEST_DATABASE_URL.to_string())
}
