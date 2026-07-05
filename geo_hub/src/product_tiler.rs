//! Global Web Mercator (XYZ / EPSG:3857) tiler for catalog raster products
//! (satellite pipeline batch 7).
//!
//! The scene-local tile route (`/api/scenes/.../tiles/...`) splits a product
//! image into 2^z x 2^z pixel-space tiles, which a web map cannot place.
//! This module renders true slippy-map tiles instead: each 256x256 output
//! pixel is inverse-projected Web Mercator -> WGS84 -> the product's UTM
//! grid and nearest-sampled from the GeoTIFF band, then colormapped by
//! product kind. Derived index GeoTIFFs (L2/L3, f32 + nodata) become a
//! MapLibre `raster` source with no client-side stitching.
//!
//! Pure math (tile bounds, projection chain, sampling, colormaps) is
//! separated from file I/O ([`load_tile_source`]) so rendering is unit-tested
//! without fixtures.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::Path;

use raster_io::GeoTiffReader;
use thiserror::Error;

use crate::utm::{wgs84_to_utm, UtmZone};

/// Output tile edge length in pixels.
pub const TILE_SIZE: u32 = 256;
/// WGS84/Web Mercator sphere radius (meters).
const EARTH_RADIUS_M: f64 = 6_378_137.0;
/// Half the Web Mercator world width: PI * R.
const MERCATOR_ORIGIN_M: f64 = std::f64::consts::PI * EARTH_RADIUS_M;
/// Deepest zoom accepted; beyond this a single UTM pixel spans many tiles
/// and requests are almost certainly malformed.
pub const MAX_ZOOM: u8 = 24;

