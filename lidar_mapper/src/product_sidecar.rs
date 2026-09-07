//! Product-record sidecar emission for LiDAR products (Track A phase 10b).
//!
//! The parallel of `imagery_processor::product_sidecar`: lidar_mapper writes a
//! `*.product_record.json` sidecar next to each derived LiDAR raster
//! (occupancy grid, coverage density, obstacle heatmap). The sidecar is a
//! [`ProductRecordDraft`] mapped from the product's reproducibility evidence; a
//! later `geo_hub catalog register` pass walks the sidecars into the catalog.
//!
//! Identity invariant: a LiDAR raster MUST list its source scans in `inputs`, or
//! two areas' same-parameter products collapse into one catalog row. The scan ids
//! from the evidence become identity-bearing L0 input refs.

use crate::LidarProductReproducibilityEvidence;
use shared::error::AgroError;
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use shared::AgroResult;
use std::path::{Path, PathBuf};

/// Catalog context the reproducibility evidence does not itself carry: the scene
/// (optional — LiDAR products may be area-scoped), the acquisition timestamp, the
/// algorithm version, and the source id.
#[derive(Debug, Clone)]
pub struct LidarSidecarContext {
    pub scene_id: Option<String>,
    pub timestamp: String,
    pub algorithm_version: String,
    pub source_id: Option<String>,
}

