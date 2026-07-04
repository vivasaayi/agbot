use serde_json::json;
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};

fn base_scope() -> ProductScope {
    ProductScope {
        farm_id: Some("farm-1".to_string()),
        field_id: Some("field-7".to_string()),
        season_id: Some("season-2026".to_string()),
        scene_id: Some("scene-42".to_string()),
        temporal_start: "2026-07-01T00:00:00Z".to_string(),
        temporal_end: "2026-07-01T01:00:00Z".to_string(),
    }
}

fn base_draft() -> ProductRecordDraft {
    ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "ndvi".to_string(),
        algorithm_id: "ndvi.standard".to_string(),
        algorithm_version: "1.2.0".to_string(),
        parameters: json!({
            "epsilon": 1e-6,
            "clamp": {"min": -1.0, "max": 1.0},
        }),
        inputs: vec![
            ProductInputRef {
                product_id: "scene-42:reflectance:aaaaaaaaaaaa".to_string(),
                role: "band:nir".to_string(),
            },
            ProductInputRef {
                product_id: "scene-42:reflectance:bbbbbbbbbbbb".to_string(),
                role: "band:red".to_string(),
            },
        ],
        scope: base_scope(),
        spatial_ref: None,
        gsd_m_per_px: Some(0.05),
        artifact: Some(ProductArtifact {
            path: "products/scene-42/ndvi.tif".to_string(),
            format: "geotiff".to_string(),
            checksum_sha256: None,
        }),
        quality_mask: None,
        confidence: Some(0.93),
        confidence_method: Some("qa_band_fraction".to_string()),
        quality_summary: None,
        evidence_digests: vec!["sha256:deadbeef".to_string()],
        source_id: Some("collector:ms-1".to_string()),
    }
}

#[test]
fn parameters_hash_is_key_order_independent() {
    let mut a = base_draft();
    a.parameters = json!({
        "epsilon": 1e-6,
        "clamp": {"min": -1.0, "max": 1.0},
    });
    let mut b = base_draft();
    b.parameters = json!({
        "clamp": {"max": 1.0, "min": -1.0},
        "epsilon": 1e-6,
    });
    assert_eq!(a.parameters_hash(), b.parameters_hash());
}

#[test]
fn parameters_hash_changes_with_parameter_value() {
    let a = base_draft();
    let mut b = base_draft();
    b.parameters = json!({
        "epsilon": 1e-5,
        "clamp": {"min": -1.0, "max": 1.0},
    });
    assert_ne!(a.parameters_hash(), b.parameters_hash());
}

#[test]
fn parameters_hash_is_input_order_independent() {
    let a = base_draft();
    let mut b = base_draft();
    b.inputs.reverse();
    assert_eq!(a.parameters_hash(), b.parameters_hash());
}

#[test]
fn parameters_hash_changes_with_algorithm_identity() {
    let a = base_draft();
    let mut b = base_draft();
    b.algorithm_version = "1.3.0".to_string();
    assert_ne!(a.parameters_hash(), b.parameters_hash());
}

#[test]
fn product_id_prefers_scene_scope_and_is_stable() {
    let draft = base_draft();
    let id = draft.product_id();
    assert!(
        id.starts_with("scene-42:ndvi:"),
        "unexpected product id shape: {id}"
    );
    let hash_part = id.rsplit(':').next().unwrap();
    assert_eq!(hash_part.len(), 12);
    assert_eq!(hash_part, &draft.parameters_hash()[..12]);
    // Golden assertion: accidental identity-rule changes must break this test.
    assert_eq!(id, "scene-42:ndvi:f6f6a38a4c6f");
}

#[test]
fn product_id_falls_back_to_field_and_temporal_then_global() {
    let mut draft = base_draft();
    draft.scope.scene_id = None;
    let id = draft.product_id();
    assert!(
        id.starts_with("field-7@2026-07-01T00:00:00Z:ndvi:"),
        "unexpected field-scoped product id: {id}"
    );

    draft.scope.field_id = None;
    let id = draft.product_id();
    assert!(
        id.starts_with("global:ndvi:"),
        "unexpected global product id: {id}"
    );
}

#[test]
fn product_level_serializes_lowercase() {
    assert_eq!(serde_json::to_string(&ProductLevel::L2).unwrap(), "\"l2\"");
    let parsed: ProductLevel = serde_json::from_str("\"l0\"").unwrap();
    assert_eq!(parsed, ProductLevel::L0);
}

#[test]
fn product_level_str_conversions_and_display() {
    assert_eq!(ProductLevel::L3.as_str(), "l3");
    assert_eq!(ProductLevel::L1.to_string(), "l1");
    assert_eq!("l2".parse::<ProductLevel>().unwrap(), ProductLevel::L2);
    assert!("l9".parse::<ProductLevel>().is_err());
}

#[test]
fn product_level_ordering() {
    assert!(ProductLevel::L0 < ProductLevel::L1);
    assert!(ProductLevel::L0 < ProductLevel::L3);
    assert!(ProductLevel::L2 < ProductLevel::L3);
}

#[test]
fn product_record_draft_serde_round_trip() {
    let draft = base_draft();
    let text = serde_json::to_string(&draft).unwrap();
    let restored: ProductRecordDraft = serde_json::from_str(&text).unwrap();
    assert_eq!(draft, restored);
}