#[derive(Debug, Error)]
pub enum TileError {
    #[error("zoom {0} exceeds the maximum of {MAX_ZOOM}")]
    ZoomTooDeep(u8),
    #[error("tile ({x}, {y}) is outside the {tiles_per_axis}x{tiles_per_axis} grid of zoom {z}")]
    TileOutOfRange {
        z: u8,
        x: u32,
        y: u32,
        tiles_per_axis: u32,
    },
    #[error("product raster has no EPSG code; cannot web-tile an ungeoreferenced raster")]
    MissingEpsg,
    #[error("product raster CRS EPSG:{0} is not supported; web-tileable grids are WGS84/UTM (326xx/327xx) or geographic (4326)")]
    UnsupportedCrs(u32),
    #[error("product raster has no geotransform")]
    MissingGeotransform,
    #[error("product raster grid is rotated (geotransform shear terms are nonzero); only north-up grids are supported")]
    RotatedGrid,
    #[error("failed to read product raster: {0}")]
    Raster(#[from] raster_io::RasterIoError),
    #[error("failed to encode tile PNG: {0}")]
    PngEncode(#[from] image::ImageError),
}

// --- Web Mercator math -------------------------------------------------------

/// Axis-aligned Web Mercator rectangle (meters).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MercatorRect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

/// Web Mercator bounds of slippy-map tile (z, x, y). Row y = 0 is the
/// northernmost tile row.
pub fn tile_mercator_rect(z: u8, x: u32, y: u32) -> Result<MercatorRect, TileError> {
    if z > MAX_ZOOM {
        return Err(TileError::ZoomTooDeep(z));
    }
    let tiles_per_axis = 1_u32 << z;
    if x >= tiles_per_axis || y >= tiles_per_axis {
        return Err(TileError::TileOutOfRange {
            z,
            x,
            y,
            tiles_per_axis,
        });
    }
    let span = 2.0 * MERCATOR_ORIGIN_M / f64::from(tiles_per_axis);
    let min_x = -MERCATOR_ORIGIN_M + f64::from(x) * span;
    let max_y = MERCATOR_ORIGIN_M - f64::from(y) * span;
    Ok(MercatorRect {
        min_x,
        max_x: min_x + span,
        min_y: max_y - span,
        max_y,
    })
}

/// Web Mercator meters -> WGS84 (lat, lon) degrees.
pub fn mercator_to_wgs84(x: f64, y: f64) -> (f64, f64) {
    let lon = (x / EARTH_RADIUS_M).to_degrees();
    let lat = (y / EARTH_RADIUS_M).sinh().atan().to_degrees();
    (lat, lon)
}

/// WGS84 (lat, lon) degrees -> Web Mercator meters. Latitude must be inside
/// the Mercator domain (|lat| < 90); callers pass map coordinates.
pub fn wgs84_to_mercator(lat: f64, lon: f64) -> (f64, f64) {
    let x = lon.to_radians() * EARTH_RADIUS_M;
    let y = lat.to_radians().tan().asinh() * EARTH_RADIUS_M;
    (x, y)
}

// --- Colormaps ---------------------------------------------------------------

/// A deterministic linear-interpolated color ramp over a fixed value domain.
/// Fixed domains keep tile colors stable across requests and products, so a
/// value renders identically regardless of the raster it came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colormap {
    /// Inclusive value domain mapped onto the ramp; values clamp to it.
    pub domain: (f32, f32),
    /// RGB stops at evenly spaced fractions of the domain (first = domain.0,
    /// last = domain.1). For categorical maps, one color per integer class
    /// code from `domain.0` upward.
    pub stops: &'static [[u8; 3]],
    /// Categorical: values round to integer class codes and pick a stop
    /// directly (no interpolation).
    pub categorical: bool,
}

/// Brown -> pale yellow -> green: vegetation indices.
const RAMP_VEGETATION: &[[u8; 3]] = &[[140, 81, 10], [246, 232, 195], [26, 152, 80]];
/// Tan -> near-white -> blue: water/moisture indices (positive = wet).
const RAMP_WATER: &[[u8; 3]] = &[[191, 160, 116], [240, 240, 240], [24, 100, 190]];
/// Red -> yellow -> green: 0-100 condition scores (drought indices).
const RAMP_CONDITION: &[[u8; 3]] = &[[215, 48, 39], [254, 224, 139], [26, 152, 80]];
/// Red -> white -> blue: standardized anomalies (SPI; dry negative, wet
/// positive).
const RAMP_DIVERGING_DRY_WET: &[[u8; 3]] = &[[178, 24, 43], [247, 247, 247], [33, 102, 172]];
/// White -> blue: precipitation accumulations (mm).
const RAMP_PRECIP: &[[u8; 3]] = &[[247, 251, 255], [8, 69, 148]];
/// Black -> white fallback for unknown kinds.
const RAMP_GRAY: &[[u8; 3]] = &[[0, 0, 0], [255, 255, 255]];
/// Green -> white -> yellow -> red -> purple: dNBR burn severity
/// (regrowth negative, burn positive; Key & Benson class range).
const RAMP_DNBR: &[[u8; 3]] = &[
    [26, 152, 80],
    [247, 247, 247],
    [254, 224, 139],
    [215, 48, 39],
    [122, 1, 119],
];
/// Blue -> pale yellow -> red: land surface temperature (Kelvin).
const RAMP_THERMAL: &[[u8; 3]] = &[[49, 54, 149], [255, 255, 191], [165, 0, 38]];
/// Binary water-mask colors: land tan, water blue.
const RAMP_WATER_MASK: &[[u8; 3]] = &[[210, 180, 140], [24, 100, 190]];
/// Categorical land-cover class colors, codes 1..=6: water blue, bare tan,
/// annual crop yellow, tree/perennial dark green, grassland light green,
/// unknown gray.
const RAMP_LANDCOVER: &[[u8; 3]] = &[
    [24, 100, 190],
    [210, 180, 140],
    [228, 180, 60],
    [0, 100, 0],
    [154, 205, 50],
    [128, 128, 128],
];

/// Colormap for a catalog product kind. Vegetation indices use the full
/// theoretical [-1, 1] domain; water and moisture indices likewise; drought
/// condition indices are percentages. Unknown kinds fall back to a gray ramp
/// over [-1, 1] so every raster still renders.
pub fn colormap_for_kind(kind: &str) -> Colormap {
    let base = kind.trim().to_ascii_lowercase();
    match base.as_str() {
        "ndvi" | "ndre" | "gndvi" | "vari" | "evi" | "evi2" | "savi" | "msavi" | "osavi"
        | "nbr" => Colormap {
            domain: (-1.0, 1.0),
            stops: RAMP_VEGETATION,
            categorical: false,
        },
        "ndwi" | "mndwi" | "ndmi" | "aweinsh" | "aweish" => Colormap {
            domain: (-1.0, 1.0),
            stops: RAMP_WATER,
            categorical: false,
        },
        "drought_index" | "drought.vci" | "drought.tci" | "drought.vhi" | "vci" | "tci" | "vhi" => {
            Colormap {
                domain: (0.0, 100.0),
                stops: RAMP_CONDITION,
                categorical: false,
            }
        }
        // LST in Kelvin: cool blue -> pale yellow -> hot red over the
        // terrestrial 250-330 K range (matches the CLI thermal viz range).
        "lst" | "thermal_lst" => Colormap {
            domain: (250.0, 330.0),
            stops: RAMP_THERMAL,
            categorical: false,
        },
        // SPI is a standard-normal quantile; McKee classes end at ±2, the
        // operational range at ~±3.09 (probability floor).
        "spi" => Colormap {
            domain: (-3.0, 3.0),
            stops: RAMP_DIVERGING_DRY_WET,
            categorical: false,
        },
        "precipitation" => Colormap {
            domain: (0.0, 500.0),
            stops: RAMP_PRECIP,
            categorical: false,
        },
        // dNBR: diverging over the Key & Benson class range (-0.5 regrowth
        // .. 1.0 high severity); white sits at unburned ~0 (domain quarter).
        "dnbr" => Colormap {
            domain: (-0.5, 1.0),
            stops: RAMP_DNBR,
            categorical: false,
        },
        // Binary water mask (0 land, 1 water).
        "water_extent" => Colormap {
            domain: (0.0, 1.0),
            stops: RAMP_WATER_MASK,
            categorical: true,
        },
        // Tier-1 rule + tier-3 learned land-cover classes share the code
        // space (1..=6: water, bare, annual crop, tree/perennial,
        // grassland, unknown).
        "landcover_rule" | "landcover_ml" => Colormap {
            domain: (1.0, 6.0),
            stops: RAMP_LANDCOVER,
            categorical: true,
        },
        _ => Colormap {
            domain: (-1.0, 1.0),
            stops: RAMP_GRAY,
            categorical: false,
        },
    }
}

impl Colormap {
    /// Map a value to RGB, clamping to the domain and interpolating linearly
    /// between adjacent stops.
    pub fn rgb(&self, value: f32) -> [u8; 3] {
        let (lo, hi) = self.domain;
        if self.categorical {
            let index = (f64::from(value).round() - f64::from(lo)).max(0.0) as usize;
            return self.stops[index.min(self.stops.len() - 1)];
        }
        let t = f64::from((value.clamp(lo, hi) - lo) / (hi - lo));
        let segments = self.stops.len() - 1;
        let scaled = t * segments as f64;
        let index = (scaled.floor() as usize).min(segments - 1);
        let frac = scaled - index as f64;
        let from = self.stops[index];
        let to = self.stops[index + 1];
        let mut rgb = [0u8; 3];
        for channel in 0..3 {
            let value = f64::from(from[channel])
                + (f64::from(to[channel]) - f64::from(from[channel])) * frac;
            rgb[channel] = value.round().clamp(0.0, 255.0) as u8;
        }
        rgb
    }
}

// --- Tile source -------------------------------------------------------------

/// How grid coordinates relate to WGS84: a projected UTM zone (satellite
/// scene products) or a plain geographic lat/lon grid (EPSG:4326, e.g.
/// CHIRPS precipitation and SPI rasters).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridProjection {
    Utm(UtmZone),
    Geographic,
}

