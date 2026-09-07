//! Crop-type (species) classification domain model (foundation batch).
//!
//! The founding satellite-pipeline goal asked to classify crop *species*
//! (maize, cereals, …), not just land-cover *type* (crop vs tree vs grass).
//! This module is the grounded domain foundation, modeled on ESA
//! WorldCereal — the reference global crop-mapping system
//! (`docs/design/crop-type-classification.md` cites the sources):
//!
//! - [`CropType`] — the crop classes WorldCereal actually publishes
//!   (temporary crops, winter/spring cereals, maize) plus `OtherCrop`.
//!   WorldCereal delivers these as per-crop **binary presence** maps with a
//!   confidence band, so this is the class space of a set of presence
//!   classifiers, not an exhaustive taxonomy.
//! - [`WorldCerealSeason`] — the five verified season identifiers the crop
//!   calendars define; a crop-type feature set is computed over a season's
//!   temporal window.
//! - [`EwocCode`] + [`CropTypeLegend`] — the WorldCereal RDM reference code
//!   (`ewoc_code`, int64, HCAT-derived) and its mapping to [`CropType`].
//!   The full code table is **not** in the public docs — it ships as a CSV
//!   in the RDM. So the legend is DATA-DRIVEN (built from parsed rows),
//!   never a fabricated hard-coded table.
//! - [`ClassLabel`] — a label-space abstraction implemented by both
//!   `CropType` and the existing [`crate::phenology::LandCoverClass`],
//!   proving one classifier can serve both without a rewrite (the next
//!   batch generalizes `NearestCentroidModel` over this trait).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::phenology::LandCoverClass;

/// A classification label space: a fixed, code-identified, named set of
/// classes a classifier can predict. Implemented by both land-cover and
/// crop-type label sets so one nearest-centroid model serves either.
pub trait ClassLabel: Copy + Eq + 'static {
    /// Stable numeric code (also the on-raster pixel value where the label
    /// is written). Distinct across a label space's [`members`](Self::members).
    fn code(&self) -> u16;
    /// Stable snake_case name for evidence/parameters.
    fn name(&self) -> &'static str;
    /// Every predictable member of the space, in a deterministic order
    /// (the tie-break order for equidistant centroids).
    fn members() -> &'static [Self]
    where
        Self: Sized;

    /// The member with `code`, if any.
    fn from_code(code: u16) -> Option<Self>
    where
        Self: Sized,
    {
        Self::members().iter().copied().find(|m| m.code() == code)
    }
}

/// Crop classes WorldCereal maps (per-crop binary presence + confidence).
/// Codes are the on-raster pixel values for a combined crop-type map; they
/// are AGBot-internal (WorldCereal ships one binary raster per crop, not a
/// coded multi-class raster).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CropType {
    /// Temporary (annual) cropland — the `tc-annual` extent class.
    TemporaryCrops,
    /// Winter cereals (wheat/barley/rye winter varieties).
    WinterCereals,
    /// Spring cereals (northern-hemisphere spring varieties).
    SpringCereals,
    /// Maize.
    Maize,
    /// A crop pixel that is cropland but not one of the mapped types.
    OtherCrop,
}

/// Every crop class, in code order (the deterministic classifier tie-break).
pub const CROP_TYPES: [CropType; 5] = [
    CropType::TemporaryCrops,
    CropType::WinterCereals,
    CropType::SpringCereals,
    CropType::Maize,
    CropType::OtherCrop,
];

impl CropType {
    /// Stable pixel/class code.
    pub fn code(self) -> u16 {
        match self {
            CropType::TemporaryCrops => 10,
            CropType::WinterCereals => 20,
            CropType::SpringCereals => 30,
            CropType::Maize => 40,
            CropType::OtherCrop => 90,
        }
    }

    /// Stable snake_case name.
    pub fn name(self) -> &'static str {
        match self {
            CropType::TemporaryCrops => "temporary_crops",
            CropType::WinterCereals => "winter_cereals",
            CropType::SpringCereals => "spring_cereals",
            CropType::Maize => "maize",
            CropType::OtherCrop => "other_crop",
        }
    }

    /// True for the specific cereal/maize types (not the generic
    /// temporary-crop extent or catch-all other).
    pub fn is_specific_crop(self) -> bool {
        matches!(
            self,
            CropType::WinterCereals | CropType::SpringCereals | CropType::Maize
        )
    }
}

impl ClassLabel for CropType {
    fn code(&self) -> u16 {
        CropType::code(*self)
    }
    fn name(&self) -> &'static str {
        CropType::name(*self)
    }
    fn members() -> &'static [Self] {
        &CROP_TYPES
    }
}

