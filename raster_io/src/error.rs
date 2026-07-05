use crate::geotiff::RasterWindow;
use shared::schemas::RasterSpatialRefError;
use std::path::PathBuf;

/// Reason-coded errors for GeoTIFF raster I/O.
#[derive(Debug, thiserror::Error)]
pub enum RasterIoError {
    #[error("failed to open raster {path}: {message}")]
    Open { path: PathBuf, message: String },
    #[error("failed to decode raster {path}: {message}")]
    Decode { path: PathBuf, message: String },
    #[error("unsupported raster dtype in {path}: {detail}")]
    UnsupportedDtype { path: PathBuf, detail: String },
    #[error(
        "raster {path} has {samples_per_pixel} samples per pixel; only single-band rasters are supported"
    )]
    MultiBandUnsupported {
        path: PathBuf,
        samples_per_pixel: u16,
    },
    #[error("raster {path} is not georeferenced: missing {missing}")]
    MissingGeoreferencing {
        path: PathBuf,
        missing: &'static str,
    },
    #[error("georeferencing invalid for {path}: {source}")]
    SpatialRef {
        path: PathBuf,
        source: RasterSpatialRefError,
    },
    #[error("window {window:?} exceeds raster bounds {width}x{height}")]
    WindowOutOfBounds {
        window: RasterWindow,
        width: u32,
        height: u32,
    },
    #[error("raster {path} is not tiled; the remote COG backend requires a tiled GeoTIFF")]
    NotTiled { path: PathBuf },
    #[error("failed to write raster {path}: {message}")]
    Write { path: PathBuf, message: String },
    #[error("unsupported georeferencing for GeoTIFF write: {detail}")]
    UnsupportedGeoreferencing { detail: String },
}