/// A raster product loaded for tiling: one f32 band on a north-up grid.
#[derive(Debug, Clone)]
pub struct TileSource {
    pub projection: GridProjection,
    /// GDAL geotransform (north-up: shear terms zero). Units are meters for
    /// UTM grids and degrees for geographic grids.
    pub transform: [f64; 6],
    pub width: u32,
    pub height: u32,
    /// Row-major band values.
    pub values: Vec<f32>,
    pub nodata: Option<f32>,
}

impl TileSource {
    /// Validate grid invariants shared by the loader and by tests that build
    /// sources directly.
    pub fn new(
        epsg: u32,
        transform: [f64; 6],
        width: u32,
        height: u32,
        values: Vec<f32>,
        nodata: Option<f32>,
    ) -> Result<Self, TileError> {
        let projection = if epsg == 4326 {
            GridProjection::Geographic
        } else {
            GridProjection::Utm(
                UtmZone::from_epsg(epsg).map_err(|_| TileError::UnsupportedCrs(epsg))?,
            )
        };
        if transform[2] != 0.0 || transform[4] != 0.0 {
            return Err(TileError::RotatedGrid);
        }
        Ok(Self {
            projection,
            transform,
            width,
            height,
            values,
            nodata,
        })
    }

    /// Grid coordinates (geotransform units) of a WGS84 point; `None` when
    /// the point is outside the projection's domain (UTM latitude range).
    fn grid_coords(&self, lat: f64, lon: f64) -> Option<(f64, f64)> {
        match self.projection {
            GridProjection::Utm(zone) => wgs84_to_utm(lat, lon, zone).ok(),
            GridProjection::Geographic => Some((lon, lat)),
        }
    }

