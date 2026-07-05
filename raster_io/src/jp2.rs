//! JPEG 2000 band reading (satellite pipeline batch 24).
//!
//! Sentinel-2 L2A products (Sen2Cor output and ESA downloads) store bands as
//! single-component unsigned JPEG 2000 images — either raw `.j2k`
//! codestreams or `.jp2` container files. This module decodes them to u16
//! digital numbers through the pure-Rust `openjp2` backend of `jpeg2k` (a
//! source port of the reference OpenJPEG library, the same decoder GDAL
//! uses), so no C toolchain or GDAL install is required.
//!
//! Geo-referencing is intentionally NOT read here: Sentinel-2 JP2s carry
//! their grid in the granule's `MTD_TL.xml` (tile geocoding), which callers
//! parse separately. This keeps the decoder a pure pixel reader.

use std::path::Path;

use crate::error::RasterIoError;

/// A decoded single-component JP2 band: unsigned DN, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jp2Gray {
    pub width: u32,
    pub height: u32,
    pub values: Vec<u16>,
}

/// Decode a single-component unsigned JPEG 2000 file (JP2 container or raw
/// J2K codestream, auto-detected) into u16 DN. Multi-component images and
/// precisions above 16 bits are refused with typed errors.
pub fn read_jp2_gray(path: &Path) -> Result<Jp2Gray, RasterIoError> {
    let bytes = std::fs::read(path).map_err(|err| RasterIoError::Open {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let image = jpeg2k::Image::from_bytes(&bytes).map_err(|err| RasterIoError::Decode {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    let components = image.components();
    if components.len() != 1 {
        return Err(RasterIoError::MultiBandUnsupported {
            path: path.to_path_buf(),
            samples_per_pixel: components.len() as u16,
        });
    }
    let component = &components[0];
    if component.precision() > 16 || component.is_signed() {
        return Err(RasterIoError::UnsupportedDtype {
            path: path.to_path_buf(),
            detail: format!(
                "JP2 component precision {} (signed: {}); only unsigned <= 16 bit is supported",
                component.precision(),
                component.is_signed()
            ),
        });
    }
    let (width, height) = (component.width(), component.height());
    let expected = width as usize * height as usize;
    let data = component.data();
    if data.len() != expected {
        return Err(RasterIoError::Decode {
            path: path.to_path_buf(),
            message: format!(
                "component has {} samples, expected {expected} ({width}x{height})",
                data.len()
            ),
        });
    }
    let mut values = Vec::with_capacity(expected);
    for &sample in data {
        if !(0..=i32::from(u16::MAX)).contains(&sample) {
            return Err(RasterIoError::Decode {
                path: path.to_path_buf(),
                message: format!("sample {sample} outside the unsigned 16-bit range"),
            });
        }
        values.push(sample as u16);
    }
    Ok(Jp2Gray {
        width,
        height,
        values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The lossless round-trip test lives in `tests/jp2_roundtrip.rs` behind
    // the `test-util` feature, which provides the reference JP2 encoder.

    #[test]
    fn non_jp2_bytes_are_a_decode_error() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("not_a_band.jp2");
        std::fs::write(&path, b"definitely not jpeg2000").expect("write");
        assert!(matches!(
            read_jp2_gray(&path),
            Err(RasterIoError::Decode { .. })
        ));
        assert!(matches!(
            read_jp2_gray(Path::new("/nonexistent/band.jp2")),
            Err(RasterIoError::Open { .. })
        ));
    }
}
