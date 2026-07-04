//! Product-record sidecar emission (Track A batch 7).
//!
//! imagery_processor writes a `*.product_record.json` sidecar next to each
//! output (the same pattern as the existing `spatial_ref.json` sidecars). The
//! sidecar is a [`ProductRecordDraft`] mapped from the processing evidence; a
//! later `geo_hub catalog register` pass (TA-08) walks the sidecars and
//! registers them into the catalog.
//!
//! Identity invariant (see catalog batch 2): an L2 index MUST list its scene's
//! L1 band products in `inputs`, or two scenes' same-parameter indices collapse
//! into one catalog row. The caller supplies those input refs via
//! [`SidecarContext`]; the evidence's `source_image_ids` are additionally
//! recorded in the parameters for provenance.

use crate::io::ProductReproducibilityEvidence;
use shared::error::AgroError;
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use shared::schemas::RasterSpatialRef;
use shared::AgroResult;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Build an L2-index sidecar context in the live CLI processing path, where
/// catalog product ids have not yet been assigned (registration happens later,
/// via `geo_hub catalog register`). Two decisions make the draft honest here:
///
/// * **Scope is scene-only.** The CLI knows the scene (the source image) but not
///   the field/season — those are enriched at register time. Leaving them `None`
///   is truthful rather than guessing.
/// * **Inputs are derived deterministically from the resolved bands.** Each
///   resolved band becomes an identity-bearing L1 input ref `scene:role:band`, so
///   two scenes' same-parameter indices carry distinct inputs and never collapse
///   into one catalog row (the identity invariant).
#[allow(clippy::too_many_arguments)]
pub fn l2_sidecar_context(
    scene_id: &str,
    kind: &str,
    algorithm_version: &str,
    inputs: Vec<ProductInputRef>,
    spatial_ref: Option<RasterSpatialRef>,
    timestamp: &str,
    mask_ref: Option<&str>,
    source_id: Option<String>,
) -> SidecarContext {
    SidecarContext {
        level: ProductLevel::L2,
        kind: kind.to_string(),
        algorithm_version: algorithm_version.to_string(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(scene_id.to_string()),
            temporal_start: timestamp.to_string(),
            temporal_end: timestamp.to_string(),
        },
        inputs,
        quality_mask: mask_ref.map(|reference| ProductInputRef {
            product_id: reference.to_string(),
            role: "mask".to_string(),
        }),
        spatial_ref,
        gsd_m_per_px: None,
        source_id,
    }
}

/// An identity-bearing L1 input ref for a resolved band in the CLI path:
/// `scene:role:band`. Two scenes' same band role stay distinct because the scene
/// and band name are folded in.
pub fn band_input_ref(scene_id: &str, role: &str, band: &str) -> ProductInputRef {
    ProductInputRef {
        product_id: format!("{scene_id}:{role}:{band}"),
        role: format!("band:{role}"),
    }
}

/// [`l2_sidecar_context`] specialized to an index product, deriving the inputs
/// from the resolved bands map (role -> band name).
#[allow(clippy::too_many_arguments)]
pub fn l2_index_sidecar_context(
    scene_id: &str,
    kind: &str,
    algorithm_version: &str,
    resolved_bands: &BTreeMap<String, String>,
    spatial_ref: RasterSpatialRef,
    timestamp: &str,
    mask_ref: Option<&str>,
    source_id: Option<String>,
) -> SidecarContext {
    let inputs = resolved_bands
        .iter()
        .map(|(role, band)| band_input_ref(scene_id, role, band))
        .collect();
    l2_sidecar_context(
        scene_id,
        kind,
        algorithm_version,
        inputs,
        Some(spatial_ref),
        timestamp,
        mask_ref,
        source_id,
    )
}

/// Catalog context the processing evidence does not itself carry: product level,
/// kind, scope, and the upstream input product refs (identity-bearing).
#[derive(Debug, Clone)]
pub struct SidecarContext {
    pub level: ProductLevel,
    pub kind: String,
    pub algorithm_version: String,
    pub scope: ProductScope,
    pub inputs: Vec<ProductInputRef>,
    pub quality_mask: Option<ProductInputRef>,
    pub spatial_ref: Option<RasterSpatialRef>,
    pub gsd_m_per_px: Option<f64>,
    pub source_id: Option<String>,
}

