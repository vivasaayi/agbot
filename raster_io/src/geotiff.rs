//! Local-file GeoTIFF band reader.
//!
//! Reads single-band striped or tiled baseline GeoTIFFs via the `tiff` crate
//! and parses the GeoTIFF georeferencing tags into `RasterSpatialRef`.

use crate::RasterIoError;
use shared::schemas::{assert_raster_spatial_ref, GeoBounds, RasterSpatialRef};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use tiff::decoder::{Decoder, DecodingResult};
use tiff::tags::Tag;

/// GeoKey IDs (GeoTIFF 1.1) resolved from `GeoKeyDirectoryTag` (34735).
const GEOKEY_GEOGRAPHIC_CRS: u16 = 2048;
const GEOKEY_PROJECTED_CRS: u16 = 3072;
/// GeoTIFF "user-defined" sentinel; not a real EPSG code.
const GEOKEY_USER_DEFINED: u16 = 32767;

/// Pixel data type of a raster band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterDtype {
    U8,
    U16,
    /// Signed 16-bit — Landsat C2, Sentinel-2, and HLS surface reflectance
    /// are Int16 (scaled DN with a signed fill, e.g. -9999).
    I16,
    F32,
}

impl RasterDtype {
    pub fn name(self) -> &'static str {
        match self {
            RasterDtype::U8 => "u8",
            RasterDtype::U16 => "u16",
            RasterDtype::I16 => "i16",
            RasterDtype::F32 => "f32",
        }
    }

    /// True for the integer dtypes (scaled DN products that a caller may
    /// need to convert to physical units with a scale/offset).
    pub fn is_integer(self) -> bool {
        matches!(self, RasterDtype::U8 | RasterDtype::U16 | RasterDtype::I16)
    }
}

/// A decoded band buffer in its native dtype.
#[derive(Debug, Clone, PartialEq)]
pub enum RasterBand {
    U8(Vec<u8>),
    U16(Vec<u16>),
    I16(Vec<i16>),
    F32(Vec<f32>),
}

