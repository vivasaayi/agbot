use crate::state::{
    RecommendationCreateTask, RecommendationDeleteTask, RecommendationFetchTask,
    RecommendationOverlayState, RecommendationUpdateTask, TileConfig, TileRenderState, TileStatus,
};
use anyhow::{Context, Result};
use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use futures_lite::future;
use serde::Serialize;
use shared::schemas::{RecommendationPriority, RecommendationRecord, RecommendationStatus};

pub struct ViewerRecommendationsPlugin;

impl Plugin for ViewerRecommendationsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                poll_recommendation_fetch,
                poll_recommendation_create,
                poll_recommendation_update,
                poll_recommendation_delete,
            ),
        );
    }
}

pub fn poll_recommendation_fetch(
    mut recommendation_fetch_task: ResMut<RecommendationFetchTask>,
    mut recommendations: ResMut<RecommendationOverlayState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = recommendation_fetch_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(items) => recommendations.items = items,
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            recommendation_fetch_task.0 = Some(task);
        }
    }
}

pub fn poll_recommendation_create(
    mut recommendation_create_task: ResMut<RecommendationCreateTask>,
    mut recommendations: ResMut<RecommendationOverlayState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = recommendation_create_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(recommendation) => {
                    recommendations.items.push(recommendation.clone());
                    recommendations
                        .items
                        .sort_by(|left, right| right.created_at.cmp(&left.created_at));
                    recommendations.selected_recommendation_id =
                        Some(recommendation.recommendation_id);
                    tile_state.status = TileStatus::Ready;
                }
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            recommendation_create_task.0 = Some(task);
        }
    }
}

pub fn poll_recommendation_update(
    mut recommendation_update_task: ResMut<RecommendationUpdateTask>,
    mut recommendations: ResMut<RecommendationOverlayState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = recommendation_update_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(updated) => {
                    if let Some(existing) =
                        recommendations.items.iter_mut().find(|recommendation| {
                            recommendation.recommendation_id == updated.recommendation_id
                        })
                    {
                        *existing = updated.clone();
                    } else {
                        recommendations.items.push(updated.clone());
                    }
                    recommendations.selected_recommendation_id = Some(updated.recommendation_id);
                    tile_state.status = TileStatus::Ready;
                }
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            recommendation_update_task.0 = Some(task);
        }
    }
}

pub fn poll_recommendation_delete(
    mut recommendation_delete_task: ResMut<RecommendationDeleteTask>,
    mut recommendations: ResMut<RecommendationOverlayState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = recommendation_delete_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(recommendation_id) => {
                    recommendations.items.retain(|recommendation| {
                        recommendation.recommendation_id != recommendation_id
                    });
                    if recommendations.selected_recommendation_id.as_deref()
                        == Some(recommendation_id.as_str())
                    {
                        recommendations.selected_recommendation_id = None;
                        clear_recommendation_draft(recommendations.as_mut());
                    }
                    tile_state.status = TileStatus::Ready;
                }
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            recommendation_delete_task.0 = Some(task);
        }
    }
}

pub fn selected_recommendation<'a>(
    recommendations: &'a RecommendationOverlayState,
) -> Option<&'a RecommendationRecord> {
    recommendations
        .selected_recommendation_id
        .as_ref()
        .and_then(|recommendation_id| {
            recommendations
                .items
                .iter()
                .find(|recommendation| recommendation.recommendation_id == *recommendation_id)
        })
}

pub fn recommendation_matches_filters(
    recommendation: &RecommendationRecord,
    recommendations: &RecommendationOverlayState,
) -> bool {
    if let Some(status_filter) = recommendations.status_filter {
        if recommendation.status != status_filter {
            return false;
        }
    }
    if let Some(priority_filter) = recommendations.priority_filter {
        if recommendation.priority != priority_filter {
            return false;
        }
    }
    true
}

pub fn load_recommendation_into_draft(
    recommendations: &mut RecommendationOverlayState,
    recommendation: &RecommendationRecord,
) {
    recommendations.draft_title = recommendation.title.clone();
    recommendations.draft_note = recommendation.note.clone().unwrap_or_default();
    recommendations.draft_category = recommendation.category.clone().unwrap_or_default();
    recommendations.draft_priority = recommendation.priority;
    recommendations.draft_status = recommendation.status;
    recommendations.linked_annotation_ids = recommendation.annotation_ids.clone();
}

pub fn seed_recommendation_from_annotation(
    recommendations: &mut RecommendationOverlayState,
    annotation_id: &str,
    annotation_label: &str,
) {
    if recommendations.draft_title.trim().is_empty() {
        recommendations.draft_title = format!("Follow up: {}", annotation_label);
    }
    recommendations.linked_annotation_ids = vec![annotation_id.to_string()];
    recommendations.draft_status = RecommendationStatus::Open;
}

