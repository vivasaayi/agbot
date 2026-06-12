use crate::findings_export::FindingExportRecord;
use crate::product_anomalies::ProductAnomalyReasonCode;
use serde::{Deserialize, Serialize};
use shared::schemas::{RecommendationPriority, RecommendationRecord, RecommendationStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldReportMetadata {
    pub field_id: String,
    pub field_name: String,
    pub org_id: String,
    pub season_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneReportMetadata {
    pub scene_id: String,
    pub captured_at: String,
    pub layer_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrowerReportRequest {
    pub report_id: String,
    pub title: String,
    pub field: FieldReportMetadata,
    pub scene: SceneReportMetadata,
    pub map_view_svg: String,
    #[serde(default)]
    pub findings: Vec<FindingExportRecord>,
    #[serde(default)]
    pub recommendations: Vec<RecommendationRecord>,
    pub generated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GrowerReportError {
    #[error("report requires field metadata")]
    MissingFieldMetadata,
    #[error("report requires scene metadata")]
    MissingSceneMetadata,
    #[error("report requires at least one layer source")]
    MissingLayerSource,
    #[error("report requires at least one map view")]
    MissingMapView,
    #[error("report requires a generation timestamp")]
    MissingGeneratedAt,
}

pub fn render_grower_ready_pdf(
    request: &GrowerReportRequest,
) -> Result<Vec<u8>, GrowerReportError> {
    validate_grower_report_request(request)?;
    let lines = grower_report_lines(request);
    Ok(render_pdf_lines(&lines))
}

fn validate_grower_report_request(request: &GrowerReportRequest) -> Result<(), GrowerReportError> {
    if request.report_id.trim().is_empty()
        || request.title.trim().is_empty()
        || request.field.field_id.trim().is_empty()
        || request.field.field_name.trim().is_empty()
        || request.field.org_id.trim().is_empty()
        || request.field.season_id.trim().is_empty()
    {
        return Err(GrowerReportError::MissingFieldMetadata);
    }
    if request.scene.scene_id.trim().is_empty() || request.scene.captured_at.trim().is_empty() {
        return Err(GrowerReportError::MissingSceneMetadata);
    }
    if request
        .scene
        .layer_refs
        .iter()
        .all(|layer| layer.trim().is_empty())
    {
        return Err(GrowerReportError::MissingLayerSource);
    }
    if request.map_view_svg.trim().is_empty() {
        return Err(GrowerReportError::MissingMapView);
    }
    if request.generated_at.trim().is_empty() {
        return Err(GrowerReportError::MissingGeneratedAt);
    }
    Ok(())
}

fn grower_report_lines(request: &GrowerReportRequest) -> Vec<String> {
    let mut lines = vec![
        request.title.clone(),
        format!("Report ID: {}", request.report_id),
        format!("Generated: {}", request.generated_at),
        format!(
            "Field: {} ({})",
            request.field.field_name, request.field.field_id
        ),
        format!("Organization: {}", request.field.org_id),
        format!("Season: {}", request.field.season_id),
        format!(
            "Scene: {} captured {}",
            request.scene.scene_id, request.scene.captured_at
        ),
        format!(
            "Layer Sources: {}",
            nonempty_values(&request.scene.layer_refs).join(", ")
        ),
        format!("Map View: {}", compact_text(&request.map_view_svg)),
        "Findings".to_string(),
    ];

    if request.findings.is_empty() {
        lines.push("No findings exported.".to_string());
    } else {
        for finding in &request.findings {
            lines.push(format!(
                "{} | zone {} | {} | {} | {:.1} m2 | evidence {}",
                finding.finding_id,
                finding.zone.zone_id,
                reason_code_str(finding.reason),
                priority_str(finding.priority),
                finding.zone.area_m2,
                nonempty_values(&finding.evidence_refs).join("|")
            ));
        }
    }

    lines.push("Recommendations".to_string());
    if request.recommendations.is_empty() {
        lines.push("No recommendations exported.".to_string());
    } else {
        for recommendation in &request.recommendations {
            lines.push(format!(
                "{} | {} | {} | {} | evidence {}",
                recommendation.recommendation_id,
                recommendation.title,
                priority_str(recommendation.priority),
                status_str(recommendation.status),
                nonempty_values(&recommendation.evidence_refs).join("|")
            ));
        }
    }

    lines
}

fn render_pdf_lines(lines: &[String]) -> Vec<u8> {
    let mut stream = String::from("BT\n/F1 10 Tf\n72 760 Td\n14 TL\n");
    for line in lines {
        stream.push_str(&format!("({}) Tj\nT*\n", escape_pdf_text(line)));
    }
    stream.push_str("ET\n");

    let objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        format!("<< /Length {} >>\nstream\n{}endstream", stream.len(), stream),
    ];

    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objects.len() + 1);
    offsets.push(0usize);
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{}\nendobj\n", index + 1, object));
    }
    let xref_start = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n", objects.len() + 1));
    pdf.push_str("0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        objects.len() + 1,
        xref_start
    ));
    pdf.into_bytes()
}

