use crate::product_anomalies::ProductAnomalyReasonCode;
use crate::zone_delineation::AnomalyZone;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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

pub fn export_findings_geojson(findings: &[FindingExportRecord]) -> Value {
    json!({
        "type": "FeatureCollection",
        "features": findings.iter().map(finding_feature).collect::<Vec<_>>(),
    })
}

fn finding_feature(finding: &FindingExportRecord) -> Value {
    json!({
        "type": "Feature",
        "id": finding.finding_id,
        "geometry": {
            "type": "Polygon",
            "coordinates": [finding.zone.polygon.coordinates
                .iter()
                .map(|(x, y)| json!([x, y]))
                .collect::<Vec<_>>()],
        },
        "properties": {
            "finding_id": finding.finding_id,
            "zone_id": finding.zone.zone_id,
            "reason": reason_code_str(finding.reason),
            "priority": priority_str(finding.priority),
            "area_m2": finding.zone.area_m2,
            "centroid_x": finding.zone.centroid.0,
            "centroid_y": finding.zone.centroid.1,
            "crs": finding.zone.crs,
            "evidence_refs": normalized_evidence_refs(&finding.evidence_refs),
        },
    })
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
        export_findings_csv, export_findings_geojson, FindingExportRecord, FINDINGS_CSV_HEADER,
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
        let geojson = export_findings_geojson(&[finding("finding-1", "zone-1")]);
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
        let geojson = export_findings_geojson(&[]);

        assert_eq!(csv.trim_end(), FINDINGS_CSV_HEADER);
        assert_eq!(geojson["type"], "FeatureCollection");
        assert!(geojson["features"]
            .as_array()
            .expect("features array")
            .is_empty());
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