pub fn clear_recommendation_draft(recommendations: &mut RecommendationOverlayState) {
    recommendations.draft_title.clear();
    recommendations.draft_note.clear();
    recommendations.draft_category.clear();
    recommendations.draft_priority = RecommendationPriority::Medium;
    recommendations.draft_status = RecommendationStatus::Open;
    recommendations.linked_annotation_ids.clear();
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecommendationCreatePayload {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub priority: RecommendationPriority,
    pub status: RecommendationStatus,
    pub annotation_ids: Vec<String>,
}

pub fn build_recommendation_create_payload(
    recommendations: &RecommendationOverlayState,
) -> Result<RecommendationCreatePayload> {
    let title = normalize_required_text(&recommendations.draft_title, "recommendation title")?;
    let annotation_ids = normalized_annotation_ids(&recommendations.linked_annotation_ids)?;

    Ok(RecommendationCreatePayload {
        title,
        note: normalize_optional_text(&recommendations.draft_note),
        category: normalize_optional_text(&recommendations.draft_category),
        priority: recommendations.draft_priority,
        status: recommendations.draft_status,
        annotation_ids,
    })
}

fn normalized_annotation_ids(annotation_ids: &[String]) -> Result<Vec<String>> {
    let normalized = annotation_ids
        .iter()
        .filter_map(|annotation_id| normalize_optional_text(annotation_id))
        .collect::<Vec<_>>();

    if normalized.is_empty() {
        anyhow::bail!("recommendation requires at least one linked annotation");
    }

    Ok(normalized)
}

fn normalize_required_text(value: &str, label: &str) -> Result<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        anyhow::bail!("{} is required", label);
    }
    Ok(normalized.to_string())
}

fn normalize_optional_text(value: &str) -> Option<String> {
    let normalized = value.trim();
    (!normalized.is_empty()).then(|| normalized.to_string())
}

pub fn start_recommendation_fetch(
    recommendation_fetch_task: &mut RecommendationFetchTask,
    config: &TileConfig,
) -> Result<()> {
    let scene_id = match &config.scene_id {
        Some(id) => id.clone(),
        None => {
            recommendation_fetch_task.0 = None;
            return Ok(());
        }
    };

    let url = format!(
        "{}/api/scenes/{}/recommendations",
        config.base_url, scene_id
    );
    recommendation_fetch_task.0 = Some(IoTaskPool::get().spawn(async move {
        let response =
            reqwest::blocking::get(&url).with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response
            .bytes()
            .context("failed to read recommendations response body")?;
        let recommendations = serde_json::from_slice::<Vec<RecommendationRecord>>(&bytes)
            .context("failed to decode recommendations")?;
        Ok(recommendations)
    }));

    Ok(())
}

pub fn start_recommendation_create(
    recommendation_create_task: &mut RecommendationCreateTask,
    config: &TileConfig,
    payload: RecommendationCreatePayload,
) -> Result<()> {
    let scene_id = crate::state::ensure_scene_id(config, "create recommendations")?;
    let url = format!(
        "{}/api/scenes/{}/recommendations",
        config.base_url, scene_id
    );
    let payload = serde_json::to_string(&payload)
        .context("failed to encode recommendation create payload")?;

    recommendation_create_task.0 = Some(IoTaskPool::get().spawn(async move {
        let client = reqwest::blocking::Client::new();
        let response = client
            .post(&url)
            .header("content-type", "application/json")
            .body(payload)
            .send()
            .with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response
            .bytes()
            .context("failed to read create recommendation response body")?;
        let recommendation = serde_json::from_slice::<RecommendationRecord>(&bytes)
            .context("failed to decode created recommendation")?;
        Ok(recommendation)
    }));

    Ok(())
}

pub fn start_recommendation_update(
    recommendation_update_task: &mut RecommendationUpdateTask,
    config: &TileConfig,
    recommendation_id: &str,
    title: String,
    note: String,
    category: String,
    priority: RecommendationPriority,
    status: RecommendationStatus,
    annotation_ids: Vec<String>,
) -> Result<()> {
    let scene_id = crate::state::ensure_scene_id(config, "update recommendations")?;
    let url = format!(
        "{}/api/scenes/{}/recommendations/{}",
        config.base_url, scene_id, recommendation_id
    );
    let payload = serde_json::json!({
        "title": title,
        "note": note,
        "category": category,
        "priority": priority,
        "status": status,
        "annotation_ids": annotation_ids
    })
    .to_string();

    recommendation_update_task.0 = Some(IoTaskPool::get().spawn(async move {
        let client = reqwest::blocking::Client::new();
        let response = client
            .put(&url)
            .header("content-type", "application/json")
            .body(payload)
            .send()
            .with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response
            .bytes()
            .context("failed to read update recommendation response body")?;
        let recommendation = serde_json::from_slice::<RecommendationRecord>(&bytes)
            .context("failed to decode updated recommendation")?;
        Ok(recommendation)
    }));

    Ok(())
}

