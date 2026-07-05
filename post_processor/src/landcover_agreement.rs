//! Tier-2 land-cover validation against reference maps (satellite pipeline
//! batch 16): agreement metrics between a tier-1 `landcover_rule` raster
//! and an ESA WorldCover-style reference raster on the same grid.
//!
//! The design's classification plan is tiered: tier 1 deterministic rules
//! (batch 10), tier 2 **bootstrap/validation from global reference maps**
//! (WorldCover, WorldCereal), tier 3 ML. This module is tier 2's
//! measurement half: map reference codes into our class space, build the
//! confusion matrix over comparably-labeled pixels, and report overall
//! agreement, per-class producer's/user's accuracy, and Cohen's kappa —
//! all deterministic and evidence-carrying. (The same reference rasters
//! later become training masks for tier 3.)
//!
//! Pixels are excluded — counted, never silently dropped — when our class
//! is `Invalid`/`Unknown`, the reference code is nodata, or the reference
//! code maps to no class of ours (e.g. WorldCover built-up 50: we have no
//! built class, so agreement there is unknowable, not wrong).

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};
use crate::phenology::LandCoverClass;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// ESA WorldCover v200 class codes (10 m, 2020/2021).
pub const WORLDCOVER_TREE: u8 = 10;
pub const WORLDCOVER_SHRUB: u8 = 20;
pub const WORLDCOVER_GRASS: u8 = 30;
pub const WORLDCOVER_CROPLAND: u8 = 40;
pub const WORLDCOVER_BUILT: u8 = 50;
pub const WORLDCOVER_BARE: u8 = 60;
pub const WORLDCOVER_SNOW: u8 = 70;
pub const WORLDCOVER_WATER: u8 = 80;
pub const WORLDCOVER_WETLAND: u8 = 90;
pub const WORLDCOVER_MANGROVE: u8 = 95;
pub const WORLDCOVER_MOSS: u8 = 100;

/// Reference-code -> our-class mapping. The default follows the WorldCover
/// legend semantics; every mapping choice is recorded in evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceClassMap {
    /// (reference code, mapped class) pairs; codes absent here are
    /// "unmapped" and excluded with a count.
    pub entries: Vec<(u8, LandCoverClass)>,
}

impl Default for ReferenceClassMap {
    fn default() -> Self {
        Self {
            entries: vec![
                (WORLDCOVER_TREE, LandCoverClass::TreeOrPerennial),
                (WORLDCOVER_MANGROVE, LandCoverClass::TreeOrPerennial),
                (WORLDCOVER_SHRUB, LandCoverClass::Grassland),
                (WORLDCOVER_GRASS, LandCoverClass::Grassland),
                (WORLDCOVER_CROPLAND, LandCoverClass::AnnualCrop),
                (WORLDCOVER_BARE, LandCoverClass::BareOrSparse),
                (WORLDCOVER_SNOW, LandCoverClass::BareOrSparse),
                (WORLDCOVER_MOSS, LandCoverClass::BareOrSparse),
                (WORLDCOVER_WATER, LandCoverClass::Water),
                (WORLDCOVER_WETLAND, LandCoverClass::Water),
                // Built-up (50) is deliberately unmapped: no built class.
            ],
        }
    }
}

impl ReferenceClassMap {
    pub fn map(&self, code: u8) -> Option<LandCoverClass> {
        self.entries
            .iter()
            .find(|(reference, _)| *reference == code)
            .map(|(_, class)| *class)
    }
}

/// Per-class accuracy line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassAccuracy {
    pub class: LandCoverClass,
    /// Pixels we labeled with this class (among compared pixels).
    pub ours: u32,
    /// Pixels the reference labels with this class.
    pub reference: u32,
    /// Pixels where both agree.
    pub agreeing: u32,
    /// agreeing / ours (a.k.a. user's accuracy / precision); None when
    /// we never predicted the class.
    pub users_accuracy: Option<f64>,
    /// agreeing / reference (a.k.a. producer's accuracy / recall); None
    /// when the reference never contains the class.
    pub producers_accuracy: Option<f64>,
}