fn artifact_format(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Map processing evidence + catalog context into a product draft. The artifact
/// checksum is taken from the evidence's `product` output hash; every output
/// hash is recorded in `evidence_digests`; `source_image_ids` are folded into
/// the parameters for provenance.
pub fn draft_from_evidence(
    evidence: &ProductReproducibilityEvidence,
    ctx: &SidecarContext,
    product_path: &Path,
) -> ProductRecordDraft {
    let checksum = evidence
        .output_hashes
        .get("product")
        .map(|hash| hash.value.clone())
        .filter(|value| !value.is_empty());

    let source_ids: Vec<String> = evidence
        .source_image_ids
        .iter()
        .map(|id| id.to_string())
        .collect();
    let mut parameters = evidence.parameters.clone();
    match &mut parameters {
        serde_json::Value::Object(map) => {
            map.insert(
                "source_image_ids".to_string(),
                serde_json::json!(source_ids),
            );
        }
        serde_json::Value::Null => {
            parameters = serde_json::json!({ "source_image_ids": source_ids });
        }
        other => {
            parameters = serde_json::json!({ "value": other, "source_image_ids": source_ids });
        }
    }

    let evidence_digests = evidence
        .output_hashes
        .values()
        .map(|hash| hash.value.clone())
        .filter(|value| !value.is_empty())
        .collect();

    let algorithm_id = if evidence.method.is_empty() {
        ctx.kind.clone()
    } else {
        evidence.method.clone()
    };

    ProductRecordDraft {
        level: ctx.level,
        kind: ctx.kind.clone(),
        algorithm_id,
        algorithm_version: ctx.algorithm_version.clone(),
        parameters,
        inputs: ctx.inputs.clone(),
        scope: ctx.scope.clone(),
        spatial_ref: ctx.spatial_ref.clone(),
        gsd_m_per_px: ctx.gsd_m_per_px,
        artifact: Some(ProductArtifact {
            path: product_path.to_string_lossy().to_string(),
            format: artifact_format(product_path),
            checksum_sha256: checksum,
        }),
        quality_mask: ctx.quality_mask.clone(),
        confidence: None,
        confidence_method: None,
        quality_summary: Some(serde_json::json!({
            "statistics": evidence.statistics,
            "coverage": evidence.coverage,
        })),
        evidence_digests,
        source_id: ctx.source_id.clone(),
    }
}

/// `foo.tif` -> `foo.product_record.json` (sibling of the product file).
pub fn product_record_sidecar_path(product_path: &Path) -> PathBuf {
    let stem = product_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("product");
    product_path.with_file_name(format!("{stem}.product_record.json"))
}

/// Write the draft as a `*.product_record.json` sidecar next to the product.
/// Returns the sidecar path.
pub async fn write_product_sidecar(
    product_path: &Path,
    draft: &ProductRecordDraft,
) -> AgroResult<PathBuf> {
    let sidecar_path = product_record_sidecar_path(product_path);
    let json = serde_json::to_string_pretty(draft)
        .map_err(|err| AgroError::Processing(format!("failed to encode product record: {err}")))?;
    tokio::fs::write(&sidecar_path, json).await?;
    Ok(sidecar_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::ProductOutputHash;
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;

    fn scope(scene: &str) -> ProductScope {
        ProductScope {
            farm_id: None,
            field_id: Some("field-1".to_string()),
            season_id: Some("2026".to_string()),
            scene_id: Some(scene.to_string()),
            temporal_start: "2026-06-01T00:00:00Z".to_string(),
            temporal_end: "2026-06-01T00:00:00Z".to_string(),
        }
    }

    fn ndvi_context(scene: &str, band_inputs: Vec<&str>) -> SidecarContext {
        SidecarContext {
            level: ProductLevel::L2,
            kind: "ndvi".to_string(),
            algorithm_version: "1.0.0".to_string(),
            scope: scope(scene),
            inputs: band_inputs
                .into_iter()
                .map(|id| ProductInputRef {
                    product_id: id.to_string(),
                    role: "band:nir".to_string(),
                })
                .collect(),
            quality_mask: Some(ProductInputRef {
                product_id: format!("{scene}:qa_mask:abcdef012345"),
                role: "mask".to_string(),
            }),
            spatial_ref: None,
            gsd_m_per_px: Some(10.0),
            source_id: Some("landsat-9".to_string()),
        }
    }

    fn ndvi_evidence(checksum: &str) -> ProductReproducibilityEvidence {
        let mut output_hashes = BTreeMap::new();
        output_hashes.insert(
            "product".to_string(),
            ProductOutputHash {
                algorithm: "sha256".to_string(),
                value: checksum.to_string(),
            },
        );
        ProductReproducibilityEvidence::new(
            vec![uuid::Uuid::new_v4()],
            "index",
            serde_json::json!({ "index": "ndvi" }),
            None,
            Some("scene-1:qa_mask:abcdef012345".to_string()),
            serde_json::json!({ "mean": 0.42 }),
            serde_json::json!({ "clear_pixel_coverage": 0.9 }),
            output_hashes,
        )
    }

    #[test]
    fn draft_maps_evidence_fields() {
        let evidence = ndvi_evidence("hash-abc");
        let ctx = ndvi_context("scene-1", vec!["scene-1:band_nir:aaa"]);
        let draft = draft_from_evidence(&evidence, &ctx, Path::new("out/ndvi.tif"));

        assert_eq!(draft.level, ProductLevel::L2);
        assert_eq!(draft.kind, "ndvi");
        assert_eq!(draft.algorithm_id, "index");
        assert_eq!(draft.inputs.len(), 1);
        assert_eq!(draft.quality_mask.as_ref().unwrap().role, "mask");
        let artifact = draft.artifact.as_ref().unwrap();
        assert_eq!(artifact.format, "tif");
        assert_eq!(artifact.checksum_sha256.as_deref(), Some("hash-abc"));
        // source_image_ids are folded into parameters for provenance.
        assert!(draft.parameters.get("source_image_ids").is_some());
    }

    #[test]
    fn identity_invariant_distinguishes_scenes_by_inputs() {
        let evidence = ndvi_evidence("hash-abc");
        // Same computation, different scenes -> different band inputs -> distinct id.
        let a = draft_from_evidence(
            &evidence,
            &ndvi_context("scene-a", vec!["scene-a:band_nir:aaa"]),
            Path::new("a/ndvi.tif"),
        );
        let b = draft_from_evidence(
            &evidence,
            &ndvi_context("scene-b", vec!["scene-b:band_nir:bbb"]),
            Path::new("b/ndvi.tif"),
        );
        assert_ne!(
            a.parameters_hash(),
            b.parameters_hash(),
            "distinct band inputs must yield distinct identity (no collapse)"
        );

        // Same inputs -> same identity (collapse), proving inputs drive identity.
        let a2 = draft_from_evidence(
            &evidence,
            &ndvi_context("scene-a", vec!["scene-a:band_nir:aaa"]),
            Path::new("a/ndvi.tif"),
        );
        assert_eq!(a.parameters_hash(), a2.parameters_hash());
    }

    fn resolved_bands(nir: &str) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("red".to_string(), "B04".to_string()),
            ("nir".to_string(), nir.to_string()),
        ])
    }

    fn bare_spatial_ref() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: false,
            crs: None,
            bbox: None,
            geo_transform: None,
            resolution: None,
        }
    }

    #[test]
    fn cli_index_context_is_scene_scoped_with_per_band_inputs() {
        let ctx = l2_index_sidecar_context(
            "scene-a",
            "ndvi",
            "1.2.3",
            &resolved_bands("B08"),
            bare_spatial_ref(),
            "2026-06-01T00:00:00Z",
            Some("mask/qa.png"),
            None,
        );
        assert_eq!(ctx.level, ProductLevel::L2);
        // Scope is scene-only: field/season are enriched later at register time.
        assert_eq!(ctx.scope.scene_id.as_deref(), Some("scene-a"));
        assert!(ctx.scope.field_id.is_none());
        assert!(ctx.scope.season_id.is_none());
        // One identity-bearing L1 input per resolved band.
        assert_eq!(ctx.inputs.len(), 2);
        assert!(ctx
            .inputs
            .iter()
            .any(|i| i.product_id == "scene-a:nir:B08" && i.role == "band:nir"));
        assert_eq!(ctx.quality_mask.as_ref().unwrap().product_id, "mask/qa.png");
    }

    #[test]
    fn cli_index_inputs_keep_scenes_distinct() {
        let evidence = ndvi_evidence("hash-abc");
        let draft_a = draft_from_evidence(
            &evidence,
            &l2_index_sidecar_context(
                "scene-a",
                "ndvi",
                "1.0.0",
                &resolved_bands("B08a"),
                bare_spatial_ref(),
                "2026-06-01T00:00:00Z",
                None,
                None,
            ),
            Path::new("a/ndvi.tif"),
        );
        let draft_b = draft_from_evidence(
            &evidence,
            &l2_index_sidecar_context(
                "scene-b",
                "ndvi",
                "1.0.0",
                &resolved_bands("B08b"),
                bare_spatial_ref(),
                "2026-06-01T00:00:00Z",
                None,
                None,
            ),
            Path::new("b/ndvi.tif"),
        );
        assert_ne!(
            draft_a.parameters_hash(),
            draft_b.parameters_hash(),
            "different scenes' bands must yield distinct catalog identity"
        );
    }

    #[tokio::test]
    async fn sidecar_checksum_matches_recomputed_file_digest() {
        let dir = std::env::temp_dir().join(format!("agbot_sidecar_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let product = dir.join("ndvi.tif");
        let bytes = b"fake geotiff bytes";
        tokio::fs::write(&product, bytes).await.unwrap();

        // The digest carried in the evidence is the real file digest.
        let digest = format!("{:x}", Sha256::digest(bytes));
        let evidence = ndvi_evidence(&digest);
        let ctx = ndvi_context("scene-1", vec!["scene-1:band_nir:aaa"]);
        let draft = draft_from_evidence(&evidence, &ctx, &product);

        // Recompute the file digest and confirm the sidecar records it.
        let recomputed = format!(
            "{:x}",
            Sha256::digest(tokio::fs::read(&product).await.unwrap())
        );
        assert_eq!(
            draft.artifact.as_ref().unwrap().checksum_sha256.as_deref(),
            Some(recomputed.as_str())
        );

        // The sidecar writes next to the product and round-trips.
        let sidecar = write_product_sidecar(&product, &draft).await.unwrap();
        assert_eq!(sidecar, dir.join("ndvi.product_record.json"));
        let loaded: ProductRecordDraft =
            serde_json::from_slice(&tokio::fs::read(&sidecar).await.unwrap()).unwrap();
        assert_eq!(loaded.parameters_hash(), draft.parameters_hash());

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn mask_sidecar_is_written_before_the_index_that_references_it() {
        let dir = std::env::temp_dir().join(format!("agbot_maskorder_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await.unwrap();

        // Mask draft first.
        let mask_ctx = SidecarContext {
            level: ProductLevel::L2,
            kind: "qa_mask".to_string(),
            algorithm_version: "1.0.0".to_string(),
            scope: scope("scene-1"),
            inputs: vec![ProductInputRef {
                product_id: "scene-1:band_qa:qqq".to_string(),
                role: "band:qa".to_string(),
            }],
            quality_mask: None,
            spatial_ref: None,
            gsd_m_per_px: Some(10.0),
            source_id: Some("landsat-9".to_string()),
        };
        let mask_product = dir.join("qa_mask.tif");
        tokio::fs::write(&mask_product, b"mask").await.unwrap();
        let mask_draft = draft_from_evidence(&ndvi_evidence("m"), &mask_ctx, &mask_product);
        let mask_id = mask_draft.product_id();
        let mask_sidecar = write_product_sidecar(&mask_product, &mask_draft)
            .await
            .unwrap();
        assert!(tokio::fs::try_exists(&mask_sidecar).await.unwrap());

        // Index draft references the already-written mask.
        let mut index_ctx = ndvi_context("scene-1", vec!["scene-1:band_nir:aaa"]);
        index_ctx.quality_mask = Some(ProductInputRef {
            product_id: mask_id.clone(),
            role: "mask".to_string(),
        });
        let index_product = dir.join("ndvi.tif");
        tokio::fs::write(&index_product, b"ndvi").await.unwrap();
        let index_draft = draft_from_evidence(&ndvi_evidence("i"), &index_ctx, &index_product);
        assert_eq!(
            index_draft.quality_mask.as_ref().unwrap().product_id,
            mask_id,
            "index references the mask that was emitted first"
        );

        tokio::fs::remove_dir_all(&dir).await.ok();
    }
}
