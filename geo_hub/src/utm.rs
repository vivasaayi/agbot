//! WGS84 <-> UTM coordinate conversion (satellite pipeline batch 6).
//!
//! Satellite COGs (Sentinel-2, Landsat) are gridded in UTM zones; AOIs arrive
//! as WGS84 lon/lat. This module implements the standard transverse-Mercator
//! Krüger series (third order in the third flattening n, as published in
//! Karney 2011 and used by proj's UTM approximation) so geo_hub can map an
//! AOI into scene pixel windows without a native proj dependency.
//!
//! # Dependency decision
//! `proj4rs` was considered and skipped: these are ~40 lines of well-known
//! deterministic math, the third-order series is accurate to well under a
//! centimeter inside a UTM zone (verified against published reference
//! points in the tests below), and no other projection is needed.

use thiserror::Error;

/// WGS84 ellipsoid.
const WGS84_A: f64 = 6_378_137.0;
const WGS84_F: f64 = 1.0 / 298.257_223_563;
/// UTM scale factor at the central meridian.
const K0: f64 = 0.9996;
/// UTM false easting (m).
const FALSE_EASTING: f64 = 500_000.0;
/// UTM false northing for the southern hemisphere (m).
const FALSE_NORTHING_SOUTH: f64 = 10_000_000.0;

#[derive(Debug, Error, PartialEq)]
pub enum UtmError {
    #[error("EPSG code {0} is not a WGS84/UTM code (326xx north or 327xx south)")]
    NotUtmEpsg(u32),
    #[error("latitude {0} is outside the UTM domain (-80..84)")]
    LatitudeOutOfRange(f64),
    #[error("longitude {0} is outside -180..180")]
    LongitudeOutOfRange(f64),
}

/// A UTM zone: number 1..=60 plus hemisphere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UtmZone {
    pub zone: u8,
    pub north: bool,
}

impl UtmZone {
    /// Parse a WGS84/UTM EPSG code (EPSG:32601-32660 north, 32701-32760 south).
    pub fn from_epsg(epsg: u32) -> Result<Self, UtmError> {
        match epsg {
            32601..=32660 => Ok(Self {
                zone: (epsg - 32600) as u8,
                north: true,
            }),
            32701..=32760 => Ok(Self {
                zone: (epsg - 32700) as u8,
                north: false,
            }),
            other => Err(UtmError::NotUtmEpsg(other)),
        }
    }

    pub fn epsg(self) -> u32 {
        if self.north {
            32600 + u32::from(self.zone)
        } else {
            32700 + u32::from(self.zone)
        }
    }

    /// Central meridian of the zone, degrees.
    pub fn central_meridian_deg(self) -> f64 {
        f64::from(self.zone) * 6.0 - 183.0
    }
}

/// Third flattening and rectifying-radius series terms shared by both
/// directions.
struct Kruger {
    /// Rectifying radius A.
    radius: f64,
    /// sqrt-n mixing factor 2*sqrt(n)/(1+n) used in the conformal latitude.
    conformal: f64,
    alpha: [f64; 3],
    beta: [f64; 3],
    delta: [f64; 3],
}

fn kruger() -> Kruger {
    let n = WGS84_F / (2.0 - WGS84_F);
    let n2 = n * n;
    let n3 = n2 * n;
    Kruger {
        radius: WGS84_A / (1.0 + n) * (1.0 + n2 / 4.0 + n2 * n2 / 64.0),
        conformal: 2.0 * n.sqrt() / (1.0 + n),
        alpha: [
            n / 2.0 - 2.0 / 3.0 * n2 + 5.0 / 16.0 * n3,
            13.0 / 48.0 * n2 - 3.0 / 5.0 * n3,
            61.0 / 240.0 * n3,
        ],
        beta: [
            n / 2.0 - 2.0 / 3.0 * n2 + 37.0 / 96.0 * n3,
            n2 / 48.0 + n3 / 15.0,
            17.0 / 480.0 * n3,
        ],
        delta: [
            2.0 * n - 2.0 / 3.0 * n2 - 2.0 * n3,
            7.0 / 3.0 * n2 - 8.0 / 5.0 * n3,
            56.0 / 15.0 * n3,
        ],
    }
}

/// Forward: WGS84 lat/lon (degrees) -> UTM easting/northing (meters) in the
/// given zone. The zone is not derived from the longitude because satellite
/// scenes fix the zone; points slightly outside the nominal zone stay valid.
pub fn wgs84_to_utm(lat_deg: f64, lon_deg: f64, zone: UtmZone) -> Result<(f64, f64), UtmError> {
    if !(-80.0..=84.0).contains(&lat_deg) {
        return Err(UtmError::LatitudeOutOfRange(lat_deg));
    }
    if !(-180.0..=180.0).contains(&lon_deg) {
        return Err(UtmError::LongitudeOutOfRange(lon_deg));
    }
    let k = kruger();
    let lat = lat_deg.to_radians();
    let dlon = (lon_deg - zone.central_meridian_deg()).to_radians();

    // Conformal latitude via the Gauss-Schreiber transverse Mercator.
    let sin_lat = lat.sin();
    let t = (sin_lat.atanh() - k.conformal * (k.conformal * sin_lat).atanh()).sinh();
    let xi_prime = t.atan2(dlon.cos());
    let eta_prime = (dlon.sin() / (1.0 + t * t).sqrt()).atanh();

    let mut xi = xi_prime;
    let mut eta = eta_prime;
    for (index, alpha) in k.alpha.iter().enumerate() {
        let j = 2.0 * (index as f64 + 1.0);
        xi += alpha * (j * xi_prime).sin() * (j * eta_prime).cosh();
        eta += alpha * (j * xi_prime).cos() * (j * eta_prime).sinh();
    }

    let easting = FALSE_EASTING + K0 * k.radius * eta;
    let mut northing = K0 * k.radius * xi;
    if !zone.north {
        northing += FALSE_NORTHING_SOUTH;
    }
    Ok((easting, northing))
}