fn artifact_format(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Map a LiDAR product's reproducibility evidence into a catalog draft. The
/// source scans become identity-bearing L0 inputs (`scan:<uuid>`); the output
/// hash is the artifact checksum and is also recorded in `evidence_digests`.
pub fn draft_from_lidar_evidence(
    evidence: &LidarProductReproducibilityEvidence,
    ctx: &LidarSidecarContext,
    product_path: &Path,
) -> ProductRecordDraft {
    let inputs = evidence
        .scan_ids
        .iter()
        .map(|scan_id| ProductInputRef {
            product_id: format!("scan:{scan_id}"),
            role: "lidar_scan".to_string(),
        })
        .collect();

    let checksum = if evidence.output_hash.value.is_empty() {
        None
    } else {
        Some(evidence.output_hash.value.clone())
    };
    let evidence_digests = checksum.clone().into_iter().collect();

    ProductRecordDraft {
        level: ProductLevel::L2,
        kind: evidence.product_kind.clone(),
        algorithm_id: format!("lidar.{}", evidence.product_kind),
        algorithm_version: ctx.algorithm_version.clone(),
        parameters: serde_json::json!({
            "scan_ids": evidence.scan_ids,
            "cleaning_params": evidence.cleaning_params,
            "thresholds": evidence.thresholds,
            "width": evidence.width,
            "height": evidence.height,
        }),
        inputs,
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: ctx.scene_id.clone(),
            temporal_start: ctx.timestamp.clone(),
            temporal_end: ctx.timestamp.clone(),
        },
        spatial_ref: Some(evidence.spatial_ref.clone()),
        gsd_m_per_px: None,
        artifact: Some(ProductArtifact {
            path: product_path.to_string_lossy().to_string(),
            format: artifact_format(product_path),
            checksum_sha256: checksum,
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: Some(serde_json::json!({
            "observation_counts": evidence.observation_counts,
        })),
        evidence_digests,
        source_id: ctx.source_id.clone(),
    }
}

/// `grid.tif` -> `grid.product_record.json` (sibling of the product file).
pub fn product_record_sidecar_path(product_path: &Path) -> PathBuf {
    let stem = product_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("product");
    product_path.with_file_name(format!("{stem}.product_record.json"))
}

/// Write the draft as a `*.product_record.json` sidecar next to the product.
pub async fn write_lidar_product_sidecar(
    product_path: &Path,
    draft: &ProductRecordDraft,
) -> AgroResult<PathBuf> {
    let sidecar_path = product_record_sidecar_path(product_path);
    let json = serde_json::to_string_pretty(draft).map_err(|err| {
        AgroError::Processing(format!("failed to encode lidar product record: {err}"))
    })?;
    tokio::fs::write(&sidecar_path, json).await?;
    Ok(sidecar_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LidarObservationCounts, LidarOccupancyGridEvidence, LidarProductOutputHash};
    use shared::schemas::RasterSpatialRef;
    use uuid::Uuid;

    fn spatial_ref() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32614".to_string()),
            bbox: None,
            geo_transform: None,
            resolution: None,
        }
    }

    fn evidence(scan_ids: Vec<Uuid>, checksum: &str) -> LidarProductReproducibilityEvidence {
        LidarProductReproducibilityEvidence {
            product_kind: "occupancy_grid".to_string(),
            scan_ids,
            cleaning_params: None,
            thresholds: LidarOccupancyGridEvidence {
                distance_threshold_m: 0.5,
                quality_threshold: 10,
                occupancy_threshold: 0.6,
                flip_y: false,
            },
            observation_counts: LidarObservationCounts {
                occupied_cells: 10,
                free_observed_cells: 90,
                obstacle_observations: 5,
                total_observations: 100,
            },
            spatial_ref: spatial_ref(),
            width: 64,
            height: 64,
            output_hash: LidarProductOutputHash {
                algorithm: "sha256".to_string(),
                value: checksum.to_string(),
            },
        }
    }

    fn ctx() -> LidarSidecarContext {
        LidarSidecarContext {
            scene_id: Some("area-1".to_string()),
            timestamp: "2026-06-01T00:00:00Z".to_string(),
            algorithm_version: "1.0.0".to_string(),
            source_id: Some("drone-lidar-1".to_string()),
        }
    }

    #[test]
    fn draft_maps_lidar_evidence_as_l2_with_scan_inputs() {
        let scan = Uuid::new_v4();
        let draft = draft_from_lidar_evidence(
            &evidence(vec![scan], "hash-xyz"),
            &ctx(),
            Path::new("out/occupancy.tif"),
        );
        assert_eq!(draft.level, ProductLevel::L2);
        assert_eq!(draft.kind, "occupancy_grid");
        assert_eq!(draft.algorithm_id, "lidar.occupancy_grid");
        assert_eq!(draft.inputs.len(), 1);
        assert_eq!(draft.inputs[0].product_id, format!("scan:{scan}"));
        assert_eq!(draft.inputs[0].role, "lidar_scan");
        assert_eq!(
            draft.artifact.as_ref().unwrap().checksum_sha256.as_deref(),
            Some("hash-xyz")
        );
        assert_eq!(draft.artifact.as_ref().unwrap().format, "tif");
    }

    #[test]
    fn identity_invariant_distinguishes_areas_by_scan_inputs() {
        let shared_hash = "same-hash";
        let a = draft_from_lidar_evidence(
            &evidence(vec![Uuid::new_v4()], shared_hash),
            &ctx(),
            Path::new("a/occupancy.tif"),
        );
        let b = draft_from_lidar_evidence(
            &evidence(vec![Uuid::new_v4()], shared_hash),
            &ctx(),
            Path::new("b/occupancy.tif"),
        );
        assert_ne!(
            a.parameters_hash(),
            b.parameters_hash(),
            "different source scans must yield distinct catalog identity"
        );
    }

    #[tokio::test]
    async fn sidecar_writes_next_to_product_and_round_trips() {
        let dir = std::env::temp_dir().join(format!("agbot_lidar_sidecar_{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let product = dir.join("occupancy.tif");
        tokio::fs::write(&product, b"fake").await.unwrap();

        let draft =
            draft_from_lidar_evidence(&evidence(vec![Uuid::new_v4()], "h"), &ctx(), &product);
        let sidecar = write_lidar_product_sidecar(&product, &draft).await.unwrap();
        assert_eq!(sidecar, dir.join("occupancy.product_record.json"));
        let loaded: ProductRecordDraft =
            serde_json::from_slice(&tokio::fs::read(&sidecar).await.unwrap()).unwrap();
        assert_eq!(loaded.parameters_hash(), draft.parameters_hash());

        tokio::fs::remove_dir_all(&dir).await.ok();
    }
}