fn compact_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn nonempty_values(values: &[String]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| {
            let value = value.trim();
            (!value.is_empty()).then_some(value.to_string())
        })
        .collect::<Vec<_>>()
}

fn escape_pdf_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
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

fn status_str(status: RecommendationStatus) -> &'static str {
    match status {
        RecommendationStatus::Open => "open",
        RecommendationStatus::Reviewed => "reviewed",
        RecommendationStatus::Completed => "completed",
        RecommendationStatus::Dismissed => "dismissed",
        RecommendationStatus::Closed => "closed",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        render_grower_ready_pdf, FieldReportMetadata, GrowerReportError, GrowerReportRequest,
        SceneReportMetadata,
    };
    use crate::findings_export::FindingExportRecord;
    use crate::product_anomalies::ProductAnomalyReasonCode;
    use crate::zone_delineation::{AnomalyZone, AnomalyZonePolygon};
    use shared::schemas::{RecommendationPriority, RecommendationRecord, RecommendationStatus};

    #[test]
    fn grower_ready_pdf_contains_field_map_findings_and_recommendations() {
        let pdf = render_grower_ready_pdf(&request()).expect("pdf renders");
        let text = String::from_utf8_lossy(&pdf);

        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("North Field"));
        assert!(text.contains("scene-1"));
        assert!(text.contains("Map View"));
        assert!(text.contains("finding-1"));
        assert!(text.contains("zone-1"));
        assert!(text.contains("Scout anomaly zone zone-1"));
        assert!(text.contains("layer:ndvi-2026-05-01"));
        assert!(text.contains("%%EOF"));
    }

    #[test]
    fn grower_ready_pdf_rejects_missing_field_metadata() {
        let mut request = request();
        request.field.field_name.clear();

        let error =
            render_grower_ready_pdf(&request).expect_err("missing field metadata is rejected");

        assert_eq!(error, GrowerReportError::MissingFieldMetadata);
    }

    #[test]
    fn grower_ready_pdf_rejects_missing_map_view() {
        let mut request = request();
        request.map_view_svg.clear();

        let error = render_grower_ready_pdf(&request).expect_err("missing map view is rejected");

        assert_eq!(error, GrowerReportError::MissingMapView);
    }

    fn request() -> GrowerReportRequest {
        GrowerReportRequest {
            report_id: "report-1".to_string(),
            title: "North Field Scout Report".to_string(),
            field: FieldReportMetadata {
                field_id: "field-1".to_string(),
                field_name: "North Field".to_string(),
                org_id: "org-a".to_string(),
                season_id: "season-2026".to_string(),
            },
            scene: SceneReportMetadata {
                scene_id: "scene-1".to_string(),
                captured_at: "2026-05-01T00:00:00Z".to_string(),
                layer_refs: vec!["layer:ndvi-2026-05-01".to_string()],
            },
            map_view_svg: "<svg><text>Map View</text></svg>".to_string(),
            findings: vec![finding()],
            recommendations: vec![recommendation()],
            generated_at: "2026-05-02T00:00:00Z".to_string(),
        }
    }

    fn finding() -> FindingExportRecord {
        FindingExportRecord {
            finding_id: "finding-1".to_string(),
            zone: AnomalyZone {
                zone_id: "zone-1".to_string(),
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

    fn recommendation() -> RecommendationRecord {
        RecommendationRecord {
            recommendation_id: "rec-zone-1".to_string(),
            scene_id: "scene-1".to_string(),
            field_id: Some("field-1".to_string()),
            org_id: "org-a".to_string(),
            author_user_id: "advisor-1".to_string(),
            title: "Scout anomaly zone zone-1".to_string(),
            note: Some("Check irrigation and re-scout in 48h".to_string()),
            category: Some("scout".to_string()),
            action_category: "scout".to_string(),
            priority: RecommendationPriority::High,
            status: RecommendationStatus::Open,
            evidence_refs: vec![
                "zone:zone-1".to_string(),
                "layer:ndvi-2026-05-01".to_string(),
            ],
            annotation_ids: Vec::new(),
            created_at: "2026-05-01T00:00:00Z".to_string(),
            updated_at: "2026-05-01T00:00:00Z".to_string(),
        }
    }
}
