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
    F32,
}

impl RasterDtype {
    pub fn name(self) -> &'static str {
        match self {
            RasterDtype::U8 => "u8",
            RasterDtype::U16 => "u16",
            RasterDtype::F32 => "f32",
        }
    }
}

/// A decoded band buffer in its native dtype.
#[derive(Debug, Clone, PartialEq)]
pub enum RasterBand {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
}

impl RasterBand {
    pub fn dtype(&self) -> RasterDtype {
        match self {
            RasterBand::U8(_) => RasterDtype::U8,
            RasterBand::U16(_) => RasterDtype::U16,
            RasterBand::F32(_) => RasterDtype::F32,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            RasterBand::U8(values) => values.len(),
            RasterBand::U16(values) => values.len(),
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
            RasterBand::F32(values) => values.get(index).map(|value| f64::from(*value)),
        }
    }

    /// All pixel values widened to f32 (u8/u16 are exact in f32).
    pub fn to_f32(&self) -> Vec<f32> {
        match self {
            RasterBand::U8(values) => values.iter().map(|value| f32::from(*value)).collect(),
            RasterBand::U16(values) => values.iter().map(|value| f32::from(*value)).collect(),
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
        let crs = self
            .info
            .crs()
            .ok_or_else(|| RasterIoError::MissingGeoreferencing {
                path: self.path.clone(),
                missing: "EPSG code (GeoKeyDirectoryTag)",
            })?;
        let transform =
            self.info
                .geo_transform
                .ok_or_else(|| RasterIoError::MissingGeoreferencing {
                    path: self.path.clone(),
                    missing: "geotransform (ModelPixelScaleTag + ModelTiepointTag)",
                })?;

        let width = f64::from(self.info.width);
        let height = f64::from(self.info.height);
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

        assert_raster_spatial_ref(Some(&spatial_ref), self.info.width, self.info.height).map_err(
            |source| RasterIoError::SpatialRef {
                path: self.path.clone(),
                source,
            },
        )
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
            DecodingResult::F32(values) => Ok(RasterBand::F32(values)),
            other => Err(RasterIoError::UnsupportedDtype {
                path: self.path.clone(),
                detail: format!("decoded buffer variant {other:?} is not u8/u16/f32"),
            }),
        }
    }

    /// Read a rectangular window into a typed buffer (row-major within the
    /// window). The local-file backend materializes the full band and crops;
    /// the future COG backend will translate the window into range reads.
    pub fn read_window(&mut self, window: RasterWindow) -> Result<RasterBand, RasterIoError> {
        let in_bounds = window.width > 0
            && window.height > 0
            && window
                .x
                .checked_add(window.width)
                .is_some_and(|right| right <= self.info.width)
            && window
                .y
                .checked_add(window.height)
                .is_some_and(|bottom| bottom <= self.info.height);
        if !in_bounds {
            return Err(RasterIoError::WindowOutOfBounds {
                window,
                width: self.info.width,
                height: self.info.height,
            });
        }

        let full = self.read_band()?;
        let row_width = self.info.width as usize;
        Ok(match full {
            RasterBand::U8(values) => RasterBand::U8(crop(&values, row_width, window)),
            RasterBand::U16(values) => RasterBand::U16(crop(&values, row_width, window)),
            RasterBand::F32(values) => RasterBand::F32(crop(&values, row_width, window)),
        })
    }
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
    let dtype = match (sample_format, bits_per_sample) {
        (1, 8) => RasterDtype::U8,
        (1, 16) => RasterDtype::U16,
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
        .and_then(|raw| raw.trim().trim_end_matches('\0').trim().parse::<f64>().ok());

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
fn geo_transform_from_tags(
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
