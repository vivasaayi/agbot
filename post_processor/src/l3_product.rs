//! L3 product drafts from analysis outputs (Track A batch 9).
//!
//! post_processor analyses (NDVI trend/health, thermal anomalies, LiDAR change,
//! index anomaly/trend, zonal stats, zone priorities) are the L3 layer. This
//! module maps an analysis run into a catalog [`ProductRecordDraft`] whose
//! `inputs` are the L2 catalog product ids it consumed — the identity invariant:
//! an L3 aggregate that omits its L2 inputs collapses with every other run of
//! the same parameters.

use crate::{AnalysisJobRequest, HealthUncertaintyBand};
use shared::product_graph::{ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope};

/// Everything needed to derive an L3 draft that the analysis result does not
/// already imply.
#[derive(Debug, Clone)]
pub struct L3DraftContext {
    pub kind: String,
    pub algorithm_id: String,
    pub algorithm_version: String,
    pub field_id: String,
    pub season_id: String,
    /// L3 aggregates may span scenes; `None` leaves the product field-scoped.
    pub scene_id: Option<String>,
    pub temporal_start: String,
    pub temporal_end: String,
    /// L2 catalog product ids consumed (identity-bearing).
    pub input_product_ids: Vec<String>,
    pub parameters: serde_json::Value,
    pub confidence: Option<f64>,
    pub confidence_method: Option<String>,
    pub evidence_digests: Vec<String>,
    pub source_id: Option<String>,
}

/// Map an uncertainty band to a scalar confidence: a narrower band is more
/// confident. `confidence = clamp(1 - (upper - lower), 0, 1)`.
pub fn confidence_from_uncertainty(band: &HealthUncertaintyBand) -> f64 {
    let width = (band.upper - band.lower).abs() as f64;
    (1.0 - width).clamp(0.0, 1.0)
}

/// Build an L3 catalog draft from context. Inputs become `l2_input` edges.
pub fn to_l3_draft(ctx: &L3DraftContext) -> ProductRecordDraft {
    ProductRecordDraft {
        level: ProductLevel::L3,
        kind: ctx.kind.clone(),
        algorithm_id: ctx.algorithm_id.clone(),
        algorithm_version: ctx.algorithm_version.clone(),
        parameters: ctx.parameters.clone(),
        inputs: ctx
            .input_product_ids
            .iter()
            .map(|id| ProductInputRef {
                product_id: id.clone(),
                role: "l2_input".to_string(),
            })
            .collect(),
        scope: ProductScope {
            farm_id: None,
            field_id: Some(ctx.field_id.clone()),
            season_id: Some(ctx.season_id.clone()),
            scene_id: ctx.scene_id.clone(),
            temporal_start: ctx.temporal_start.clone(),
            temporal_end: ctx.temporal_end.clone(),
        },
        spatial_ref: None,
        gsd_m_per_px: None,
        artifact: None, // L3 aggregates need not have a single backing file
        quality_mask: None,
        confidence: ctx.confidence,
        confidence_method: ctx.confidence_method.clone(),
        quality_summary: None,
        evidence_digests: ctx.evidence_digests.clone(),
        source_id: ctx.source_id.clone(),
    }
}

/// Convenience: build an L3 draft from an analysis request (scope + L2 input
/// refs in `product_refs`) plus the result's kind/parameters/uncertainty.
pub fn l3_draft_from_request(
    request: &AnalysisJobRequest,
    kind: &str,
    algorithm_id: &str,
    algorithm_version: &str,
    parameters: serde_json::Value,
    uncertainty: Option<&HealthUncertaintyBand>,
    evidence_digests: Vec<String>,
) -> ProductRecordDraft {
    let (confidence, confidence_method) = match uncertainty {
        Some(band) => (
            Some(confidence_from_uncertainty(band)),
            Some("uncertainty_band_width".to_string()),
        ),
        None => (None, None),
    };
    to_l3_draft(&L3DraftContext {
        kind: kind.to_string(),
        algorithm_id: algorithm_id.to_string(),
        algorithm_version: algorithm_version.to_string(),
        field_id: request.field_id.clone(),
        season_id: request.season_id.clone(),
        scene_id: Some(request.scene_id.clone()),
        temporal_start: request.scene_id.clone(),
        temporal_end: request.scene_id.clone(),
        input_product_ids: request.product_refs.clone(),
        parameters,
        confidence,
        confidence_method,
        evidence_digests,
        source_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(field: &str, inputs: Vec<&str>) -> L3DraftContext {
        L3DraftContext {
            kind: "ndvi_trend".to_string(),
            algorithm_id: "ndvi_trend.compute".to_string(),
            algorithm_version: "1.0.0".to_string(),
            field_id: field.to_string(),
            season_id: "2026".to_string(),
            scene_id: None,
            temporal_start: "2026-06-01T00:00:00Z".to_string(),
            temporal_end: "2026-06-30T00:00:00Z".to_string(),
            input_product_ids: inputs.into_iter().map(String::from).collect(),
            parameters: serde_json::json!({ "window": 3 }),
            confidence: None,
            confidence_method: None,
            evidence_digests: vec![],
            source_id: None,
        }
    }

    #[test]
    fn l3_draft_is_l3_with_l2_input_edges() {
        let draft = to_l3_draft(&ctx(
            "field-1",
            vec!["scene-a:ndvi:aaa", "scene-b:ndvi:bbb"],
        ));
        assert_eq!(draft.level, ProductLevel::L3);
        assert_eq!(draft.inputs.len(), 2);
        assert!(draft.inputs.iter().all(|i| i.role == "l2_input"));
        assert!(draft.artifact.is_none());
    }

    #[test]
    fn identity_distinguishes_runs_by_l2_inputs() {
        // Same params, different L2 inputs -> distinct identity (no collapse).
        let a = to_l3_draft(&ctx("field-1", vec!["scene-a:ndvi:aaa"]));
        let b = to_l3_draft(&ctx("field-1", vec!["scene-b:ndvi:bbb"]));
        assert_ne!(a.parameters_hash(), b.parameters_hash());
        // Same inputs -> same identity.
        let a2 = to_l3_draft(&ctx("field-9", vec!["scene-a:ndvi:aaa"]));
        assert_eq!(
            a.parameters_hash(),
            a2.parameters_hash(),
            "scope is not identity"
        );
    }

    #[test]
    fn confidence_narrows_with_the_uncertainty_band() {
        let tight = confidence_from_uncertainty(&HealthUncertaintyBand {
            lower: 0.48,
            upper: 0.52,
        });
        let wide = confidence_from_uncertainty(&HealthUncertaintyBand {
            lower: 0.1,
            upper: 0.9,
        });
        assert!(tight > wide);
        assert!((0.0..=1.0).contains(&tight) && (0.0..=1.0).contains(&wide));
    }
}
