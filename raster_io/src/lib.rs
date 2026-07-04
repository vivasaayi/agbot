//! GeoTIFF band raster reading (and baseline writing) for the AGBot workspace.
//!
//! Scope: open single-band GeoTIFF rasters from a local path and expose
//! dimensions, dtype (u8/u16/f32), CRS (EPSG code from the GeoKey directory),
//! geotransform (`ModelPixelScaleTag` 33550 + `ModelTiepointTag` 33922),
//! nodata (`GDAL_NODATA` 42113), and full-band or rectangular-window reads.
//! Georeferencing is surfaced as `shared::schemas::RasterSpatialRef` so
//! `imagery_processor` and `geo_hub` share one contract.
//!
//! # Dependency decision (batch 2, 2026-07)
//!
//! The satellite-pipeline design prefers `async-tiff` + `object_store` for
//! HTTP/S3 COG range reads. `async-tiff` 0.3 was evaluated (docs.rs, 2026-07)
//! and is explicitly "async, read-only support for **tiled** TIFF images".
//! Our network-free test fixtures must be written locally with the `tiff`
//! crate encoder, which produces baseline **striped** TIFFs that `async-tiff`
//! cannot decode, and the local-file path is the only requirement for this
//! batch. We therefore use the sync `tiff` crate (0.10) for local files.
//!
//! The public API is deliberately backend-agnostic so an async
//! `object_store`-backed COG reader can be added later without breaking
//! callers: `GeoTiffInfo`, `RasterBand`, `RasterDtype`, and `RasterWindow`
//! carry no reader state, and windowed reads are already part of the
//! contract (the local backend materializes the full band internally; a COG
//! backend will honor the window with range reads).

mod error;
mod geotiff;
mod write;

pub use error::RasterIoError;
pub use geotiff::{GeoTiffInfo, GeoTiffReader, RasterBand, RasterDtype, RasterWindow};
pub use write::{write_geotiff_f32, write_geotiff_u16, write_geotiff_u8, GeoTiffTags};
