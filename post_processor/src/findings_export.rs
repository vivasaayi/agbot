use crate::product_anomalies::ProductAnomalyReasonCode;
use crate::zone_delineation::AnomalyZone;
use interop::{
    export_findings_geojson as interop_export_findings_geojson,
    export_findings_shapefile as interop_export_findings_shapefile, FindingsExportFeature,
    FindingsExportRequest, FindingsShapefileExport, InteropCoordinate, InteropError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::schemas::RecommendationPriority;

pub const FINDINGS_CSV_HEADER: &str =
    "finding_id,zone_id,reason,priority,area_m2,centroid_x,centroid_y,crs,evidence_refs";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingExportRecord {
    pub finding_id: String,
    pub zone: AnomalyZone,
    pub reason: ProductAnomalyReasonCode,
    pub priority: RecommendationPriority,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum FindingsExportError {
    #[error("csv writer failed: {0}")]
    Csv(#[from] csv::Error),
    #[error("csv flush failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("csv output was not valid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("interop export failed: {0}")]
    Interop(#[from] InteropError),
    #[error("geojson output was not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn export_findings_csv(
    findings: &[FindingExportRecord],
) -> Result<String, FindingsExportError> {
    let mut buffer = Vec::new();
    {
        let mut writer = csv::Writer::from_writer(&mut buffer);
        writer.write_record(FINDINGS_CSV_HEADER.split(','))?;
        for finding in findings {
            writer.write_record([
                finding.finding_id.as_str(),
                finding.zone.zone_id.as_str(),
                reason_code_str(finding.reason),
                priority_str(finding.priority),
                &format!("{:.1}", finding.zone.area_m2),
                &format!("{:.6}", finding.zone.centroid.0),
                &format!("{:.6}", finding.zone.centroid.1),
                finding.zone.crs.as_str(),
                &normalized_evidence_refs(&finding.evidence_refs).join("|"),
            ])?;
        }
        writer.flush()?;
    }

    String::from_utf8(buffer).map_err(FindingsExportError::from)
}

pub fn export_findings_geojson(
    findings: &[FindingExportRecord],
) -> Result<Value, FindingsExportError> {
    let report = interop_export_findings_geojson(interop_findings_request(findings))?;
    serde_json::from_slice(&report.exported_bytes).map_err(FindingsExportError::from)
}

pub fn export_findings_shapefile(
    findings: &[FindingExportRecord],
) -> Result<FindingsShapefileExport, FindingsExportError> {
    interop_export_findings_shapefile(interop_findings_request(findings))
        .map_err(FindingsExportError::from)
}

fn interop_findings_request(findings: &[FindingExportRecord]) -> FindingsExportRequest {
    let crs = findings
        .first()
        .map(|finding| finding.zone.crs.clone())
        .unwrap_or_else(|| "EPSG:4326".to_string());
    FindingsExportRequest {
        export_id: "post_processor_findings".to_string(),
        crs,
        findings: findings.iter().map(interop_finding_feature).collect(),
    }
}

fn interop_finding_feature(finding: &FindingExportRecord) -> FindingsExportFeature {
    FindingsExportFeature {
        finding_id: finding.finding_id.clone(),
        zone_id: finding.zone.zone_id.clone(),
        reason: reason_code_str(finding.reason).to_string(),
        priority: priority_str(finding.priority).to_string(),
        area_m2: f64::from(finding.zone.area_m2),
        centroid: InteropCoordinate {
            x: finding.zone.centroid.0,
            y: finding.zone.centroid.1,
        },
        crs: finding.zone.crs.clone(),
        polygon: finding
            .zone
            .polygon
            .coordinates
            .iter()
            .map(|(x, y)| InteropCoordinate { x: *x, y: *y })
            .collect(),
        evidence_refs: normalized_evidence_refs(&finding.evidence_refs),
    }
}

fn normalized_evidence_refs(evidence_refs: &[String]) -> Vec<String> {
    evidence_refs
        .iter()
        .filter_map(|evidence_ref| {
            let evidence_ref = evidence_ref.trim();
            (!evidence_ref.is_empty()).then_some(evidence_ref.to_string())
        })
        .collect::<Vec<_>>()
}

fn reason_code_str(reason: ProductAnomalyReasonCode) -> &'static str {
    match reason {
        ProductAnomalyReasonCode::BelowAbsoluteThreshold => "below_absolute_threshold",
        ProductAnomalyReasonCode::AboveAbsoluteThreshold => "above_absolute_threshold",
        ProductAnomalyReasonCode::BelowStatisticalBand => "below_statistical_band",
        ProductAnomalyReasonCode::AboveStatisticalBand => "above_statistical_band",
    }
}

fn priority_str(priority: RecommendationPriority) -> &'static str {
    match priority {
        RecommendationPriority::Low => "low",
        RecommendationPriority::Medium => "medium",
        RecommendationPriority::High => "high",
        RecommendationPriority::Critical => "critical",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        export_findings_csv, export_findings_geojson, export_findings_shapefile,
        FindingExportRecord, FINDINGS_CSV_HEADER,
    };
    use crate::product_anomalies::ProductAnomalyReasonCode;
    use crate::zone_delineation::{AnomalyZone, AnomalyZonePolygon};
    use shared::schemas::RecommendationPriority;

    #[test]
    fn findings_csv_exports_header_and_matching_rows() {
        let csv =
            export_findings_csv(&[finding("finding-1", "zone-1")]).expect("csv export succeeds");
        let mut lines = csv.lines();

        assert_eq!(lines.next(), Some(FINDINGS_CSV_HEADER));
        assert_eq!(
            lines.next(),
            Some("finding-1,zone-1,below_absolute_threshold,high,3000.0,500010.000000,4500015.000000,EPSG:32614,zone:zone-1|layer:ndvi-2026-05-01")
        );
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn findings_geojson_exports_zone_geometry_and_properties() {
        let geojson = export_findings_geojson(&[finding("finding-1", "zone-1")])
            .expect("geojson export succeeds");
        let features = geojson["features"].as_array().expect("features array");
        let feature = &features[0];

        assert_eq!(geojson["type"], "FeatureCollection");
        assert_eq!(feature["type"], "Feature");
        assert_eq!(feature["id"], "finding-1");
        assert_eq!(feature["geometry"]["type"], "Polygon");
        assert_eq!(
            feature["geometry"]["coordinates"][0][0],
            serde_json::json!([500000.0, 4500020.0])
        );
        assert_eq!(feature["properties"]["zone_id"], "zone-1");
        assert_eq!(feature["properties"]["reason"], "below_absolute_threshold");
        assert_eq!(feature["properties"]["area_m2"], 3000.0);
        assert_eq!(feature["properties"]["priority"], "high");
        assert_eq!(feature["properties"]["crs"], "EPSG:32614");
    }

    #[test]
    fn empty_findings_export_as_valid_empty_csv_and_geojson() {
        let csv = export_findings_csv(&[]).expect("empty csv export succeeds");
        let geojson = export_findings_geojson(&[]).expect("empty geojson export succeeds");

        assert_eq!(csv.trim_end(), FINDINGS_CSV_HEADER);
        assert_eq!(geojson["type"], "FeatureCollection");
        assert!(geojson["features"]
            .as_array()
            .expect("features array")
            .is_empty());
    }

    #[test]
    fn findings_shapefile_routes_through_interop_and_supports_empty_export() {
        let shapefile = export_findings_shapefile(&[finding("finding-1", "zone-1")])
            .expect("shapefile export succeeds");
        assert_eq!(shapefile.crs, "EPSG:32614");
        assert_eq!(shapefile.feature_count, 1);
        assert!(!shapefile.files.shp.is_empty());
        assert!(!shapefile.files.shx.is_empty());
        assert!(!shapefile.files.dbf.is_empty());
        assert!(!shapefile.files.prj.is_empty());

        let empty = export_findings_shapefile(&[]).expect("empty shapefile export succeeds");
        assert_eq!(empty.feature_count, 0);
        assert!(empty.extent.is_none());
    }

    fn finding(finding_id: &str, zone_id: &str) -> FindingExportRecord {
        FindingExportRecord {
            finding_id: finding_id.to_string(),
            zone: AnomalyZone {
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
                area_m2: 3_000.0,
                centroid: (500010.0, 4500015.0),
                crs: "EPSG:32614".to_string(),
                evidence: Vec::new(),
            },
            reason: ProductAnomalyReasonCode::BelowAbsoluteThreshold,
            priority: RecommendationPriority::High,
            evidence_refs: vec![
                "zone:zone-1".to_string(),
                "layer:ndvi-2026-05-01".to_string(),
            ],
        }
    }
}