impl ClassLabel for LandCoverClass {
    fn code(&self) -> u16 {
        // LandCoverClass::class_code is 1..=6 (None only for Invalid, which
        // is never a predictable member); map the never-predicted Invalid
        // to 0 so the trait stays total.
        u16::from(self.class_code().unwrap_or(0))
    }
    fn name(&self) -> &'static str {
        match self {
            LandCoverClass::Water => "water",
            LandCoverClass::BareOrSparse => "bare_or_sparse",
            LandCoverClass::AnnualCrop => "annual_crop",
            LandCoverClass::TreeOrPerennial => "tree_or_perennial",
            LandCoverClass::Grassland => "grassland",
            LandCoverClass::Unknown => "unknown",
            LandCoverClass::Invalid => "invalid",
        }
    }
    fn members() -> &'static [Self] {
        &crate::crop_features::LEARNABLE_CLASSES
    }
}

/// The five WorldCereal season identifiers (crop calendars, per AEZ). A
/// crop-type feature set is computed over a season's SOS→EOS window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldCerealSeason {
    /// One-year temporary-crop cycle.
    Annual,
    /// Main (winter) cereals season.
    WinterCereals,
    /// Optional spring-cereals season (northern AEZs).
    SpringCereals,
    /// Main maize season.
    MaizeMain,
    /// Optional second maize season (tropical multi-cycle AEZs).
    MaizeSecond,
}

/// Every season, in canonical order.
pub const WORLDCEREAL_SEASONS: [WorldCerealSeason; 5] = [
    WorldCerealSeason::Annual,
    WorldCerealSeason::WinterCereals,
    WorldCerealSeason::SpringCereals,
    WorldCerealSeason::MaizeMain,
    WorldCerealSeason::MaizeSecond,
];

impl WorldCerealSeason {
    /// The WorldCereal token (e.g. `tc-annual`).
    pub fn token(self) -> &'static str {
        match self {
            WorldCerealSeason::Annual => "tc-annual",
            WorldCerealSeason::WinterCereals => "tc-wintercereals",
            WorldCerealSeason::SpringCereals => "tc-springcereals",
            WorldCerealSeason::MaizeMain => "tc-maize-main",
            WorldCerealSeason::MaizeSecond => "tc-maize-second",
        }
    }

    /// Parse a WorldCereal season token.
    pub fn from_token(token: &str) -> Option<Self> {
        WORLDCEREAL_SEASONS
            .into_iter()
            .find(|season| season.token() == token)
    }

    /// The crop type this season is primarily used to map, if a specific
    /// one (Annual maps the generic temporary-crop extent).
    pub fn primary_crop(self) -> CropType {
        match self {
            WorldCerealSeason::Annual => CropType::TemporaryCrops,
            WorldCerealSeason::WinterCereals => CropType::WinterCereals,
            WorldCerealSeason::SpringCereals => CropType::SpringCereals,
            WorldCerealSeason::MaizeMain | WorldCerealSeason::MaizeSecond => CropType::Maize,
        }
    }
}

/// A WorldCereal RDM reference code (`ewoc_code`, int64). Opaque: the
/// public docs describe it as "5 numeric parts in one number" (HCAT/
/// EuroCrops-derived land-cover → crop-group → crop-type hierarchy) but do
/// NOT publish the digit semantics, so this crate never decodes it — it is
/// a key into a data-driven [`CropTypeLegend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EwocCode(pub i64);

/// Data-driven `ewoc_code → CropType` mapping. Built from the WorldCereal
/// RDM legend CSV (`WorldCereal_LC_CT_legend_latest.pdf`/CSV), which is the
/// authoritative table the public web docs do not enumerate. Never
/// pre-populated with a fabricated table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CropTypeLegend {
    entries: BTreeMap<i64, CropType>,
}

impl CropTypeLegend {
    /// An empty legend (operator loads the CSV rows into it).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Register a code → crop mapping (overwrites any prior mapping for the
    /// code). Returns the previous mapping, if any.
    pub fn insert(&mut self, code: EwocCode, crop: CropType) -> Option<CropType> {
        self.entries.insert(code.0, crop)
    }

    /// The crop a code maps to, if the legend covers it.
    pub fn map(&self, code: EwocCode) -> Option<CropType> {
        self.entries.get(&code.0).copied()
    }

    /// Number of registered codes.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Build from `(ewoc_code, crop_type_name)` rows (e.g. parsed CSV).
    /// Unknown crop names are collected as errors so a malformed legend is
    /// caught, not silently dropped.
    pub fn from_rows<I>(rows: I) -> Result<Self, CropLegendError>
    where
        I: IntoIterator<Item = (i64, String)>,
    {
        let mut legend = Self::empty();
        for (code, name) in rows {
            match crop_type_from_name(&name) {
                Some(crop) => {
                    legend.insert(EwocCode(code), crop);
                }
                None => return Err(CropLegendError::UnknownCropName { code, name }),
            }
        }
        Ok(legend)
    }
}

