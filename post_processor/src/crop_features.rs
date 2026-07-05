//! Phenology feature vectors + tier-3 learned land-cover classification
//! (satellite pipeline batch 17).
//!
//! The design's classification plan tops out at tier 3: a model over
//! temporal (phenology) features, trained offline and inferred in Rust. This
//! module is the deterministic, testable core of that tier:
//! - **feature extraction** — a stable-ordered feature vector per
//!   feature-valid phenology pixel (the same vectors whether they feed
//!   training or inference);
//! - **training-sample export** — those vectors labeled from a reference
//!   map (WorldCover, via batch 16's class mapping) over comparably-labeled
//!   pixels, ready for an offline trainer;
//! - **a [`CropClassifier`] seam** with a deterministic [`NearestCentroidModel`]
//!   reference implementation, so the pipeline runs and tests end-to-end
//!   without shipping a trained model or an ONNX runtime. A real
//!   RF/GBM/ONNX model plugs in behind the same trait later.
//!
//! Honest scope: without WorldCereal crop-type labels the model classifies
//! into the same [`LandCoverClass`] space as tier 1 — but *learned* from the
//! full feature vector rather than hand-thresholded, and validatable by the
//! batch-16 agreement engine. Crop-type-proper (maize vs wheat) is future
//! work gated on crop-type training labels.

use crate::evidence::{deterministic_fingerprint, AnalysisEvidenceError};
use crate::landcover_agreement::ReferenceClassMap;
use crate::phenology::{LandCoverClass, PhenologyPixelReason, PhenologyResult};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable feature order. Changing this changes model identity — models
/// record the names they were fit on and inference checks them.
pub const FEATURE_NAMES: [&str; 8] = [
    "ndvi_min",
    "ndvi_max",
    "amplitude",
    "peak_doy",
    "sos_doy",
    "eos_doy",
    "season_length_days",
    "season_integral",
];
pub const FEATURE_COUNT: usize = FEATURE_NAMES.len();

/// The five learnable classes (Unknown/Invalid are never predicted). Order
/// is the deterministic tie-break for equidistant centroids.
pub const LEARNABLE_CLASSES: [LandCoverClass; 5] = [
    LandCoverClass::Water,
    LandCoverClass::BareOrSparse,
    LandCoverClass::AnnualCrop,
    LandCoverClass::TreeOrPerennial,
    LandCoverClass::Grassland,
];

/// Per-pixel feature vectors extracted from a phenology raster; `None` for
/// pixels without a full feature set (below-min-observations or flat
/// series, which lack SOS/EOS/season metrics).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureMatrix {
    pub width: u32,
    pub height: u32,
    pub feature_names: Vec<String>,
    /// Row-major; one `Some([f32; FEATURE_COUNT])` per feature-valid pixel.
    pub rows: Vec<Option<[f32; FEATURE_COUNT]>>,
}

impl FeatureMatrix {
    pub fn valid_count(&self) -> usize {
        self.rows.iter().filter(|row| row.is_some()).count()
    }
}

/// Extract the feature matrix from a phenology result. Only `Computed`
/// pixels (all metrics finite) get a vector; the rest are `None`.
pub fn extract_features(phenology: &PhenologyResult) -> FeatureMatrix {
    let pixel_count = phenology.width as usize * phenology.height as usize;
    let mut rows = Vec::with_capacity(pixel_count);
    for pixel in 0..pixel_count {
        if phenology.reason_codes[pixel] != PhenologyPixelReason::Computed {
            rows.push(None);
            continue;
        }
        let vector = [
            phenology.ndvi_min[pixel],
            phenology.ndvi_max[pixel],
            phenology.amplitude[pixel],
            phenology.peak_doy[pixel],
            phenology.sos_doy[pixel],
            phenology.eos_doy[pixel],
            phenology.season_length_days[pixel],
            phenology.season_integral[pixel],
        ];
        rows.push(if vector.iter().all(|v| v.is_finite()) {
            Some(vector)
        } else {
            None
        });
    }
    FeatureMatrix {
        width: phenology.width,
        height: phenology.height,
        feature_names: FEATURE_NAMES.iter().map(|s| s.to_string()).collect(),
        rows,
    }
}

/// One labeled training sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabeledSample {
    pub features: [f32; FEATURE_COUNT],
    pub label: LandCoverClass,
}