    /// Nearest-neighbor sample at a grid coordinate (meters for UTM grids,
    /// degrees for geographic ones); `None` outside the grid or on a
    /// nodata/non-finite pixel.
    pub fn sample_grid(&self, easting: f64, northing: f64) -> Option<f32> {
        let col = (easting - self.transform[0]) / self.transform[1];
        let row = (northing - self.transform[3]) / self.transform[5];
        if col < 0.0 || row < 0.0 {
            return None;
        }
        let (col, row) = (col.floor() as u64, row.floor() as u64);
        if col >= u64::from(self.width) || row >= u64::from(self.height) {
            return None;
        }
        let value = self.values[(row * u64::from(self.width) + col) as usize];
        if !value.is_finite() {
            return None;
        }
        if let Some(nodata) = self.nodata {
            if value == nodata {
                return None;
            }
        }
        Some(value)
    }

    /// WGS84 envelope of the grid extent (corner inverse projection), used
    /// for the cheap tile/raster disjointness test.
    fn wgs84_envelope(&self) -> (f64, f64, f64, f64) {
        let min_x = self.transform[0];
        let max_x = self.transform[0] + f64::from(self.width) * self.transform[1];
        let max_y = self.transform[3];
        let min_y = self.transform[3] + f64::from(self.height) * self.transform[5];
        match self.projection {
            GridProjection::Geographic => (min_y, min_x, max_y, max_x),
            GridProjection::Utm(zone) => {
                let corners = [
                    crate::utm::utm_to_wgs84(min_x, min_y, zone),
                    crate::utm::utm_to_wgs84(max_x, min_y, zone),
                    crate::utm::utm_to_wgs84(min_x, max_y, zone),
                    crate::utm::utm_to_wgs84(max_x, max_y, zone),
                ];
                let min_lat = corners.iter().map(|(lat, _)| *lat).fold(f64::MAX, f64::min);
                let max_lat = corners.iter().map(|(lat, _)| *lat).fold(f64::MIN, f64::max);
                let min_lon = corners.iter().map(|(_, lon)| *lon).fold(f64::MAX, f64::min);
                let max_lon = corners.iter().map(|(_, lon)| *lon).fold(f64::MIN, f64::max);
                (min_lat, min_lon, max_lat, max_lon)
            }
        }
    }
}

/// Load a GeoTIFF product artifact as a [`TileSource`]. Any band dtype is
/// accepted (u8/u16 widen losslessly to f32, matching `RasterBand::to_f32`).
pub fn load_tile_source(path: &Path) -> Result<TileSource, TileError> {
    let mut reader = GeoTiffReader::open(path)?;
    let info = reader.info().clone();
    let epsg = info.epsg.ok_or(TileError::MissingEpsg)?;
    let transform = info.geo_transform.ok_or(TileError::MissingGeotransform)?;
    let values = reader.read_band()?.to_f32();
    TileSource::new(
        epsg,
        transform,
        info.width,
        info.height,
        values,
        info.nodata.map(|n| n as f32),
    )
}

// --- Rendering ---------------------------------------------------------------

/// A rendered RGBA tile plus render evidence.
#[derive(Debug, Clone)]
pub struct RenderedTile {
    /// TILE_SIZE x TILE_SIZE RGBA, row-major.
    pub rgba: Vec<u8>,
    pub opaque_pixels: usize,
    /// Per-reason transparent-pixel counts (`outside_grid`, `nodata_or_masked`,
    /// `outside_utm_domain`).
    pub transparent_reasons: BTreeMap<String, usize>,
}

