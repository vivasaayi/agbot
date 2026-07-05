//! Baseline (striped, uncompressed) GeoTIFF writing.
//!
//! Primarily used to build deterministic test fixtures and small derived
//! products; COG output stays with the feature-gated GDAL path for now.

use crate::RasterIoError;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use tiff::encoder::{colortype, colortype::ColorType, TiffEncoder, TiffValue};
use tiff::tags::Tag;

const GEOKEY_MODEL_TYPE: u16 = 1024;
const GEOKEY_RASTER_TYPE: u16 = 1025;
const GEOKEY_GEOGRAPHIC_CRS: u16 = 2048;
const GEOKEY_PROJECTED_CRS: u16 = 3072;
const MODEL_TYPE_PROJECTED: u16 = 1;
const MODEL_TYPE_GEOGRAPHIC: u16 = 2;
const RASTER_TYPE_PIXEL_IS_AREA: u16 = 1;

/// Georeferencing tags to stamp on a written GeoTIFF.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GeoTiffTags {
    /// EPSG code; 4000..5000 is written as a geographic CRS key, everything
    /// else as a projected CRS key.
    pub epsg: Option<u32>,
    /// GDAL-order geotransform; must be north-up (no rotation terms).
    pub geo_transform: Option<[f64; 6]>,
    /// Written as the `GDAL_NODATA` ASCII tag.
    pub nodata: Option<f64>,
}

pub fn write_geotiff_u16(
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
    pixels: &[u16],
    tags: &GeoTiffTags,
) -> Result<(), RasterIoError> {
    write_geotiff::<colortype::Gray16>(path.as_ref(), width, height, pixels, tags)
}

pub fn write_geotiff_u8(
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
    pixels: &[u8],
    tags: &GeoTiffTags,
) -> Result<(), RasterIoError> {
    write_geotiff::<colortype::Gray8>(path.as_ref(), width, height, pixels, tags)
}

pub fn write_geotiff_i16(
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
    pixels: &[i16],
    tags: &GeoTiffTags,
) -> Result<(), RasterIoError> {
    write_geotiff::<colortype::GrayI16>(path.as_ref(), width, height, pixels, tags)
}

pub fn write_geotiff_f32(
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
    pixels: &[f32],
    tags: &GeoTiffTags,
) -> Result<(), RasterIoError> {
    write_geotiff::<colortype::Gray32Float>(path.as_ref(), width, height, pixels, tags)
}

fn write_geotiff<C: ColorType>(
    path: &Path,
    width: u32,
    height: u32,
    pixels: &[C::Inner],
    tags: &GeoTiffTags,
) -> Result<(), RasterIoError>
where
    [C::Inner]: TiffValue,
{
    let write_error = |message: String| RasterIoError::Write {
        path: path.to_path_buf(),
        message,
    };

    if pixels.len() != width as usize * height as usize {
        return Err(write_error(format!(
            "pixel buffer length {} does not match {width}x{height}",
            pixels.len()
        )));
    }

    let file = File::create(path).map_err(|err| write_error(err.to_string()))?;
    let mut encoder =
        TiffEncoder::new(BufWriter::new(file)).map_err(|err| write_error(err.to_string()))?;
    let mut image = encoder
        .new_image::<C>(width, height)
        .map_err(|err| write_error(err.to_string()))?;

    if let Some(transform) = tags.geo_transform {
        if transform[2] != 0.0 || transform[4] != 0.0 || transform[1] <= 0.0 || transform[5] >= 0.0
        {
            return Err(RasterIoError::UnsupportedGeoreferencing {
                detail: format!(
                    "geotransform {transform:?} must be north-up with positive x and negative y pixel size"
                ),
            });
        }
        image
            .encoder()
            .write_tag(
                Tag::ModelPixelScaleTag,
                &[transform[1], -transform[5], 0.0][..],
            )
            .map_err(|err| write_error(err.to_string()))?;
        image
            .encoder()
            .write_tag(
                Tag::ModelTiepointTag,
                &[0.0, 0.0, 0.0, transform[0], transform[3], 0.0][..],
            )
            .map_err(|err| write_error(err.to_string()))?;
    }

    if let Some(epsg) = tags.epsg {
        let directory = geokey_directory_for_epsg(epsg)?;
        image
            .encoder()
            .write_tag(Tag::GeoKeyDirectoryTag, &directory[..])
            .map_err(|err| write_error(err.to_string()))?;
    }

    if let Some(nodata) = tags.nodata {
        image
            .encoder()
            .write_tag(Tag::GdalNodata, format!("{nodata}").as_str())
            .map_err(|err| write_error(err.to_string()))?;
    }

    image
        .write_data(pixels)
        .map_err(|err| write_error(err.to_string()))
}

fn geokey_directory_for_epsg(epsg: u32) -> Result<Vec<u16>, RasterIoError> {
    let code = u16::try_from(epsg).map_err(|_| RasterIoError::UnsupportedGeoreferencing {
        detail: format!("EPSG code {epsg} does not fit the GeoKey short-value encoding"),
    })?;
    let geographic = (4000..5000).contains(&epsg);
    let (model_type, crs_key) = if geographic {
        (MODEL_TYPE_GEOGRAPHIC, GEOKEY_GEOGRAPHIC_CRS)
    } else {
        (MODEL_TYPE_PROJECTED, GEOKEY_PROJECTED_CRS)
    };
    Ok(vec![
        1,
        1,
        0,
        3,
        GEOKEY_MODEL_TYPE,
        0,
        1,
        model_type,
        GEOKEY_RASTER_TYPE,
        0,
        1,
        RASTER_TYPE_PIXEL_IS_AREA,
        crs_key,
        0,
        1,
        code,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geokey_directory_encodes_projected_and_geographic_codes() {
        let projected = geokey_directory_for_epsg(32643).expect("projected key set");
        assert_eq!(projected[3], 3);
        assert!(projected.ends_with(&[GEOKEY_PROJECTED_CRS, 0, 1, 32643]));

        let geographic = geokey_directory_for_epsg(4326).expect("geographic key set");
        assert!(geographic.ends_with(&[GEOKEY_GEOGRAPHIC_CRS, 0, 1, 4326]));

        assert!(matches!(
            geokey_directory_for_epsg(100_000),
            Err(RasterIoError::UnsupportedGeoreferencing { .. })
        ));
    }

    #[test]
    fn rotated_geotransform_is_rejected() {
        let tags = GeoTiffTags {
            epsg: Some(32643),
            geo_transform: Some([0.0, 10.0, 0.5, 0.0, 0.0, -10.0]),
            nodata: None,
        };
        let path = std::env::temp_dir().join("raster_io_rotated_reject.tif");
        let error = write_geotiff_u16(&path, 2, 2, &[0; 4], &tags).unwrap_err();
        assert!(matches!(
            error,
            RasterIoError::UnsupportedGeoreferencing { .. }
        ));
    }
}