/// Training samples plus the reason-counted pixels that could not be
/// labeled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingSamples {
    pub samples: Vec<LabeledSample>,
    /// `no_features`, `reference_nodata`, `reference_unmapped`.
    pub excluded: std::collections::BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum CropFeatureError {
    #[error("feature matrix has {features} pixels, reference has {reference}")]
    LengthMismatch { features: usize, reference: usize },
    #[error("no labeled training samples could be built (all {excluded} pixels excluded)")]
    NoSamples { excluded: u32 },
    #[error("model was fit on features {fit:?} but inference features are {given:?}")]
    FeatureMismatch {
        fit: Vec<String>,
        given: Vec<String>,
    },
    #[error("evidence metadata failed: {0}")]
    Evidence(#[from] AnalysisEvidenceError),
}

/// Build labeled samples: feature-valid pixels whose reference code maps to
/// a learnable class. Same-grid alignment is the caller's responsibility.
pub fn build_training_samples(
    features: &FeatureMatrix,
    reference_codes: &[u8],
    reference_nodata: u8,
    class_map: &ReferenceClassMap,
) -> Result<TrainingSamples, CropFeatureError> {
    if features.rows.len() != reference_codes.len() {
        return Err(CropFeatureError::LengthMismatch {
            features: features.rows.len(),
            reference: reference_codes.len(),
        });
    }
    let mut samples = Vec::new();
    let mut excluded: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    let exclude = |reason: &str, map: &mut std::collections::BTreeMap<String, u32>| {
        *map.entry(reason.to_string()).or_insert(0) += 1;
    };
    for (row, code) in features.rows.iter().zip(reference_codes) {
        let Some(vector) = row else {
            exclude("no_features", &mut excluded);
            continue;
        };
        if *code == reference_nodata {
            exclude("reference_nodata", &mut excluded);
            continue;
        }
        match class_map.map(*code) {
            Some(label) => samples.push(LabeledSample {
                features: *vector,
                label,
            }),
            None => exclude("reference_unmapped", &mut excluded),
        }
    }
    if samples.is_empty() {
        return Err(CropFeatureError::NoSamples {
            excluded: excluded.values().sum(),
        });
    }
    Ok(TrainingSamples { samples, excluded })
}

/// Per-feature standardization (z-score) parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Standardizer {
    pub mean: [f32; FEATURE_COUNT],
    /// Population std; zero-variance features store 1 so standardization is
    /// a no-op there (avoids divide-by-zero, keeps the feature inert).
    pub std: [f32; FEATURE_COUNT],
}

impl Standardizer {
    fn fit(samples: &[LabeledSample]) -> Self {
        let n = samples.len() as f64;
        let mut mean = [0f64; FEATURE_COUNT];
        for sample in samples {
            for (acc, value) in mean.iter_mut().zip(&sample.features) {
                *acc += f64::from(*value);
            }
        }
        for acc in &mut mean {
            *acc /= n;
        }
        let mut var = [0f64; FEATURE_COUNT];
        for sample in samples {
            for (index, value) in sample.features.iter().enumerate() {
                let delta = f64::from(*value) - mean[index];
                var[index] += delta * delta;
            }
        }
        let mut mean_f = [0f32; FEATURE_COUNT];
        let mut std_f = [0f32; FEATURE_COUNT];
        for index in 0..FEATURE_COUNT {
            mean_f[index] = mean[index] as f32;
            let sd = (var[index] / n).sqrt();
            std_f[index] = if sd > 1e-9 { sd as f32 } else { 1.0 };
        }
        Self {
            mean: mean_f,
            std: std_f,
        }
    }

    fn apply(&self, features: &[f32; FEATURE_COUNT]) -> [f32; FEATURE_COUNT] {
        let mut out = [0f32; FEATURE_COUNT];
        for index in 0..FEATURE_COUNT {
            out[index] = (features[index] - self.mean[index]) / self.std[index];
        }
        out
    }
}

/// Inference seam: a fitted model classifies one standardized-or-raw feature
/// vector. A real ONNX/smartcore model implements this behind the trait.
pub trait CropClassifier {
    fn classify(&self, features: &[f32; FEATURE_COUNT]) -> LandCoverClass;
    /// Feature names the model expects, in order.
    fn feature_names(&self) -> &[String];
}

/// Deterministic nearest-centroid classifier over standardized features:
/// each class's centroid is the mean of its standardized training vectors;
/// a pixel takes the nearest centroid (Euclidean), ties broken by
/// [`LEARNABLE_CLASSES`] order. Serializable so a fitted model registers as
/// a catalog product and reproduces exactly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NearestCentroidModel {
    pub feature_names: Vec<String>,
    pub standardizer: Standardizer,
    /// (class, standardized centroid), one per class present in training.
    pub centroids: Vec<(LandCoverClass, [f32; FEATURE_COUNT])>,
    /// Deterministic fingerprint of the training set.
    pub training_hash: String,
    pub training_sample_count: usize,
}