/// Render one Web Mercator tile from a tile source. Every output pixel
/// center is projected Mercator -> WGS84 -> the source grid (UTM meters or
/// geographic degrees) and nearest-sampled; pixels outside the grid, outside
/// the UTM latitude domain, or on nodata stay fully transparent.
pub fn render_web_tile(
    source: &TileSource,
    colormap: &Colormap,
    z: u8,
    x: u32,
    y: u32,
) -> Result<RenderedTile, TileError> {
    let rect = tile_mercator_rect(z, x, y)?;
    let mut rgba = vec![0u8; (TILE_SIZE * TILE_SIZE * 4) as usize];
    let mut opaque_pixels = 0usize;
    let mut transparent_reasons: BTreeMap<String, usize> = BTreeMap::new();
    let transparent = |reason: &str, counts: &mut BTreeMap<String, usize>| {
        *counts.entry(reason.to_string()).or_insert(0) += 1;
    };

    // Cheap rejection: if the tile's WGS84 envelope misses the raster's,
    // skip the per-pixel projection loop entirely.
    let (src_min_lat, src_min_lon, src_max_lat, src_max_lon) = source.wgs84_envelope();
    let (tile_min_lat, tile_min_lon) = mercator_to_wgs84(rect.min_x, rect.min_y);
    let (tile_max_lat, tile_max_lon) = mercator_to_wgs84(rect.max_x, rect.max_y);
    if tile_max_lat < src_min_lat
        || tile_min_lat > src_max_lat
        || tile_max_lon < src_min_lon
        || tile_min_lon > src_max_lon
    {
        transparent_reasons.insert("outside_grid".to_string(), (TILE_SIZE * TILE_SIZE) as usize);
        return Ok(RenderedTile {
            rgba,
            opaque_pixels: 0,
            transparent_reasons,
        });
    }

    let span_x = (rect.max_x - rect.min_x) / f64::from(TILE_SIZE);
    let span_y = (rect.max_y - rect.min_y) / f64::from(TILE_SIZE);
    for py in 0..TILE_SIZE {
        let merc_y = rect.max_y - (f64::from(py) + 0.5) * span_y;
        for px in 0..TILE_SIZE {
            let merc_x = rect.min_x + (f64::from(px) + 0.5) * span_x;
            let (lat, lon) = mercator_to_wgs84(merc_x, merc_y);
            let Some((easting, northing)) = source.grid_coords(lat, lon) else {
                transparent("outside_utm_domain", &mut transparent_reasons);
                continue;
            };
            let offset = ((py * TILE_SIZE + px) * 4) as usize;
            match source.sample_grid(easting, northing) {
                Some(value) => {
                    let rgb = colormap.rgb(value);
                    rgba[offset..offset + 3].copy_from_slice(&rgb);
                    rgba[offset + 3] = 255;
                    opaque_pixels += 1;
                }
                None => transparent("nodata_or_outside_grid", &mut transparent_reasons),
            }
        }
    }

    Ok(RenderedTile {
        rgba,
        opaque_pixels,
        transparent_reasons,
    })
}

/// Encode a rendered tile as PNG bytes.
pub fn encode_tile_png(tile: &RenderedTile) -> Result<Vec<u8>, TileError> {
    let image = image::RgbaImage::from_raw(TILE_SIZE, TILE_SIZE, tile.rgba.clone())
        .expect("rgba buffer is TILE_SIZE x TILE_SIZE x 4 by construction");
    let mut bytes = Vec::new();
    image.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)?;
    Ok(bytes)
}