pub fn start_recommendation_delete(
    recommendation_delete_task: &mut RecommendationDeleteTask,
    config: &TileConfig,
    recommendation_id: &str,
) -> Result<()> {
    let scene_id = crate::state::ensure_scene_id(config, "delete recommendations")?;
    let url = format!(
        "{}/api/scenes/{}/recommendations/{}",
        config.base_url, scene_id, recommendation_id
    );
    let recommendation_id = recommendation_id.to_string();

    recommendation_delete_task.0 = Some(IoTaskPool::get().spawn(async move {
        let client = reqwest::blocking::Client::new();
        let response = client
            .delete(&url)
            .send()
            .with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        Ok(recommendation_id)
    }));

    Ok(())
}

pub fn clear_recommendations(
    recommendations: &mut RecommendationOverlayState,
    recommendation_fetch_task: &mut RecommendationFetchTask,
    recommendation_create_task: &mut RecommendationCreateTask,
    recommendation_update_task: &mut RecommendationUpdateTask,
    recommendation_delete_task: &mut RecommendationDeleteTask,
) {
    recommendations.items.clear();
    recommendations.selected_recommendation_id = None;
    clear_recommendation_draft(recommendations);
    recommendation_fetch_task.0 = None;
    recommendation_create_task.0 = None;
    recommendation_update_task.0 = None;
    recommendation_delete_task.0 = None;
}

#[cfg(test)]
mod tests {
    use super::{
        build_recommendation_create_payload, recommendation_matches_filters,
        RecommendationOverlayState,
    };
    use shared::schemas::{RecommendationPriority, RecommendationRecord, RecommendationStatus};

    fn sample_recommendation() -> RecommendationRecord {
        RecommendationRecord {
            recommendation_id: "rec-1".to_string(),
            scene_id: "scene-1".to_string(),
            field_id: Some("field-1".to_string()),
            org_id: "org-a".to_string(),
            author_user_id: "user-author".to_string(),
            title: "Scout stress zone".to_string(),
            note: Some("Inspect irrigation line".to_string()),
            category: Some("irrigation".to_string()),
            action_category: "irrigation".to_string(),
            priority: RecommendationPriority::High,
            status: RecommendationStatus::Open,
            evidence_refs: vec!["annotation:ann-1".to_string()],
            annotation_ids: vec!["ann-1".to_string()],
            created_at: "2026-04-19T00:00:00Z".to_string(),
            updated_at: "2026-04-19T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn recommendation_filters_match_status_and_priority() {
        let recommendation = sample_recommendation();
        let mut state = RecommendationOverlayState::default();
        assert!(recommendation_matches_filters(&recommendation, &state));

        state.status_filter = Some(RecommendationStatus::Closed);
        assert!(!recommendation_matches_filters(&recommendation, &state));

        state.status_filter = Some(RecommendationStatus::Open);
        state.priority_filter = Some(RecommendationPriority::High);
        assert!(recommendation_matches_filters(&recommendation, &state));

        state.priority_filter = Some(RecommendationPriority::Low);
        assert!(!recommendation_matches_filters(&recommendation, &state));
    }

    #[test]
    fn recommendation_create_payload_links_annotation_evidence() {
        let state = RecommendationOverlayState {
            draft_title: "Scout irrigation line".to_string(),
            draft_note: "Verify nozzle coverage".to_string(),
            draft_category: "irrigation".to_string(),
            draft_priority: RecommendationPriority::High,
            draft_status: RecommendationStatus::Open,
            linked_annotation_ids: vec!["ann-1".to_string()],
            ..Default::default()
        };

        let payload =
            build_recommendation_create_payload(&state).expect("annotation evidence should create");

        assert_eq!(payload.title, "Scout irrigation line");
        assert_eq!(payload.category.as_deref(), Some("irrigation"));
        assert_eq!(payload.priority, RecommendationPriority::High);
        assert_eq!(payload.status, RecommendationStatus::Open);
        assert_eq!(payload.annotation_ids, vec!["ann-1".to_string()]);
    }

    #[test]
    fn recommendation_create_payload_rejects_zero_annotations() {
        let state = RecommendationOverlayState {
            draft_title: "Scout irrigation line".to_string(),
            ..Default::default()
        };

        let err = build_recommendation_create_payload(&state)
            .expect_err("zero linked annotations should be rejected");

        assert!(err.to_string().contains("annotation"));
    }
}