/// A completed agreement report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgreementResult {
    /// Pixels compared (both sides comparable).
    pub compared_pixels: u32,
    /// Exclusions, by reason: `ours_invalid`, `ours_unknown`,
    /// `reference_nodata`, `reference_unmapped`.
    pub excluded: BTreeMap<String, u32>,
    /// (our class, reference-mapped class) -> count, over compared pixels.
    pub confusion: Vec<((LandCoverClass, LandCoverClass), u32)>,
    /// Diagonal fraction of the confusion matrix.
    pub overall_agreement: f64,
    /// Cohen's kappa over the shared class space.
    pub kappa: f64,
    pub per_class: Vec<ClassAccuracy>,
    pub class_map: ReferenceClassMap,
    pub input_hash: String,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum AgreementError {
    #[error("class rasters differ in length: ours {ours}, reference {reference}")]
    LengthMismatch { ours: usize, reference: usize },
    #[error("no comparable pixels (all {excluded} excluded)")]
    NoComparablePixels { excluded: u32 },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

/// The five comparable classes (Unknown/Invalid are excluded upstream).
const COMPARABLE: [LandCoverClass; 5] = [
    LandCoverClass::Water,
    LandCoverClass::BareOrSparse,
    LandCoverClass::AnnualCrop,
    LandCoverClass::TreeOrPerennial,
    LandCoverClass::Grassland,
];

/// Compare a tier-1 classification against reference codes on the same
/// grid. `reference_nodata` marks reference fill (WorldCover uses 0).
pub fn compare_landcover(
    ours: &[LandCoverClass],
    reference_codes: &[u8],
    reference_nodata: u8,
    class_map: &ReferenceClassMap,
) -> Result<AgreementResult, AgreementError> {
    if ours.len() != reference_codes.len() {
        return Err(AgreementError::LengthMismatch {
            ours: ours.len(),
            reference: reference_codes.len(),
        });
    }

    let mut excluded: BTreeMap<String, u32> = BTreeMap::new();
    let exclude = |reason: &str, map: &mut BTreeMap<String, u32>| {
        *map.entry(reason.to_string()).or_insert(0) += 1;
    };
    let mut confusion: BTreeMap<(usize, usize), u32> = BTreeMap::new();
    let mut compared = 0u32;
    let index_of = |class: LandCoverClass| COMPARABLE.iter().position(|c| *c == class);

    for (mine, code) in ours.iter().zip(reference_codes) {
        let our_index = match mine {
            LandCoverClass::Invalid => {
                exclude("ours_invalid", &mut excluded);
                continue;
            }
            LandCoverClass::Unknown => {
                exclude("ours_unknown", &mut excluded);
                continue;
            }
            class => index_of(*class).expect("comparable class"),
        };
        if *code == reference_nodata {
            exclude("reference_nodata", &mut excluded);
            continue;
        }
        let Some(reference_class) = class_map.map(*code) else {
            exclude("reference_unmapped", &mut excluded);
            continue;
        };
        let reference_index = index_of(reference_class).expect("map targets comparable classes");
        *confusion.entry((our_index, reference_index)).or_insert(0) += 1;
        compared += 1;
    }
    if compared == 0 {
        return Err(AgreementError::NoComparablePixels {
            excluded: excluded.values().sum(),
        });
    }

    let total = f64::from(compared);
    let count = |our: usize, reference: usize| {
        f64::from(confusion.get(&(our, reference)).copied().unwrap_or(0))
    };
    let agreeing: f64 = (0..COMPARABLE.len()).map(|i| count(i, i)).sum();
    let overall_agreement = agreeing / total;
    // Cohen's kappa: pe = sum over classes of (row marginal * col marginal).
    let expected: f64 = (0..COMPARABLE.len())
        .map(|class| {
            let row: f64 = (0..COMPARABLE.len()).map(|r| count(class, r)).sum();
            let col: f64 = (0..COMPARABLE.len()).map(|o| count(o, class)).sum();
            (row / total) * (col / total)
        })
        .sum();
    let kappa = if (1.0 - expected).abs() < f64::EPSILON {
        // Degenerate marginals (single class on both sides): agreement is
        // total by construction; report kappa 1 on full agreement, 0 else.
        if (overall_agreement - 1.0).abs() < f64::EPSILON {
            1.0
        } else {
            0.0
        }
    } else {
        (overall_agreement - expected) / (1.0 - expected)
    };

    let per_class = COMPARABLE
        .iter()
        .enumerate()
        .map(|(index, class)| {
            let ours_count: f64 = (0..COMPARABLE.len()).map(|r| count(index, r)).sum();
            let reference_count: f64 = (0..COMPARABLE.len()).map(|o| count(o, index)).sum();
            let agreeing = count(index, index);
            ClassAccuracy {
                class: *class,
                ours: ours_count as u32,
                reference: reference_count as u32,
                agreeing: agreeing as u32,
                users_accuracy: (ours_count > 0.0).then(|| agreeing / ours_count),
                producers_accuracy: (reference_count > 0.0).then(|| agreeing / reference_count),
            }
        })
        .collect();

    let confusion_out: Vec<((LandCoverClass, LandCoverClass), u32)> = confusion
        .iter()
        .map(|((our, reference), count)| ((COMPARABLE[*our], COMPARABLE[*reference]), *count))
        .collect();
    let input_hash = deterministic_fingerprint(&(
        "landcover_agreement_v1",
        ours,
        reference_codes,
        reference_nodata,
        &class_map.entries,
    ))?;

    Ok(AgreementResult {
        compared_pixels: compared,
        excluded,
        confusion: confusion_out,
        overall_agreement,
        kappa,
        per_class,
        class_map: class_map.clone(),
        input_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hand_computed_confusion_agreement_and_kappa() {
        // Ours:      [crop, water, tree,  grass]
        // Reference: [40,   80,    30,    30] -> [crop, water, grass, grass]
        // Agreement on 3 of 4; hand-computed kappa:
        //   po = 0.75
        //   pe = crop 0.25*0.25 + water 0.25*0.25 + tree 0.25*0 + grass 0.25*0.5
        //      = 0.0625 + 0.0625 + 0 + 0.125 = 0.25
        //   kappa = (0.75 - 0.25) / (1 - 0.25) = 2/3
        let ours = [
            LandCoverClass::AnnualCrop,
            LandCoverClass::Water,
            LandCoverClass::TreeOrPerennial,
            LandCoverClass::Grassland,
        ];
        let reference = [40u8, 80, 30, 30];
        let result =
            compare_landcover(&ours, &reference, 0, &ReferenceClassMap::default()).unwrap();

        assert_eq!(result.compared_pixels, 4);
        assert!(result.excluded.is_empty());
        assert!((result.overall_agreement - 0.75).abs() < 1e-12);
        assert!((result.kappa - 2.0 / 3.0).abs() < 1e-12, "{}", result.kappa);

        let tree = result
            .per_class
            .iter()
            .find(|line| line.class == LandCoverClass::TreeOrPerennial)
            .unwrap();
        assert_eq!(tree.ours, 1);
        assert_eq!(tree.reference, 0);
        assert_eq!(tree.users_accuracy, Some(0.0));
        assert_eq!(tree.producers_accuracy, None);

        let grass = result
            .per_class
            .iter()
            .find(|line| line.class == LandCoverClass::Grassland)
            .unwrap();
        assert_eq!(grass.users_accuracy, Some(1.0));
        assert_eq!(grass.producers_accuracy, Some(0.5));
    }

    #[test]
    fn exclusions_are_counted_by_reason_never_dropped() {
        let ours = [
            LandCoverClass::Invalid,
            LandCoverClass::Unknown,
            LandCoverClass::Water,
            LandCoverClass::Water,
            LandCoverClass::Water,
        ];
        // nodata 0, built-up 50 (unmapped), then real water.
        let reference = [80u8, 80, 0, 50, 80];
        let result =
            compare_landcover(&ours, &reference, 0, &ReferenceClassMap::default()).unwrap();
        assert_eq!(result.compared_pixels, 1);
        assert_eq!(result.excluded.get("ours_invalid"), Some(&1));
        assert_eq!(result.excluded.get("ours_unknown"), Some(&1));
        assert_eq!(result.excluded.get("reference_nodata"), Some(&1));
        assert_eq!(result.excluded.get("reference_unmapped"), Some(&1));
        assert!((result.overall_agreement - 1.0).abs() < 1e-12);
        // Single-class degenerate marginals: kappa reports 1 on full
        // agreement per the documented convention.
        assert!((result.kappa - 1.0).abs() < 1e-12);
    }

    #[test]
    fn worldcover_default_mapping_is_pinned() {
        let map = ReferenceClassMap::default();
        assert_eq!(map.map(10), Some(LandCoverClass::TreeOrPerennial));
        assert_eq!(map.map(95), Some(LandCoverClass::TreeOrPerennial));
        assert_eq!(map.map(20), Some(LandCoverClass::Grassland));
        assert_eq!(map.map(30), Some(LandCoverClass::Grassland));
        assert_eq!(map.map(40), Some(LandCoverClass::AnnualCrop));
        assert_eq!(map.map(60), Some(LandCoverClass::BareOrSparse));
        assert_eq!(map.map(80), Some(LandCoverClass::Water));
        assert_eq!(map.map(90), Some(LandCoverClass::Water));
        assert_eq!(map.map(50), None, "built-up must stay unmapped");
        assert_eq!(map.map(0), None);
    }

    #[test]
    fn no_comparable_pixels_is_an_error() {
        let ours = [LandCoverClass::Invalid, LandCoverClass::Unknown];
        let reference = [80u8, 80];
        assert!(matches!(
            compare_landcover(&ours, &reference, 0, &ReferenceClassMap::default()),
            Err(AgreementError::NoComparablePixels { excluded: 2 })
        ));
    }
}
