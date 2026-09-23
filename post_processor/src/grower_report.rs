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
    let mut layout = PdfLayout::new();
    for (index, line) in lines.iter().enumerate() {
        // The ninth entry is the map slot; user-supplied titles are text.
        if index == 8 {
            layout.boundary(&request.map_view_svg);
        } else {
            layout.line(
                line,
                index == 0 || line == "Findings" || line == "Recommendations",
            );
        }
    }
    Ok(layout.finish())
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
        "Map View".to_string(),
        "Findings".to_string(),
    ];

    if request.findings.is_empty() {
        lines.push("No supported anomaly findings for this observation. This does not establish field health.".to_string());
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
                "{} | {} | {} | {} | evidence {} | annotations {}",
                recommendation.recommendation_id,
                recommendation.title,
                priority_str(recommendation.priority),
                status_str(recommendation.status),
                nonempty_values(&recommendation.evidence_refs).join("|"),
                nonempty_values(&recommendation.annotation_ids).join("|")
            ));
            if let Some(note) = &recommendation.note {
                if !note.trim().is_empty() {
                    lines.push(format!("Action: {note}"));
                }
            }
        }
    }

    lines
}

// Standard PDF fonts do not accept UTF-8. Keep output deterministic and
// visibly replace unsupported glyphs instead of silently emitting corrupt text.
fn printable_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\u{2010}'..='\u{2015}' => '-',
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{00a0}' => ' ',
            ch if ch.is_ascii_graphic() || ch == ' ' => ch,
            ch if ch.is_whitespace() => ' ',
            _ => '?',
        })
        .collect()
}

fn wrap_text(value: &str, width: usize) -> Vec<String> {
    let value = printable_text(value);
    let mut result = Vec::new();
    let mut line = String::new();
    for word in value.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + word.len() > width {
            result.push(std::mem::take(&mut line));
        }
        let mut remainder = word;
        while remainder.len() > width {
            result.push(remainder[..width].to_string());
            remainder = &remainder[width..];
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(remainder);
    }
    if !line.is_empty() {
        result.push(line);
    }
    if result.is_empty() {
        result.push(String::new());
    }
    result
}

/// Deliberately supports only the untransformed polygon generated by
/// geo_hub::portal_overview::boundary_to_svg. This is not a general SVG renderer.
fn boundary_polygon(svg: &str) -> Option<Vec<(f64, f64)>> {
    if svg.contains("transform") || svg.matches("<polygon").count() != 1 || svg.contains("<path") {
        return None;
    }
    let polygon = svg.split_once("<polygon")?.1.split_once('>')?.0;
    let points = polygon.split_once("points=\"")?.1.split_once('"')?.0;
    let coordinates: Option<Vec<_>> = points
        .split_whitespace()
        .map(|point| {
            let (x, y) = point.split_once(',')?;
            let (x, y) = (x.parse::<f64>().ok()?, y.parse::<f64>().ok()?);
            (x.is_finite() && y.is_finite() && x.abs() <= 1_000_000.0 && y.abs() <= 1_000_000.0)
                .then_some((x, y))
        })
        .collect();
    let coordinates = coordinates?;
    if coordinates.len() < 3 || coordinates.len() > 10000 {
        return None;
    }
    let area: f64 = coordinates
        .iter()
        .zip(coordinates.iter().cycle().skip(1))
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum();
    (area.abs() > f64::EPSILON).then_some(coordinates)
}

struct PdfLayout {
    pages: Vec<String>,
    stream: String,
    y: f64,
}