impl NearestCentroidModel {
    /// Fit from labeled samples (deterministic; class order fixed by
    /// [`LEARNABLE_CLASSES`]).
    pub fn fit(samples: &[LabeledSample]) -> Result<Self, CropFeatureError> {
        if samples.is_empty() {
            return Err(CropFeatureError::NoSamples { excluded: 0 });
        }
        let standardizer = Standardizer::fit(samples);
        let mut centroids = Vec::new();
        for class in LEARNABLE_CLASSES {
            let members: Vec<[f32; FEATURE_COUNT]> = samples
                .iter()
                .filter(|s| s.label == class)
                .map(|s| standardizer.apply(&s.features))
                .collect();
            if members.is_empty() {
                continue;
            }
            let mut centroid = [0f64; FEATURE_COUNT];
            for member in &members {
                for (acc, value) in centroid.iter_mut().zip(member) {
                    *acc += f64::from(*value);
                }
            }
            let mut centroid_f = [0f32; FEATURE_COUNT];
            for index in 0..FEATURE_COUNT {
                centroid_f[index] = (centroid[index] / members.len() as f64) as f32;
            }
            centroids.push((class, centroid_f));
        }
        let training_hash = deterministic_fingerprint(&("nearest_centroid_v1", samples))?;
        Ok(Self {
            feature_names: FEATURE_NAMES.iter().map(|s| s.to_string()).collect(),
            standardizer,
            centroids,
            training_hash,
            training_sample_count: samples.len(),
        })
    }
}

impl CropClassifier for NearestCentroidModel {
    fn classify(&self, features: &[f32; FEATURE_COUNT]) -> LandCoverClass {
        let standardized = self.standardizer.apply(features);
        let mut best: Option<(LandCoverClass, f32)> = None;
        for (class, centroid) in &self.centroids {
            let distance: f32 = standardized
                .iter()
                .zip(centroid)
                .map(|(a, b)| (a - b) * (a - b))
                .sum();
            // Strictly-less keeps the first (LEARNABLE_CLASSES-ordered)
            // centroid on ties.
            if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                best = Some((*class, distance));
            }
        }
        best.map(|(class, _)| class)
            .unwrap_or(LandCoverClass::Unknown)
    }

    fn feature_names(&self) -> &[String] {
        &self.feature_names
    }
}