/// Parse a crop-type name (as it appears in a legend row) into a [`CropType`].
pub fn crop_type_from_name(name: &str) -> Option<CropType> {
    match name.trim().to_ascii_lowercase().as_str() {
        "temporary_crops" | "temporary crops" | "annual_cropland" | "annual cropland" => {
            Some(CropType::TemporaryCrops)
        }
        "winter_cereals" | "winter cereals" => Some(CropType::WinterCereals),
        "spring_cereals" | "spring cereals" => Some(CropType::SpringCereals),
        "maize" | "corn" => Some(CropType::Maize),
        "other_crop" | "other" => Some(CropType::OtherCrop),
        _ => None,
    }
}

/// Error parsing a crop-type legend.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CropLegendError {
    #[error("legend row for ewoc_code {code} has unknown crop name {name:?}")]
    UnknownCropName { code: i64, name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_type_codes_and_names_are_pinned_and_distinct() {
        let codes: Vec<u16> = CROP_TYPES.iter().map(|c| c.code()).collect();
        assert_eq!(codes, vec![10, 20, 30, 40, 90]);
        // Codes and names round-trip through the ClassLabel trait.
        for crop in CROP_TYPES {
            assert_eq!(<CropType as ClassLabel>::from_code(crop.code()), Some(crop));
            assert!(!crop.name().is_empty());
        }
        assert!(CropType::Maize.is_specific_crop());
        assert!(!CropType::TemporaryCrops.is_specific_crop());
        assert!(!CropType::OtherCrop.is_specific_crop());
    }

    #[test]
    fn class_label_abstraction_serves_both_label_spaces() {
        // CropType and LandCoverClass are two label spaces of one trait.
        assert_eq!(<CropType as ClassLabel>::members().len(), 5);
        assert_eq!(<LandCoverClass as ClassLabel>::members().len(), 5);
        // LandCoverClass codes stay 1..=6 (its established raster codes).
        assert_eq!(ClassLabel::code(&LandCoverClass::AnnualCrop), 3);
        assert_eq!(ClassLabel::name(&LandCoverClass::AnnualCrop), "annual_crop");
        assert_eq!(
            <LandCoverClass as ClassLabel>::from_code(3),
            Some(LandCoverClass::AnnualCrop)
        );
        // The two spaces do not collide on the trait: crop codes are 10+.
        assert!(CROP_TYPES.iter().all(|c| ClassLabel::code(c) >= 10));
    }

    #[test]
    fn worldcereal_season_tokens_are_verified_and_round_trip() {
        let tokens: Vec<&str> = WORLDCEREAL_SEASONS.iter().map(|s| s.token()).collect();
        assert_eq!(
            tokens,
            vec![
                "tc-annual",
                "tc-wintercereals",
                "tc-springcereals",
                "tc-maize-main",
                "tc-maize-second"
            ]
        );
        for season in WORLDCEREAL_SEASONS {
            assert_eq!(WorldCerealSeason::from_token(season.token()), Some(season));
        }
        assert_eq!(WorldCerealSeason::from_token("tc-bogus"), None);
        assert_eq!(
            WorldCerealSeason::MaizeSecond.primary_crop(),
            CropType::Maize
        );
    }

    #[test]
    fn legend_is_data_driven_and_maps_the_documented_example() {
        // An empty legend maps nothing (never a fabricated default table).
        let empty = CropTypeLegend::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.map(EwocCode(1111020036)), None);

        // The documented example ewoc_code, once loaded, maps as configured.
        let legend = CropTypeLegend::from_rows([
            (1111020036, "maize".to_string()),
            (1110000000, "winter_cereals".to_string()),
        ])
        .expect("valid rows");
        assert_eq!(legend.len(), 2);
        assert_eq!(legend.map(EwocCode(1111020036)), Some(CropType::Maize));
        assert_eq!(
            legend.map(EwocCode(1110000000)),
            Some(CropType::WinterCereals)
        );
        assert_eq!(legend.map(EwocCode(9999999999)), None);
    }

    #[test]
    fn malformed_legend_rows_are_typed_errors_not_silent_drops() {
        let err = CropTypeLegend::from_rows([(1234, "rutabaga".to_string())]).unwrap_err();
        assert_eq!(
            err,
            CropLegendError::UnknownCropName {
                code: 1234,
                name: "rutabaga".to_string()
            }
        );
        // Case/space-insensitive parsing of the accepted names.
        assert_eq!(
            crop_type_from_name(" Winter Cereals "),
            Some(CropType::WinterCereals)
        );
        assert_eq!(crop_type_from_name("corn"), Some(CropType::Maize));
    }
}