/// Slippy-map tile coordinates containing a WGS84 point at a zoom level.
/// Test/UI helper mirroring the standard XYZ formula.
pub fn tile_containing(lat: f64, lon: f64, z: u8) -> (u32, u32) {
    let n = f64::from(1_u32 << z);
    let x = ((lon + 180.0) / 360.0 * n).floor();
    let lat_rad = lat.to_radians();
    let y = ((1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n).floor();
    (
        (x.max(0.0) as u32).min((n as u32) - 1),
        (y.max(0.0) as u32).min((n as u32) - 1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone_43n() -> UtmZone {
        UtmZone {
            zone: 43,
            north: true,
        }
    }

    /// Sentinel-2 43PFN-style grid: 10 m pixels at (600000, 1300020), EPSG:32643.
    fn source_43pfn(values: Vec<f32>, width: u32, height: u32) -> TileSource {
        TileSource::new(
            32643,
            [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0],
            width,
            height,
            values,
            Some(-9999.0),
        )
        .unwrap()
    }

    #[test]
    fn zoom_zero_tile_is_the_whole_mercator_world() {
        let rect = tile_mercator_rect(0, 0, 0).unwrap();
        assert!((rect.min_x + MERCATOR_ORIGIN_M).abs() < 1e-6);
        assert!((rect.max_x - MERCATOR_ORIGIN_M).abs() < 1e-6);
        assert!((rect.min_y + MERCATOR_ORIGIN_M).abs() < 1e-6);
        assert!((rect.max_y - MERCATOR_ORIGIN_M).abs() < 1e-6);
    }

    #[test]
    fn tile_rect_and_tile_containing_agree() {
        // The tile containing a point must have a mercator rect containing
        // that point's mercator coordinates.
        let (lat, lon) = (11.75, 76.91);
        for z in [1u8, 5, 10, 14] {
            let (x, y) = tile_containing(lat, lon, z);
            let rect = tile_mercator_rect(z, x, y).unwrap();
            let (mx, my) = wgs84_to_mercator(lat, lon);
            assert!(rect.min_x <= mx && mx < rect.max_x, "x at z={z}");
            assert!(rect.min_y <= my && my < rect.max_y, "y at z={z}");
        }
    }

    #[test]
    fn out_of_range_tiles_are_reason_coded() {
        assert!(matches!(
            tile_mercator_rect(25, 0, 0),
            Err(TileError::ZoomTooDeep(25))
        ));
        assert!(matches!(
            tile_mercator_rect(2, 4, 0),
            Err(TileError::TileOutOfRange { .. })
        ));
    }

    #[test]
    fn mercator_wgs84_roundtrip() {
        for (lat, lon) in [(0.0, 0.0), (45.0, -120.5), (-33.86, 151.21), (11.75, 76.91)] {
            let (x, y) = wgs84_to_mercator(lat, lon);
            let (lat2, lon2) = mercator_to_wgs84(x, y);
            assert!((lat - lat2).abs() < 1e-9);
            assert!((lon - lon2).abs() < 1e-9);
        }
    }

    #[test]
    fn colormaps_pin_kind_families_and_interpolate() {
        let ndvi = colormap_for_kind("ndvi");
        assert_eq!(ndvi.domain, (-1.0, 1.0));
        assert_eq!(ndvi.rgb(-1.0), [140, 81, 10]);
        assert_eq!(ndvi.rgb(0.0), [246, 232, 195]);
        assert_eq!(ndvi.rgb(1.0), [26, 152, 80]);
        // Clamps outside the domain.
        assert_eq!(ndvi.rgb(4.0), ndvi.rgb(1.0));
        // Midpoint of the upper segment interpolates.
        let mid = ndvi.rgb(0.5);
        assert_eq!(mid, [136, 192, 138]);

        assert_eq!(colormap_for_kind("mndwi").stops, RAMP_WATER);
        assert_eq!(colormap_for_kind("drought.vhi").domain, (0.0, 100.0));
        assert_eq!(colormap_for_kind("mystery_kind").stops, RAMP_GRAY);
    }

    #[test]
    fn unsupported_and_rotated_grids_are_rejected() {
        // 3857 (mercator) has no sampling path; 32661 is not a valid UTM code.
        for epsg in [3857u32, 32661] {
            assert!(matches!(
                TileSource::new(epsg, [0.0; 6], 1, 1, vec![0.0], None),
                Err(TileError::UnsupportedCrs(code)) if code == epsg
            ));
        }
        assert!(matches!(
            TileSource::new(
                32643,
                [600_000.0, 10.0, 0.5, 1_300_020.0, 0.0, -10.0],
                1,
                1,
                vec![0.0],
                None
            ),
            Err(TileError::RotatedGrid)
        ));
    }

    #[test]
    fn sample_grid_is_nearest_and_respects_nodata_and_bounds() {
        // 2x2 grid: [[0.1, 0.5], [nodata, 0.9]].
        let source = source_43pfn(vec![0.1, 0.5, -9999.0, 0.9], 2, 2);
        // Pixel centers.
        assert_eq!(source.sample_grid(600_005.0, 1_300_015.0), Some(0.1));
        assert_eq!(source.sample_grid(600_015.0, 1_300_015.0), Some(0.5));
        assert_eq!(source.sample_grid(600_005.0, 1_300_005.0), None); // nodata
        assert_eq!(source.sample_grid(600_015.0, 1_300_005.0), Some(0.9));
        // Outside the grid on every side.
        assert_eq!(source.sample_grid(599_999.0, 1_300_015.0), None);
        assert_eq!(source.sample_grid(600_025.0, 1_300_015.0), None);
        assert_eq!(source.sample_grid(600_005.0, 1_300_021.0), None);
        assert_eq!(source.sample_grid(600_005.0, 1_299_999.0), None);
    }

    #[test]
    fn render_paints_the_raster_footprint_and_leaves_the_rest_transparent() {
        // A 20x20 px (200 m) uniform NDVI=1.0 patch. Zoom deep enough that
        // the containing tile is mostly raster or mostly background either
        // way; assert both opaque and transparent pixels exist and opaque
        // pixels carry the exact ramp-end color.
        let source = source_43pfn(vec![1.0; 400], 20, 20);
        let (center_lat, center_lon) = crate::utm::utm_to_wgs84(600_100.0, 1_299_920.0, zone_43n());
        let z = 14;
        let (x, y) = tile_containing(center_lat, center_lon, z);
        let tile = render_web_tile(&source, &colormap_for_kind("ndvi"), z, x, y).unwrap();

        assert!(tile.opaque_pixels > 0, "raster must land in its own tile");
        assert!(
            tile.opaque_pixels < (TILE_SIZE * TILE_SIZE) as usize,
            "a 200 m patch cannot fill a z14 tile"
        );
        let first_opaque = tile
            .rgba
            .chunks_exact(4)
            .find(|px| px[3] == 255)
            .expect("an opaque pixel");
        assert_eq!(&first_opaque[..3], &[26, 152, 80]);
    }

    #[test]
    fn tile_far_from_the_raster_is_fully_transparent_via_fast_path() {
        let source = source_43pfn(vec![1.0; 400], 20, 20);
        // A z10 tile near null island: nowhere near UTM zone 43 tile 43PFN.
        let (x, y) = tile_containing(0.0, 0.0, 10);
        let tile = render_web_tile(&source, &colormap_for_kind("ndvi"), 10, x, y).unwrap();
        assert_eq!(tile.opaque_pixels, 0);
        assert_eq!(
            tile.transparent_reasons.get("outside_grid"),
            Some(&((TILE_SIZE * TILE_SIZE) as usize))
        );
        assert!(tile.rgba.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn nodata_pixels_render_transparent_inside_the_footprint() {
        // Left half valid, right half nodata, on a large patch.
        let width = 40u32;
        let mut values = vec![0.5f32; (width * width) as usize];
        for row in 0..width {
            for col in width / 2..width {
                values[(row * width + col) as usize] = -9999.0;
            }
        }
        let source = source_43pfn(values, width, width);
        let (lat, lon) = crate::utm::utm_to_wgs84(600_200.0, 1_299_820.0, zone_43n());
        let z = 15;
        let (x, y) = tile_containing(lat, lon, z);
        let tile = render_web_tile(&source, &colormap_for_kind("ndvi"), z, x, y).unwrap();
        assert!(tile.opaque_pixels > 0);
        assert!(
            tile.transparent_reasons
                .get("nodata_or_outside_grid")
                .copied()
                .unwrap_or(0)
                > 0,
            "nodata half must produce transparent pixels: {:?}",
            tile.transparent_reasons
        );
    }

    #[test]
    fn geographic_grid_renders_without_utm_projection() {
        // A CHIRPS-style 0.05 degree grid over ~(76.0..76.2, 11.0..11.2):
        // 4x4 pixels, EPSG:4326, uniform SPI -2.0 with one nodata pixel.
        let mut values = vec![-2.0f32; 16];
        values[5] = -9999.0;
        let source = TileSource::new(
            4326,
            [76.0, 0.05, 0.0, 11.2, 0.0, -0.05],
            4,
            4,
            values,
            Some(-9999.0),
        )
        .unwrap();
        assert_eq!(source.projection, GridProjection::Geographic);

        // Sampling is direct lon/lat indexing.
        assert_eq!(source.sample_grid(76.01, 11.19), Some(-2.0));
        assert_eq!(source.sample_grid(76.06, 11.12), None); // nodata pixel (1,1)
        assert_eq!(source.sample_grid(75.99, 11.19), None); // west of grid

        let z = 12;
        let (x, y) = tile_containing(11.1, 76.1, z);
        let tile = render_web_tile(&source, &colormap_for_kind("spi"), z, x, y).unwrap();
        assert!(tile.opaque_pixels > 0, "geographic footprint must render");
        // SPI -2.0 on the (-3, 3) diverging ramp = 1/6 of the way up the
        // dry->neutral segment: exact deterministic color.
        let expected = colormap_for_kind("spi").rgb(-2.0);
        let first_opaque = tile
            .rgba
            .chunks_exact(4)
            .find(|px| px[3] == 255)
            .expect("an opaque pixel");
        assert_eq!(&first_opaque[..3], &expected);

        // Far away is fully transparent via the fast path.
        let (fx, fy) = tile_containing(48.0, 2.0, 8);
        let far = render_web_tile(&source, &colormap_for_kind("spi"), 8, fx, fy).unwrap();
        assert_eq!(far.opaque_pixels, 0);
    }

    #[test]
    fn dnbr_colormap_is_pinned_diverging() {
        let dnbr = colormap_for_kind("dnbr");
        assert_eq!(dnbr.domain, (-0.5, 1.0));
        assert_eq!(dnbr.rgb(-0.5), [26, 152, 80]); // strong regrowth
                                                   // Stops sit every 0.375 across the domain: -0.125 is exact white.
        assert_eq!(dnbr.rgb(-0.125), [247, 247, 247]);
        assert_eq!(dnbr.rgb(1.0), [122, 1, 119]); // high severity
    }

    #[test]
    fn water_extent_colormap_is_categorical_binary() {
        let mask = colormap_for_kind("water_extent");
        assert!(mask.categorical);
        assert_eq!(mask.rgb(0.0), [210, 180, 140]); // land
        assert_eq!(mask.rgb(1.0), [24, 100, 190]); // water
    }

    #[test]
    fn landcover_colormap_is_categorical_with_exact_class_colors() {
        let landcover = colormap_for_kind("landcover_rule");
        assert!(landcover.categorical);
        assert_eq!(landcover.rgb(1.0), [24, 100, 190]); // water
        assert_eq!(landcover.rgb(3.0), [228, 180, 60]); // annual crop
        assert_eq!(landcover.rgb(3.4), [228, 180, 60]); // rounds to code 3
        assert_eq!(landcover.rgb(6.0), [128, 128, 128]); // unknown
                                                         // Out-of-range codes clamp instead of panicking.
        assert_eq!(landcover.rgb(0.0), [24, 100, 190]);
        assert_eq!(landcover.rgb(9.0), [128, 128, 128]);
    }

    #[test]
    fn spi_and_precipitation_colormaps_are_pinned() {
        let spi = colormap_for_kind("spi");
        assert_eq!(spi.domain, (-3.0, 3.0));
        assert_eq!(spi.rgb(-3.0), [178, 24, 43]);
        assert_eq!(spi.rgb(0.0), [247, 247, 247]);
        assert_eq!(spi.rgb(3.0), [33, 102, 172]);
        let precip = colormap_for_kind("precipitation");
        assert_eq!(precip.domain, (0.0, 500.0));
    }

    #[test]
    fn encode_tile_png_produces_a_decodable_256px_png() {
        let source = source_43pfn(vec![0.5; 400], 20, 20);
        let (lat, lon) = crate::utm::utm_to_wgs84(600_100.0, 1_299_920.0, zone_43n());
        let (x, y) = tile_containing(lat, lon, 14);
        let tile = render_web_tile(&source, &colormap_for_kind("ndvi"), 14, x, y).unwrap();
        let bytes = encode_tile_png(&tile).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (TILE_SIZE, TILE_SIZE));
        assert_eq!(decoded.as_raw(), &tile.rgba);
    }
}
