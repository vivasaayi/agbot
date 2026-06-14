use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionPointKind {
    Index,
    Processor,
    ReportTemplate,
    MapLayer,
    AlertRule,
    ImportExportAdapter,
}

impl ExtensionPointKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ExtensionPointKind::Index => "index",
            ExtensionPointKind::Processor => "processor",
            ExtensionPointKind::ReportTemplate => "report_template",
            ExtensionPointKind::MapLayer => "map_layer",
            ExtensionPointKind::AlertRule => "alert_rule",
            ExtensionPointKind::ImportExportAdapter => "import_export_adapter",
        }
    }
}

impl std::str::FromStr for ExtensionPointKind {
    type Err = ExtensionPointKindParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "index" => Ok(Self::Index),
            "processor" => Ok(Self::Processor),
            "report_template" | "report-template" => Ok(Self::ReportTemplate),
            "map_layer" | "map-layer" => Ok(Self::MapLayer),
            "alert_rule" | "alert-rule" => Ok(Self::AlertRule),
            "import_export_adapter" | "import-export-adapter" => Ok(Self::ImportExportAdapter),
            _ => Err(ExtensionPointKindParseError {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionPointContract {
    pub kind: ExtensionPointKind,
    pub contract_signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown extension-point kind: {value}")]
pub struct ExtensionPointKindParseError {
    pub value: String,
}

pub fn extension_point_taxonomy() -> Vec<ExtensionPointContract> {
    vec![
        ExtensionPointContract {
            kind: ExtensionPointKind::Index,
            contract_signature:
                "IndexPlugin::evaluate(scene_ref, bands, parameters) -> RasterProduct".to_string(),
        },
        ExtensionPointContract {
            kind: ExtensionPointKind::Processor,
            contract_signature:
                "ProcessorPlugin::process(job_ref, inputs, parameters) -> ProcessorOutput"
                    .to_string(),
        },
        ExtensionPointContract {
            kind: ExtensionPointKind::ReportTemplate,
            contract_signature:
                "ReportTemplatePlugin::render(report_context, parameters) -> ReportDocument"
                    .to_string(),
        },
        ExtensionPointContract {
            kind: ExtensionPointKind::MapLayer,
            contract_signature:
                "MapLayerPlugin::layer(layer_context, parameters) -> MapLayerDescriptor".to_string(),
        },
        ExtensionPointContract {
            kind: ExtensionPointKind::AlertRule,
            contract_signature:
                "AlertRulePlugin::evaluate(alert_event, parameters) -> AlertFinding".to_string(),
        },
        ExtensionPointContract {
            kind: ExtensionPointKind::ImportExportAdapter,
            contract_signature:
                "ImportExportAdapterPlugin::adapt(direction, payload, parameters) -> AdapterResult"
                    .to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{extension_point_taxonomy, ExtensionPointKind};

    #[test]
    fn taxonomy_lists_exact_six_extension_points_with_signatures() {
        let taxonomy = extension_point_taxonomy();
        let kinds = taxonomy
            .iter()
            .map(|contract| contract.kind)
            .collect::<Vec<_>>();

        assert_eq!(
            kinds,
            vec![
                ExtensionPointKind::Index,
                ExtensionPointKind::Processor,
                ExtensionPointKind::ReportTemplate,
                ExtensionPointKind::MapLayer,
                ExtensionPointKind::AlertRule,
                ExtensionPointKind::ImportExportAdapter,
            ]
        );
        assert!(taxonomy
            .iter()
            .all(|contract| !contract.contract_signature.trim().is_empty()));
    }
}