impl RasterBand {
    pub fn dtype(&self) -> RasterDtype {
        match self {
            RasterBand::U8(_) => RasterDtype::U8,
            RasterBand::U16(_) => RasterDtype::U16,
            RasterBand::I16(_) => RasterDtype::I16,
            RasterBand::F32(_) => RasterDtype::F32,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            RasterBand::U8(values) => values.len(),
            RasterBand::U16(values) => values.len(),
            RasterBand::I16(values) => values.len(),
            RasterBand::F32(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_u16(&self) -> Option<&[u16]> {
        match self {
            RasterBand::U16(values) => Some(values),
            _ => None,
        }
    }

    /// Native pixel value at `index` widened to f64, if in bounds.
    pub fn value_as_f64(&self, index: usize) -> Option<f64> {
        match self {
            RasterBand::U8(values) => values.get(index).map(|value| f64::from(*value)),
            RasterBand::U16(values) => values.get(index).map(|value| f64::from(*value)),
            RasterBand::I16(values) => values.get(index).map(|value| f64::from(*value)),
            RasterBand::F32(values) => values.get(index).map(|value| f64::from(*value)),
        }
    }

    /// All pixel values widened to f32 (u8/u16/i16 are exact in f32).
    pub fn to_f32(&self) -> Vec<f32> {
        match self {
            RasterBand::U8(values) => values.iter().map(|value| f32::from(*value)).collect(),
            RasterBand::U16(values) => values.iter().map(|value| f32::from(*value)).collect(),
            RasterBand::I16(values) => values.iter().map(|value| f32::from(*value)).collect(),
            RasterBand::F32(values) => values.clone(),
        }
    }
}

/// Rectangular pixel window (origin top-left, row-major).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterWindow {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Structural + georeferencing metadata parsed from a GeoTIFF.
#[derive(Debug, Clone, PartialEq)]
pub struct GeoTiffInfo {
    pub width: u32,
    pub height: u32,
    pub dtype: RasterDtype,
    /// EPSG code from the GeoKey directory (projected key preferred).
    pub epsg: Option<u32>,
    /// GDAL-order geotransform derived from ModelPixelScale + ModelTiepoint.
    pub geo_transform: Option<[f64; 6]>,
    /// Parsed `GDAL_NODATA` tag value.
    pub nodata: Option<f64>,
}

impl GeoTiffInfo {
    /// CRS in the workspace-standard `EPSG:<code>` string form.
    pub fn crs(&self) -> Option<String> {
        self.epsg.map(|code| format!("EPSG:{code}"))
    }
}

/// Sync local-file GeoTIFF reader. See the crate docs for the backend
/// decision; the API shape is shared with the future async COG backend.
pub struct GeoTiffReader {
    path: PathBuf,
    decoder: Decoder<BufReader<File>>,
    info: GeoTiffInfo,
}

impl GeoTiffReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RasterIoError> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(|err| RasterIoError::Open {
            path: path.clone(),
            message: err.to_string(),
        })?;
        let mut decoder =
            Decoder::new(BufReader::new(file)).map_err(|err| RasterIoError::Open {
                path: path.clone(),
                message: err.to_string(),
            })?;

        let info = parse_geotiff_info(&mut decoder, &path)?;
        Ok(Self {
            path,
            decoder,
            info,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn info(&self) -> &GeoTiffInfo {
        &self.info
    }

    /// Full georeferencing as the workspace-standard `RasterSpatialRef`,
    /// validated through `shared::schemas::assert_raster_spatial_ref`.
    pub fn spatial_ref(&self) -> Result<RasterSpatialRef, RasterIoError> {
        spatial_ref_from_info(&self.info, &self.path)
    }

    /// Read the whole band into a typed buffer (row-major).
    pub fn read_band(&mut self) -> Result<RasterBand, RasterIoError> {
        let decoded = self
            .decoder
            .read_image()
            .map_err(|err| RasterIoError::Decode {
                path: self.path.clone(),
                message: err.to_string(),
            })?;
        match decoded {
            DecodingResult::U8(values) => Ok(RasterBand::U8(values)),
            DecodingResult::U16(values) => Ok(RasterBand::U16(values)),
            DecodingResult::I16(values) => Ok(RasterBand::I16(values)),
            DecodingResult::F32(values) => Ok(RasterBand::F32(values)),
            other => Err(RasterIoError::UnsupportedDtype {
                path: self.path.clone(),
                detail: format!("decoded buffer variant {other:?} is not u8/u16/i16/f32"),
            }),
        }
    }

    /// Read a rectangular window into a typed buffer (row-major within the
    /// window). The local-file backend materializes the full band and crops;
    /// the remote COG backend (`RemoteCogReader`) translates the window into
    /// range reads instead.
    pub fn read_window(&mut self, window: RasterWindow) -> Result<RasterBand, RasterIoError> {
        validate_window(window, self.info.width, self.info.height)?;

        let full = self.read_band()?;
        let row_width = self.info.width as usize;
        Ok(match full {
            RasterBand::U8(values) => RasterBand::U8(crop(&values, row_width, window)),
            RasterBand::U16(values) => RasterBand::U16(crop(&values, row_width, window)),
            RasterBand::I16(values) => RasterBand::I16(crop(&values, row_width, window)),
            RasterBand::F32(values) => RasterBand::F32(crop(&values, row_width, window)),
        })
    }
}

/// Validate that `window` is non-empty and fully inside a `width`x`height`
/// raster. Shared by the local and remote backends so both report the same
/// reason-coded error.
pub(crate) fn validate_window(
    window: RasterWindow,
    width: u32,
    height: u32,
) -> Result<(), RasterIoError> {
    let in_bounds = window.width > 0
        && window.height > 0
        && window
            .x
            .checked_add(window.width)
            .is_some_and(|right| right <= width)
        && window
            .y
            .checked_add(window.height)
            .is_some_and(|bottom| bottom <= height);
    if in_bounds {
        Ok(())
    } else {
        Err(RasterIoError::WindowOutOfBounds {
            window,
            width,
            height,
        })
    }
}

/// Build the workspace-standard `RasterSpatialRef` from parsed GeoTIFF
/// metadata, validated through `shared::schemas::assert_raster_spatial_ref`.
/// Shared by the local and remote backends.
pub(crate) fn spatial_ref_from_info(
    info: &GeoTiffInfo,
    path: &Path,
) -> Result<RasterSpatialRef, RasterIoError> {
    let crs = info
        .crs()
        .ok_or_else(|| RasterIoError::MissingGeoreferencing {
            path: path.to_path_buf(),
            missing: "EPSG code (GeoKeyDirectoryTag)",
        })?;
    let transform = info
        .geo_transform
        .ok_or_else(|| RasterIoError::MissingGeoreferencing {
            path: path.to_path_buf(),
            missing: "geotransform (ModelPixelScaleTag + ModelTiepointTag)",
        })?;

    let width = f64::from(info.width);
    let height = f64::from(info.height);
    let min_x = transform[0];
    let max_x = transform[0] + width * transform[1];
    let max_y = transform[3];
    let min_y = transform[3] + height * transform[5];
    let spatial_ref = RasterSpatialRef {
        georeferenced: true,
        crs: Some(crs),
        bbox: Some(GeoBounds {
            min_lon: min_x.min(max_x),
            min_lat: min_y.min(max_y),
            max_lon: min_x.max(max_x),
            max_lat: min_y.max(max_y),
        }),
        geo_transform: Some(transform),
        resolution: None,
    };

    assert_raster_spatial_ref(Some(&spatial_ref), info.width, info.height).map_err(|source| {
        RasterIoError::SpatialRef {
            path: path.to_path_buf(),
            source,
        }
    })
}

/// Parse the `GDAL_NODATA` ASCII tag payload (possibly NUL-terminated).
pub(crate) fn parse_gdal_nodata(raw: &str) -> Option<f64> {
    raw.trim().trim_end_matches('\0').trim().parse::<f64>().ok()
}

fn crop<T: Copy>(values: &[T], row_width: usize, window: RasterWindow) -> Vec<T> {
    let mut out = Vec::with_capacity(window.width as usize * window.height as usize);
    for row in 0..window.height as usize {
        let start = (window.y as usize + row) * row_width + window.x as usize;
        out.extend_from_slice(&values[start..start + window.width as usize]);
    }
    out
}

fn parse_geotiff_info(
    decoder: &mut Decoder<BufReader<File>>,
    path: &Path,
) -> Result<GeoTiffInfo, RasterIoError> {
    let decode_error = |err: tiff::TiffError| RasterIoError::Decode {
        path: path.to_path_buf(),
        message: err.to_string(),
    };

    let (width, height) = decoder.dimensions().map_err(decode_error)?;

    let samples_per_pixel: u16 = decoder
        .find_tag_unsigned(Tag::SamplesPerPixel)
        .map_err(decode_error)?
        .unwrap_or(1);
    if samples_per_pixel != 1 {
        return Err(RasterIoError::MultiBandUnsupported {
            path: path.to_path_buf(),
            samples_per_pixel,
        });
    }

    let bits_per_sample: u16 = decoder
        .find_tag_unsigned_vec(Tag::BitsPerSample)
        .map_err(decode_error)?
        .and_then(|bits: Vec<u16>| bits.first().copied())
        .unwrap_or(1);
    let sample_format: u16 = decoder
        .find_tag_unsigned_vec(Tag::SampleFormat)
        .map_err(decode_error)?
        .and_then(|formats: Vec<u16>| formats.first().copied())
        .unwrap_or(1);
    // TIFF SampleFormat: 1 = unsigned int, 2 = signed int, 3 = IEEE float.
    let dtype = match (sample_format, bits_per_sample) {
        (1, 8) => RasterDtype::U8,
        (1, 16) => RasterDtype::U16,
        (2, 16) => RasterDtype::I16,
        (3, 32) => RasterDtype::F32,
        (format, bits) => {
            return Err(RasterIoError::UnsupportedDtype {
                path: path.to_path_buf(),
                detail: format!("sample_format={format}, bits_per_sample={bits}"),
            })
        }
    };

    let pixel_scale = decoder
        .find_tag(Tag::ModelPixelScaleTag)
        .map_err(decode_error)?
        .map(|value| value.into_f64_vec())
        .transpose()
        .map_err(decode_error)?;
    let tiepoint = decoder
        .find_tag(Tag::ModelTiepointTag)
        .map_err(decode_error)?
        .map(|value| value.into_f64_vec())
        .transpose()
        .map_err(decode_error)?;
    let geo_transform = geo_transform_from_tags(pixel_scale.as_deref(), tiepoint.as_deref());

    let geokey_directory = decoder
        .find_tag(Tag::GeoKeyDirectoryTag)
        .map_err(decode_error)?
        .map(|value| value.into_u16_vec())
        .transpose()
        .map_err(decode_error)?;
    let epsg = geokey_directory
        .as_deref()
        .and_then(epsg_from_geokey_directory);

    let nodata = decoder
        .find_tag(Tag::GdalNodata)
        .map_err(decode_error)?
        .map(|value| value.into_string())
        .transpose()
        .map_err(decode_error)?
        .and_then(|raw| parse_gdal_nodata(&raw));

    Ok(GeoTiffInfo {
        width,
        height,
        dtype,
        epsg,
        geo_transform,
        nodata,
    })
}

/// GDAL-order geotransform from ModelPixelScale [sx, sy, sz] and a raster→
/// model tiepoint [i, j, k, x, y, z]. North-up only (no rotation terms).
/// Shared by the local and remote backends.
pub(crate) fn geo_transform_from_tags(
    pixel_scale: Option<&[f64]>,
    tiepoint: Option<&[f64]>,
) -> Option<[f64; 6]> {
    let scale = pixel_scale?;
    let tie = tiepoint?;
    if scale.len() < 2 || tie.len() < 6 {
        return None;
    }
    let (scale_x, scale_y) = (scale[0], scale[1]);
    let (raster_i, raster_j, model_x, model_y) = (tie[0], tie[1], tie[3], tie[4]);
    Some([
        model_x - raster_i * scale_x,
        scale_x,
        0.0,
        model_y + raster_j * scale_y,
        0.0,
        -scale_y,
    ])
}

/// Resolve the EPSG code from a GeoKey directory: header [version, rev,
/// minor, count] followed by `count` entries of [key_id, location, count,
/// value]. Only inline (location==0) short values are consulted; the
/// projected CRS key wins over the geographic one.
fn epsg_from_geokey_directory(directory: &[u16]) -> Option<u32> {
    if directory.len() < 4 {
        return None;
    }
    let entry_count = directory[3] as usize;
    let mut projected = None;
    let mut geographic = None;
    for entry in directory[4..].chunks_exact(4).take(entry_count) {
        let (key_id, location, value) = (entry[0], entry[1], entry[3]);
        if location != 0 || value == 0 || value == GEOKEY_USER_DEFINED {
            continue;
        }
        match key_id {
            GEOKEY_PROJECTED_CRS => projected = Some(u32::from(value)),
            GEOKEY_GEOGRAPHIC_CRS => geographic = Some(u32::from(value)),
            _ => {}
        }
    }
    projected.or(geographic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{write_geotiff_i16, GeoTiffTags};

    #[test]
    fn int16_geotiff_round_trips_signed_values_and_widens_to_f32() {
        // Landsat/Sentinel/HLS-style Int16 surface reflectance with a signed
        // -9999 fill: read back the exact signed values, and to_f32() widens
        // them losslessly.
        let dir = std::env::temp_dir().join("raster_io_i16_roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("i16.tif");
        let pixels: [i16; 4] = [2000, 6000, -9999, 0];
        write_geotiff_i16(
            &path,
            2,
            2,
            &pixels,
            &GeoTiffTags {
                epsg: Some(32643),
                geo_transform: Some([600_000.0, 30.0, 0.0, 1_300_020.0, 0.0, -30.0]),
                nodata: Some(-9999.0),
            },
        )
        .unwrap();

        let mut reader = GeoTiffReader::open(&path).unwrap();
        assert_eq!(reader.info().epsg, Some(32643));
        assert_eq!(reader.info().nodata, Some(-9999.0));
        let band = reader.read_band().unwrap();
        assert_eq!(band.dtype(), RasterDtype::I16);
        assert!(band.dtype().is_integer());
        assert_eq!(band, RasterBand::I16(pixels.to_vec()));
        assert_eq!(band.value_as_f64(2), Some(-9999.0));
        assert_eq!(band.to_f32(), vec![2000.0, 6000.0, -9999.0, 0.0]);

        // Windowed read crops the signed buffer.
        let window = reader
            .read_window(RasterWindow {
                x: 1,
                y: 0,
                width: 1,
                height: 1,
            })
            .unwrap();
        assert_eq!(window, RasterBand::I16(vec![6000]));
    }

    #[test]
    fn geokey_directory_prefers_projected_crs_and_skips_user_defined() {
        // header + [geographic 4326, projected 32643]
        let directory = [
            1,
            1,
            0,
            2, //
            GEOKEY_GEOGRAPHIC_CRS,
            0,
            1,
            4326, //
            GEOKEY_PROJECTED_CRS,
            0,
            1,
            32643,
        ];
        assert_eq!(epsg_from_geokey_directory(&directory), Some(32643));

        let geographic_only = [1, 1, 0, 1, GEOKEY_GEOGRAPHIC_CRS, 0, 1, 4326];
        assert_eq!(epsg_from_geokey_directory(&geographic_only), Some(4326));

        let user_defined = [1, 1, 0, 1, GEOKEY_PROJECTED_CRS, 0, 1, GEOKEY_USER_DEFINED];
        assert_eq!(epsg_from_geokey_directory(&user_defined), None);
        assert_eq!(epsg_from_geokey_directory(&[1, 1, 0]), None);
    }

    #[test]
    fn geo_transform_derives_gdal_order_from_scale_and_tiepoint() {
        let transform = geo_transform_from_tags(
            Some(&[10.0, 10.0, 0.0]),
            Some(&[0.0, 0.0, 0.0, 500_000.0, 4_300_000.0, 0.0]),
        )
        .expect("transform should derive");
        assert_eq!(transform, [500_000.0, 10.0, 0.0, 4_300_000.0, 0.0, -10.0]);

        // Tiepoint anchored away from the raster origin shifts the origin back.
        let shifted = geo_transform_from_tags(
            Some(&[10.0, 10.0, 0.0]),
            Some(&[2.0, 3.0, 0.0, 500_020.0, 4_299_970.0, 0.0]),
        )
        .expect("transform should derive");
        assert_eq!(shifted, [500_000.0, 10.0, 0.0, 4_300_000.0, 0.0, -10.0]);

        assert_eq!(geo_transform_from_tags(None, None), None);
        assert_eq!(
            geo_transform_from_tags(Some(&[10.0]), Some(&[0.0; 6])),
            None
        );
    }
}