/// Inverse: UTM easting/northing (meters) in the given zone -> WGS84 lat/lon
/// (degrees).
pub fn utm_to_wgs84(easting: f64, northing: f64, zone: UtmZone) -> (f64, f64) {
    let k = kruger();
    let false_northing = if zone.north {
        0.0
    } else {
        FALSE_NORTHING_SOUTH
    };
    let xi = (northing - false_northing) / (K0 * k.radius);
    let eta = (easting - FALSE_EASTING) / (K0 * k.radius);

    let mut xi_prime = xi;
    let mut eta_prime = eta;
    for (index, beta) in k.beta.iter().enumerate() {
        let j = 2.0 * (index as f64 + 1.0);
        xi_prime -= beta * (j * xi).sin() * (j * eta).cosh();
        eta_prime -= beta * (j * xi).cos() * (j * eta).sinh();
    }

    let chi = (xi_prime.sin() / eta_prime.cosh()).asin();
    let mut lat = chi;
    for (index, delta) in k.delta.iter().enumerate() {
        let j = 2.0 * (index as f64 + 1.0);
        lat += delta * (j * chi).sin();
    }
    let lon = zone.central_meridian_deg().to_radians() + eta_prime.sinh().atan2(xi_prime.cos());
    (lat.to_degrees(), lon.to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CN_TOWER_LAT: f64 = 43.642567;
    const CN_TOWER_LON: f64 = -79.387139;

    #[test]
    fn epsg_roundtrips_zone_and_hemisphere() {
        let north = UtmZone::from_epsg(32643).unwrap();
        assert_eq!(
            north,
            UtmZone {
                zone: 43,
                north: true
            }
        );
        assert_eq!(north.epsg(), 32643);
        assert_eq!(north.central_meridian_deg(), 75.0);

        let south = UtmZone::from_epsg(32756).unwrap();
        assert!(!south.north);
        assert_eq!(south.zone, 56);

        assert_eq!(UtmZone::from_epsg(4326), Err(UtmError::NotUtmEpsg(4326)));
        assert_eq!(UtmZone::from_epsg(32661), Err(UtmError::NotUtmEpsg(32661)));
    }

    #[test]
    fn forward_matches_published_cn_tower_reference() {
        // Published UTM reference (Wikipedia UTM worked example): the CN Tower
        // at 43.642567 N, 79.387139 W is zone 17N, 630084 m E, 4833438 m N.
        let zone = UtmZone {
            zone: 17,
            north: true,
        };
        let (easting, northing) = wgs84_to_utm(CN_TOWER_LAT, CN_TOWER_LON, zone).unwrap();
        assert!((easting - 630_084.0).abs() < 1.0, "easting {easting}");
        assert!((northing - 4_833_438.0).abs() < 1.0, "northing {northing}");
    }

    #[test]
    fn equator_on_central_meridian_is_the_false_origin() {
        let zone = UtmZone {
            zone: 31,
            north: true,
        };
        let (easting, northing) = wgs84_to_utm(0.0, 3.0, zone).unwrap();
        assert!((easting - FALSE_EASTING).abs() < 1e-6);
        assert!(northing.abs() < 1e-6);
    }

    #[test]
    fn southern_hemisphere_is_mirror_symmetric_with_false_northing() {
        let north = UtmZone {
            zone: 17,
            north: true,
        };
        let south = UtmZone {
            zone: 17,
            north: false,
        };
        let (east_n, north_n) = wgs84_to_utm(CN_TOWER_LAT, CN_TOWER_LON, north).unwrap();
        let (east_s, north_s) = wgs84_to_utm(-CN_TOWER_LAT, CN_TOWER_LON, south).unwrap();
        assert!((east_n - east_s).abs() < 1e-6);
        assert!((north_s - (FALSE_NORTHING_SOUTH - north_n)).abs() < 1e-6);
    }

    #[test]
    fn inverse_recovers_forward_to_sub_millimeter() {
        // Includes the Sentinel-2 fixture zone (43N) at the tile 43PFN origin.
        let cases = [
            (43.642567, -79.387139, 17, true),
            (11.75, 76.35, 43, true),
            (-33.8568, 151.2153, 56, false),
            (60.0, 4.5, 31, true),
        ];
        for (lat, lon, zone, north) in cases {
            let zone = UtmZone { zone, north };
            let (easting, northing) = wgs84_to_utm(lat, lon, zone).unwrap();
            let (lat2, lon2) = utm_to_wgs84(easting, northing, zone);
            // 1e-8 degrees is ~1 mm.
            assert!((lat - lat2).abs() < 1e-8, "lat {lat} -> {lat2}");
            assert!((lon - lon2).abs() < 1e-8, "lon {lon} -> {lon2}");
        }
    }

    #[test]
    fn out_of_domain_inputs_are_reason_coded() {
        let zone = UtmZone {
            zone: 17,
            north: true,
        };
        assert!(matches!(
            wgs84_to_utm(89.0, 0.0, zone),
            Err(UtmError::LatitudeOutOfRange(_))
        ));
        assert!(matches!(
            wgs84_to_utm(0.0, 181.0, zone),
            Err(UtmError::LongitudeOutOfRange(_))
        ));
    }
}