impl PdfLayout {
    fn new() -> Self {
        Self {
            pages: Vec::new(),
            stream: String::new(),
            y: 744.0,
        }
    }
    fn ensure_space(&mut self, height: f64) {
        if self.y - height < 60.0 {
            self.pages.push(std::mem::take(&mut self.stream));
            self.y = 744.0;
        }
    }
    fn line(&mut self, text: &str, heading: bool) {
        let (size, width) = if heading { (12.0, 70) } else { (9.5, 86) };
        if heading {
            self.ensure_space(42.0);
            self.y -= 8.0;
        }
        for line in wrap_text(text, width) {
            self.ensure_space(14.0);
            self.stream.push_str(&format!(
                "BT /{} {size} Tf 48 {:.1} Td ({}) Tj ET\n",
                if heading { "F2" } else { "F1" },
                self.y,
                escape_pdf_text(&line)
            ));
            self.y -= 14.0;
        }
    }
    fn boundary(&mut self, svg: &str) {
        self.ensure_space(250.0);
        self.line("Map View", true);
        let Some(points) = boundary_polygon(svg) else {
            self.line(
                "Boundary preview unavailable. No satellite image is included.",
                false,
            );
            return;
        };
        self.ensure_space(220.0);
        self.line(
            "Field boundary (schematic; not satellite imagery or a survey)",
            false,
        );
        let min_x = points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let max_x = points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        let min_y = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let max_y = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        let scale = 160.0 / (max_x - min_x).max(max_y - min_y);
        let left = 48.0 + (516.0 - (max_x - min_x) * scale) / 2.0;
        let top = self.y - 10.0;
        self.stream
            .push_str("q 0.90 0.95 0.90 rg 0.18 0.43 0.24 RG 1.5 w\n");
        for (i, (x, y)) in points.iter().enumerate() {
            self.stream.push_str(&format!(
                "{:.2} {:.2} {}\n",
                left + (x - min_x) * scale,
                top - (y - min_y) * scale,
                if i == 0 { "m" } else { "l" }
            ));
        }
        self.stream.push_str(" h B\nQ\n");
        self.y -= 185.0;
    }
    fn finish(mut self) -> Vec<u8> {
        self.pages.push(self.stream);
        let count = self.pages.len();
        for (i, stream) in self.pages.iter_mut().enumerate() {
            stream.push_str(&format!(
                "BT /F1 9 Tf 48 32 Td (AGBot field report - Page {} of {count}) Tj ET\n",
                i + 1
            ));
        }
        render_pdf_pages(&self.pages)
    }
}

fn render_pdf_pages(streams: &[String]) -> Vec<u8> {
    let kids = (0..streams.len())
        .map(|i| format!("{} 0 R", 5 + i * 2))
        .collect::<Vec<_>>()
        .join(" ");
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", streams.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>".to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Courier-Bold >>".to_string(),
    ];
    for (i, stream) in streams.iter().enumerate() {
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",6+i*2));
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            stream
        ));
    }
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{}\nendobj\n", index + 1, object));
    }
    let xref_start = pdf.len();
    pdf.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objects.len() + 1
    ));
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
        assert!(text.contains("ann-01"));
        assert!(text.contains("ann-02"));
        assert!(text.contains("%%EOF"));
    }

    #[test]
    fn pdf_draws_supported_boundary_instead_of_printing_svg() {
        let mut request = request();
        request.map_view_svg =
            r#"<svg viewBox="0 0 200 200"><polygon points="10,190 190,190 190,10 10,10"/></svg>"#
                .into();
        let pdf = render_grower_ready_pdf(&request).unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(!text.contains("<svg"));
        assert!(text.contains("Field boundary"));
        assert!(
            text.contains(" h B"),
            "expected a filled and stroked vector outline"
        );
        assert!(text.contains("Check irrigation and re-scout in 48h"));
        if let Ok(path) = std::env::var("AGBOT_PDF_REVIEW_OUTPUT") {
            std::fs::write(path, pdf).unwrap();
        }
    }

    #[test]
    fn pdf_paginates_long_reports_and_wraps_long_identifiers() {
        let mut request = request();
        request.scene.layer_refs = vec!["source-".repeat(90)];
        request.recommendations = (0..35)
            .map(|i| {
                let mut item = recommendation();
                item.title = format!(
                    "Recommendation {i}: inspect water delivery and compare scouting observations."
                );
                item
            })
            .collect();
        let pdf = render_grower_ready_pdf(&request).unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(!text.contains("/Count 1 "));
        assert!(text.contains("Recommendation 34"));
        assert!(!text.contains(&"source-".repeat(90)));
        assert!(text.contains("Boundary preview unavailable"));
        assert!(!text.contains("<svg"));
        if let Ok(path) = std::env::var("AGBOT_PDF_REVIEW_LONG_OUTPUT") {
            std::fs::write(path, pdf).unwrap();
        }
    }

    #[test]
    fn pdf_text_is_ascii_safe_and_unsupported_svg_is_not_claimed_as_map() {
        let mut request = request();
        request.title = "Farmer’s report – field 日本".into();
        request.map_view_svg =
            r#"<svg><g transform="rotate(45)"><polygon points="0,0 5,0 5,5"/></g></svg>"#.into();
        let pdf = render_grower_ready_pdf(&request).unwrap();
        assert!(pdf.is_ascii());
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("Farmer's report - field ??"));
        assert!(text.contains("Boundary preview unavailable"));
        assert!(!text.contains(" h B"));
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
            annotation_ids: vec!["ann-01".to_string(), "ann-02".to_string()],
            created_at: "2026-05-01T00:00:00Z".to_string(),
            updated_at: "2026-05-01T00:00:00Z".to_string(),
        }
    }
}
