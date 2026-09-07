//! Naming conventions for the per-field satellite time series.
//!
//! Formalizes the identifiers that tie zonal-statistics observations back to
//! fields, catalog products, and satellite sources:
//!
//! - entity refs: `field:{field_id}` (matches the existing ad-hoc geo_hub
//!   convention in alert evaluation and field routes);
//! - metric names: `sat.{index}.{stat}` (e.g. `sat.ndvi.mean`);
//! - source refs: `product:{product_id}`;
//! - source family constants: [`SOURCE_LANDSAT`], [`SOURCE_SENTINEL2`],
//!   [`SOURCE_HLS`], [`SOURCE_MODIS`].
//!
//! Keep these pure and dependency-free so every crate that reads or writes
//! the time series shares one spelling.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Landsat Collection 2 source family.
pub const SOURCE_LANDSAT: &str = "landsat";
/// Sentinel-2 L2A source family.
pub const SOURCE_SENTINEL2: &str = "sentinel2";
/// Harmonized Landsat Sentinel-2 source family.
pub const SOURCE_HLS: &str = "hls";
/// MODIS source family.
pub const SOURCE_MODIS: &str = "modis";

/// Canonical entity reference for a field: `field:{field_id}`.
pub fn field_entity_ref(field_id: &str) -> String {
    format!("field:{field_id}")
}

/// Canonical source reference for a catalog product: `product:{product_id}`.
pub fn product_source_ref(product_id: &str) -> String {
    format!("product:{product_id}")
}

/// Zonal statistic reduced over a field's valid pixels for one observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZonalStat {
    Mean,
    Median,
    P10,
    P90,
    ValidFraction,
}

impl ZonalStat {
    /// Every variant, for exhaustive iteration in parsers and tests.
    pub const ALL: [ZonalStat; 5] = [
        ZonalStat::Mean,
        ZonalStat::Median,
        ZonalStat::P10,
        ZonalStat::P90,
        ZonalStat::ValidFraction,
    ];

    /// Canonical lowercase key (also the serde string form).
    pub fn as_str(&self) -> &'static str {
        match self {
            ZonalStat::Mean => "mean",
            ZonalStat::Median => "median",
            ZonalStat::P10 => "p10",
            ZonalStat::P90 => "p90",
            ZonalStat::ValidFraction => "valid_fraction",
        }
    }
}

impl fmt::Display for ZonalStat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Reason-coded parse failure for [`ZonalStat`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown zonal stat {0:?}; expected mean|median|p10|p90|valid_fraction")]
pub struct ParseZonalStatError(pub String);

impl FromStr for ZonalStat {
    type Err = ParseZonalStatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ZonalStat::ALL
            .into_iter()
            .find(|stat| stat.as_str() == s)
            .ok_or_else(|| ParseZonalStatError(s.to_string()))
    }
}

/// Canonical time-series metric name for a satellite index statistic:
/// `sat.{index}.{stat}` (e.g. `sat.ndvi.mean`).
pub fn satellite_metric(index: &str, stat: ZonalStat) -> String {
    format!("sat.{index}.{}", stat.as_str())
}

/// Parse a `sat.{index}.{stat}` metric name back into its parts. Returns
/// `None` for anything that is not a well-formed satellite metric.
pub fn parse_satellite_metric(metric: &str) -> Option<(String, ZonalStat)> {
    let rest = metric.strip_prefix("sat.")?;
    let (index, stat) = rest.rsplit_once('.')?;
    if index.is_empty() {
        return None;
    }
    let stat = stat.parse().ok()?;
    Some((index.to_string(), stat))
}
