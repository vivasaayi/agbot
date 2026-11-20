//! Derive explorer point readings from the archive's named, unscaled pixel samples.
use crate::earth_search::landsat_asset_key;
use crate::landsat_derive::{
    cloud_qa_clear, instrument_for_scene, qa_aerosol_acceptable, qa_pixel_clear,
    qa_radsat_reject_mask, LandsatInstrument,
};
use imagery_processor::pipeline::calibration::{apply_radiometric_scaling, SensorProfile};
use imagery_processor::{IndexBandRole, IndexBandValues, IndexKind, SensorPreset};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub(crate) struct PointSample {
    band_names: Vec<String>,
    values: Vec<Option<f64>>,
}

impl PointSample {
    fn dn(&self, asset: &str) -> Option<u16> {
        let name = format!("{asset}_b1");
        let i = self.band_names.iter().position(|band| band == &name)?;
        let v = self.values.get(i).copied().flatten()?;
        (v.fract() == 0.0 && (0.0..=u16::MAX as f64).contains(&v)).then_some(v as u16)
    }
}

#[derive(Serialize)]
pub struct Observation {
    item: String,
    indices: BTreeMap<&'static str, Option<f32>>,
}

pub(crate) const ROLES: [IndexBandRole; 4] = [
    IndexBandRole::Green,
    IndexBandRole::Nir,
    IndexBandRole::Red,
    IndexBandRole::Swir1,
];

pub(crate) fn asset(role: IndexBandRole, sentinel: bool) -> &'static str {
    if sentinel {
        SensorPreset::Sentinel2.default_band_for_role(role)
    } else {
        landsat_asset_key(role)
    }
    .expect("all explorer index bands are available on both sensors")
}

pub(crate) fn observation(sample: &PointSample, profile: SensorProfile, item: &str) -> Observation {
    let sentinel = profile != SensorProfile::LandsatC2L2Sr;
    let instrument = instrument_for_scene(item);
    let clear = if sentinel {
        // Clear land and water. Snow and unclassified pixels obscure the target.
        matches!(sample.dn("SCL"), Some(4..=6))
    } else {
        sample.dn("qa_pixel").is_some_and(qa_pixel_clear)
            && match instrument {
                LandsatInstrument::Oli => {
                    sample.dn("qa_aerosol").is_some_and(qa_aerosol_acceptable)
                }
                LandsatInstrument::TmEtm => sample.dn("cloud_qa").is_some_and(cloud_qa_clear),
            }
    };
    let mut bands = IndexBandValues::default();
    for role in ROLES {
        if let Some(dn) = sample.dn(asset(role, sentinel)) {
            if let Some(value) = apply_radiometric_scaling(profile, &[dn]).pixels[0].value() {
                bands.insert(role, value);
            }
        }
    }
    let mut indices = BTreeMap::new();
    for (name, index) in [
        ("ndwi", IndexKind::Ndwi),
        ("ndvi", IndexKind::Ndvi),
        ("mndwi", IndexKind::Mndwi),
        ("ndmi", IndexKind::Ndmi),
    ] {
        let usable = clear
            && (sentinel
                || sample.dn("qa_radsat").is_some_and(|code| {
                    code & qa_radsat_reject_mask(instrument, index.required_bands(), false) == 0
                }));
        let value = if usable {
            index
                .compute_value(&bands)
                .ok()
                .and_then(|pixel| pixel.value())
        } else {
            None
        };
        indices.insert(name, value);
    }
    Observation {
        item: item.to_owned(),
        indices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn landsat_water_uses_scaled_reflectance_and_per_index_quality() {
        let mut point = PointSample {
            band_names: [
                "green_b1",
                "nir08_b1",
                "red_b1",
                "swir16_b1",
                "qa_pixel_b1",
                "qa_radsat_b1",
                "qa_aerosol_b1",
            ]
            .map(String::from)
            .into(),
            values: [12000., 10000., 11500., 9000., 128., 8., 0.]
                .map(Some)
                .into(),
        };
        let read =
            |p: &PointSample| observation(p, SensorProfile::LandsatC2L2Sr, "LC08_test").indices;
        assert!((read(&point)["ndwi"].unwrap() - (0.13 - 0.075) / (0.13 + 0.075)).abs() < 1e-5);
        assert_eq!(read(&point)["ndvi"], None); // saturated red
        point.values[4] = Some(8.);
        assert!(read(&point).values().all(Option::is_none));
    }

    #[test]
    fn sentinel_baseline_fill_and_clouds_are_respected() {
        let mut point = PointSample {
            band_names: ["B03_b1", "B08_b1", "SCL_b1"].map(String::from).into(),
            values: [3000., 2000., 6.].map(Some).into(),
        };
        let read = |p: &PointSample, profile| observation(p, profile, "S2A_test").indices["ndwi"];
        assert!(
            (read(&point, SensorProfile::Sentinel2L2ABaseline0400).unwrap() - 1. / 3.).abs() < 1e-5
        );
        assert!((read(&point, SensorProfile::Sentinel2L2ALegacy).unwrap() - 0.2).abs() < 1e-5);
        point.values[2] = Some(9.);
        assert_eq!(read(&point, SensorProfile::Sentinel2L2ALegacy), None);
        point.values[2] = Some(6.);
        point.values[1] = None;
        assert_eq!(read(&point, SensorProfile::Sentinel2L2ALegacy), None);
    }
}
