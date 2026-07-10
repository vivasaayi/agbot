//! Golden-string and round-trip tests for the per-field satellite time
//! series naming conventions (`shared::timeseries_naming`).

use std::str::FromStr;

use shared::timeseries_naming::{
    field_entity_ref, parse_satellite_metric, product_source_ref, satellite_metric, ZonalStat,
    SOURCE_HLS, SOURCE_LANDSAT, SOURCE_MODIS, SOURCE_SENTINEL2,
};

#[test]
fn entity_and_source_refs_match_existing_conventions() {
    // Matches the ad-hoc `format!("field:{field_id}")` convention already
    // used by geo_hub alert evaluation and routes.
    assert_eq!(field_entity_ref("field-42"), "field:field-42");
    assert_eq!(product_source_ref("abc123"), "product:abc123");
}

#[test]
fn source_constants_are_stable() {
    assert_eq!(SOURCE_LANDSAT, "landsat");
    assert_eq!(SOURCE_SENTINEL2, "sentinel2");
    assert_eq!(SOURCE_HLS, "hls");
    assert_eq!(SOURCE_MODIS, "modis");
}

#[test]
fn zonal_stat_as_str_golden_strings() {
    assert_eq!(ZonalStat::Mean.as_str(), "mean");
    assert_eq!(ZonalStat::Median.as_str(), "median");
    assert_eq!(ZonalStat::P10.as_str(), "p10");
    assert_eq!(ZonalStat::P90.as_str(), "p90");
    assert_eq!(ZonalStat::ValidFraction.as_str(), "valid_fraction");
}

#[test]
fn zonal_stat_from_str_round_trips_every_variant() {
    for stat in ZonalStat::ALL {
        let parsed = ZonalStat::from_str(stat.as_str()).expect("round trip");
        assert_eq!(parsed, stat);
    }
    assert!(ZonalStat::from_str("p50").is_err());
    assert!(ZonalStat::from_str("").is_err());
}

#[test]
fn zonal_stat_serde_uses_snake_case_strings() {
    for stat in ZonalStat::ALL {
        let json = serde_json::to_string(&stat).expect("serialize");
        assert_eq!(json, format!("\"{}\"", stat.as_str()));
        let back: ZonalStat = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, stat);
    }
    assert!(serde_json::from_str::<ZonalStat>("\"p50\"").is_err());
}

#[test]
fn satellite_metric_golden_strings() {
    assert_eq!(satellite_metric("ndvi", ZonalStat::Mean), "sat.ndvi.mean");
    assert_eq!(satellite_metric("ndmi", ZonalStat::P90), "sat.ndmi.p90");
    assert_eq!(
        satellite_metric("mndwi", ZonalStat::ValidFraction),
        "sat.mndwi.valid_fraction"
    );
}

#[test]
fn parse_satellite_metric_round_trips_and_rejects_malformed() {
    for stat in ZonalStat::ALL {
        let metric = satellite_metric("ndvi", stat);
        assert_eq!(
            parse_satellite_metric(&metric),
            Some(("ndvi".to_string(), stat))
        );
    }
    assert_eq!(
        parse_satellite_metric("ndvi.mean"),
        None,
        "missing sat. prefix"
    );
    assert_eq!(
        parse_satellite_metric("sat.mean"),
        None,
        "missing index segment"
    );
    assert_eq!(parse_satellite_metric("sat.ndvi.p50"), None, "unknown stat");
    assert_eq!(parse_satellite_metric("sat..mean"), None, "empty index");
    assert_eq!(parse_satellite_metric(""), None);
}