/// Classify every feature-valid pixel with `model`; `None` rows become
/// [`LandCoverClass::Invalid`]. Errors if the model's feature order differs
/// from this matrix's.
pub fn classify_raster<M: CropClassifier>(
    features: &FeatureMatrix,
    model: &M,
) -> Result<Vec<LandCoverClass>, CropFeatureError> {
    if model.feature_names() != features.feature_names.as_slice() {
        return Err(CropFeatureError::FeatureMismatch {
            fit: model.feature_names().to_vec(),
            given: features.feature_names.clone(),
        });
    }
    Ok(features
        .rows
        .iter()
        .map(|row| match row {
            Some(vector) => model.classify(vector),
            None => LandCoverClass::Invalid,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phenology::{compute_phenology, PhenologyObservation, PhenologyRequest};
    use chrono::NaiveDate;
    use shared::schemas::{GeoBounds, RasterResolution, RasterSpatialRef};

    fn spatial_ref() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32643".to_string()),
            bbox: Some(GeoBounds {
                min_lon: 600000.0,
                min_lat: 1300000.0,
                max_lon: 600020.0,
                max_lat: 1300020.0,
            }),
            geo_transform: Some([600000.0, 10.0, 0.0, 1300020.0, 0.0, -10.0]),
            resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
        }
    }

    /// A 2x2 phenology from a crop-pulse series shared by all pixels except
    /// pixel 3, which is flat (feature-invalid).
    fn phenology_2x2() -> PhenologyResult {
        let dates = [
            NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
        ];
        // pixels 0..3 crop pulse, pixel 3 flat.
        let series = [
            [0.2, 0.5, 0.8, 0.5, 0.2],
            [0.2, 0.5, 0.8, 0.5, 0.2],
            [0.2, 0.5, 0.8, 0.5, 0.2],
            [0.6, 0.6, 0.61, 0.6, 0.6],
        ];
        let observations = dates
            .iter()
            .enumerate()
            .map(|(i, date)| PhenologyObservation {
                product_id: format!("p{i}"),
                observed_on: *date,
                values: series.iter().map(|s| s[i]).collect(),
                valid_mask: vec![true; 4],
                spatial_ref: spatial_ref(),
            })
            .collect();
        compute_phenology(&PhenologyRequest {
            width: 2,
            height: 2,
            spatial_ref: spatial_ref(),
            observations,
            min_observations: 4,
            season_threshold_fraction: 0.5,
        })
        .unwrap()
    }

    #[test]
    fn features_extract_only_for_computed_pixels() {
        let features = extract_features(&phenology_2x2());
        assert_eq!(features.feature_names, FEATURE_NAMES);
        assert_eq!(features.valid_count(), 3, "pixel 3 is flat -> None");
        assert!(features.rows[3].is_none());
        let row = features.rows[0].unwrap();
        assert!((row[0] - 0.2).abs() < 1e-6); // ndvi_min
        assert!((row[2] - 0.6).abs() < 1e-6); // amplitude
    }

    #[test]
    fn training_samples_label_from_reference_and_count_exclusions() {
        let features = extract_features(&phenology_2x2());
        // crop 40, water 80, nodata 0, and pixel 3 has no features anyway.
        let reference = [40u8, 80, 0, 40];
        let training =
            build_training_samples(&features, &reference, 0, &ReferenceClassMap::default())
                .unwrap();
        assert_eq!(training.samples.len(), 2);
        assert_eq!(training.samples[0].label, LandCoverClass::AnnualCrop);
        assert_eq!(training.samples[1].label, LandCoverClass::Water);
        assert_eq!(training.excluded.get("reference_nodata"), Some(&1));
        assert_eq!(training.excluded.get("no_features"), Some(&1));
    }

    #[test]
    fn nearest_centroid_separates_two_well_spaced_classes() {
        // Two synthetic classes far apart in feature space.
        let mut samples = Vec::new();
        for _ in 0..5 {
            samples.push(LabeledSample {
                features: [0.1, 0.3, 0.2, 150.0, 100.0, 200.0, 100.0, 20.0],
                label: LandCoverClass::AnnualCrop,
            });
            samples.push(LabeledSample {
                features: [-0.5, -0.3, 0.2, 150.0, 100.0, 200.0, 100.0, 5.0],
                label: LandCoverClass::Water,
            });
        }
        let model = NearestCentroidModel::fit(&samples).unwrap();
        assert_eq!(model.centroids.len(), 2);
        // A vector near the crop centroid classifies crop; near water, water.
        assert_eq!(
            model.classify(&[0.1, 0.3, 0.2, 150.0, 100.0, 200.0, 100.0, 20.0]),
            LandCoverClass::AnnualCrop
        );
        assert_eq!(
            model.classify(&[-0.5, -0.3, 0.2, 150.0, 100.0, 200.0, 100.0, 5.0]),
            LandCoverClass::Water
        );
        // Deterministic identity.
        let again = NearestCentroidModel::fit(&samples).unwrap();
        assert_eq!(again.training_hash, model.training_hash);
    }

    #[test]
    fn classify_raster_marks_featureless_pixels_invalid() {
        let features = extract_features(&phenology_2x2());
        let samples = vec![
            LabeledSample {
                features: features.rows[0].unwrap(),
                label: LandCoverClass::AnnualCrop,
            },
            LabeledSample {
                features: [-0.5, -0.3, 0.2, 150.0, 100.0, 200.0, 100.0, 5.0],
                label: LandCoverClass::Water,
            },
        ];
        let model = NearestCentroidModel::fit(&samples).unwrap();
        let classes = classify_raster(&features, &model).unwrap();
        assert_eq!(classes.len(), 4);
        assert_eq!(classes[0], LandCoverClass::AnnualCrop);
        assert_eq!(classes[3], LandCoverClass::Invalid); // flat pixel
    }

    #[test]
    fn feature_name_mismatch_is_rejected() {
        let features = extract_features(&phenology_2x2());
        let samples = vec![LabeledSample {
            features: features.rows[0].unwrap(),
            label: LandCoverClass::AnnualCrop,
        }];
        let mut model = NearestCentroidModel::fit(&samples).unwrap();
        model.feature_names[0] = "renamed".to_string();
        assert!(matches!(
            classify_raster(&features, &model),
            Err(CropFeatureError::FeatureMismatch { .. })
        ));
    }
}
