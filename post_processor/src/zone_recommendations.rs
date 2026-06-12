use crate::zone_delineation::AnomalyZone;
use serde::{Deserialize, Serialize};
use shared::schemas::{
    RecommendationLifecycleRegistry, RecommendationPersistenceError, RecommendationPriority,
    RecommendationRecord, RecommendationStatus,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneRecommendationRequest {
    pub recommendation_id: String,
    pub scene_id: String,
    pub field_id: String,
    pub org_id: String,
    pub author_user_id: String,
    pub zone: Option<AnomalyZone>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ZoneRecommendationError {
    #[error("recommendation requires a delineated anomaly zone")]
    MissingZone,
    #[error("recommendation persistence failed: {source}")]
    Persistence {
        source: RecommendationPersistenceError,
    },
}

pub fn create_recommendation_from_zone(
    registry: &mut RecommendationLifecycleRegistry,
    request: ZoneRecommendationRequest,
) -> Result<RecommendationRecord, ZoneRecommendationError> {
    let zone = request.zone.ok_or(ZoneRecommendationError::MissingZone)?;
    let action_category = "scout".to_string();
    let evidence_refs = zone_evidence_refs(&zone, request.evidence_refs);
    let recommendation = RecommendationRecord {
        recommendation_id: request.recommendation_id,
        scene_id: request.scene_id,
        field_id: Some(request.field_id),
        org_id: request.org_id,
        author_user_id: request.author_user_id,
        title: format!("Scout anomaly zone {}", zone.zone_id),
        note: Some(format!(
            "Zone {} covers {:.1} m2; centroid ({:.6}, {:.6}) in {}.",
            zone.zone_id, zone.area_m2, zone.centroid.0, zone.centroid.1, zone.crs
        )),
        category: Some(action_category.clone()),
        action_category,
        priority: priority_for_zone_area(zone.area_m2),
        status: RecommendationStatus::Open,
        evidence_refs,
        annotation_ids: Vec::new(),
        created_at: request.created_at.clone(),
        updated_at: request.created_at,
    };

    registry
        .create_recommendation(recommendation)
        .map_err(|source| ZoneRecommendationError::Persistence { source })
}

pub fn priority_for_zone_area(area_m2: f32) -> RecommendationPriority {
    if !area_m2.is_finite() || area_m2 < 500.0 {
        RecommendationPriority::Low
    } else if area_m2 < 2_500.0 {
        RecommendationPriority::Medium
    } else if area_m2 < 10_000.0 {
        RecommendationPriority::High
    } else {
        RecommendationPriority::Critical
    }
}

fn zone_evidence_refs(zone: &AnomalyZone, evidence_refs: Vec<String>) -> Vec<String> {
    let mut refs = Vec::from([format!("zone:{}", zone.zone_id.trim())]);
    for evidence_ref in evidence_refs {
        let evidence_ref = evidence_ref.trim();
        if !evidence_ref.is_empty() && !refs.iter().any(|existing| existing == evidence_ref) {
            refs.push(evidence_ref.to_string());
        }
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::{
        create_recommendation_from_zone, priority_for_zone_area, ZoneRecommendationError,
        ZoneRecommendationRequest,
    };
    use crate::zone_delineation::{AnomalyZone, AnomalyZonePolygon};
    use shared::schemas::{
        RecommendationLifecycleRegistry, RecommendationPriority, RecommendationStatus,
        RecommendationStatusChangeType,
    };

    #[test]
    fn zone_recommendation_persists_priority_category_and_evidence() {
        let mut registry = RecommendationLifecycleRegistry::default();
        let recommendation = create_recommendation_from_zone(
            &mut registry,
            ZoneRecommendationRequest {
                recommendation_id: "rec-zone-1".to_string(),
                scene_id: "scene-1".to_string(),
                field_id: "field-1".to_string(),
                org_id: "org-a".to_string(),
                author_user_id: "advisor-1".to_string(),
                zone: Some(zone("zone-1", 3_000.0)),
                evidence_refs: vec!["layer:ndvi-2026-05-01".to_string()],
                created_at: "2026-05-01T00:00:00Z".to_string(),
            },
        )
        .expect("zone recommendation persists");

        assert_eq!(recommendation.recommendation_id, "rec-zone-1");
        assert_eq!(recommendation.field_id.as_deref(), Some("field-1"));
        assert_eq!(recommendation.org_id, "org-a");
        assert_eq!(recommendation.author_user_id, "advisor-1");
        assert_eq!(recommendation.priority, RecommendationPriority::High);
        assert_eq!(recommendation.action_category, "scout");
        assert_eq!(recommendation.status, RecommendationStatus::Open);
        assert_eq!(
            recommendation.evidence_refs,
            vec![
                "zone:zone-1".to_string(),
                "layer:ndvi-2026-05-01".to_string()
            ]
        );

        let stored = registry.recommendations_for_org("org-a");
        assert_eq!(stored, vec![recommendation]);
        assert!(registry.recommendations_for_org("org-b").is_empty());

        registry
            .transition_recommendation_status(
                "org-a",
                "rec-zone-1",
                "advisor-2",
                "2026-05-02T00:00:00Z",
                RecommendationStatus::Reviewed,
            )
            .expect("review transition persists");
        registry
            .transition_recommendation_status(
                "org-a",
                "rec-zone-1",
                "advisor-2",
                "2026-05-03T00:00:00Z",
                RecommendationStatus::Completed,
            )
            .expect("completion transition persists");
        let history = registry.recommendation_history("org-a", "rec-zone-1");
        assert_eq!(history.len(), 3);
        assert_eq!(
            history[0].change_type,
            RecommendationStatusChangeType::Created
        );
        assert_eq!(
            history[1].change_type,
            RecommendationStatusChangeType::StatusChanged
        );
        assert_eq!(history[2].after, RecommendationStatus::Completed);
    }

    #[test]
    fn zone_recommendation_rejects_missing_zone_without_persisting() {
        let mut registry = RecommendationLifecycleRegistry::default();
        let error = create_recommendation_from_zone(
            &mut registry,
            ZoneRecommendationRequest {
                recommendation_id: "rec-zone-1".to_string(),
                scene_id: "scene-1".to_string(),
                field_id: "field-1".to_string(),
                org_id: "org-a".to_string(),
                author_user_id: "advisor-1".to_string(),
                zone: None,
                evidence_refs: vec!["layer:ndvi-2026-05-01".to_string()],
                created_at: "2026-05-01T00:00:00Z".to_string(),
            },
        )
        .expect_err("missing zone is rejected");

        assert_eq!(error, ZoneRecommendationError::MissingZone);
        assert!(registry.recommendations_for_org("org-a").is_empty());
    }

    #[test]
    fn zone_priority_ranking_is_deterministic() {
        assert_eq!(priority_for_zone_area(99.0), RecommendationPriority::Low);
        assert_eq!(
            priority_for_zone_area(500.0),
            RecommendationPriority::Medium
        );
        assert_eq!(
            priority_for_zone_area(2_500.0),
            RecommendationPriority::High
        );
        assert_eq!(
            priority_for_zone_area(10_000.0),
            RecommendationPriority::Critical
        );
    }

    fn zone(zone_id: &str, area_m2: f32) -> AnomalyZone {
        AnomalyZone {
            zone_id: zone_id.to_string(),
            cell_indices: vec![0, 1],
            polygon: AnomalyZonePolygon {
                coordinates: vec![
                    (500000.0, 4500020.0),
                    (500020.0, 4500020.0),
                    (500020.0, 4500010.0),
                    (500000.0, 4500010.0),
                    (500000.0, 4500020.0),
                ],
            },
            area_m2,
            centroid: (500010.0, 4500015.0),
            crs: "EPSG:32614".to_string(),
        }
    }
}
