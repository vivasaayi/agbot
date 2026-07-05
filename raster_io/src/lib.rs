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
//! The public API is deliberately backend-agnostic so multiple backends can
//! share one contract: `GeoTiffInfo`, `RasterBand`, `RasterDtype`, and
//! `RasterWindow` carry no reader state, and windowed reads are part of the
//! contract (the local backend materializes the full band internally; the
//! remote COG backend honors the window with range reads).
//!
//! # Remote COG backend (batch 4b, 2026-07; feature `remote`)
//!
//! `RemoteCogReader` reads band windows from tiled Cloud-Optimized GeoTIFFs
//! on object storage (HTTP/S3, e.g. the `sentinel-cogs` bucket) without
//! downloading whole files, via `async-tiff` 0.3 + `object_store`. It shares
//! `GeoTiffInfo`/`RasterBand`/`RasterWindow` and the geokey/geotransform/
//! nodata parsing contract with the local reader, and instruments every
//! fetch (`RemoteFetchMetrics`) for deterministic evidence. The feature is
//! off by default so the sync local path keeps zero async/network deps.

mod error;
mod geotiff;
#[cfg(feature = "remote")]
mod remote;
#[cfg(feature = "test-util")]
pub mod test_util;
mod write;

pub use error::RasterIoError;
pub use geotiff::{GeoTiffInfo, GeoTiffReader, RasterBand, RasterDtype, RasterWindow};
#[cfg(feature = "remote")]
pub use remote::{CogTileLayout, RemoteCogReader, RemoteFetchMetrics};
pub use write::{
    write_geotiff_f32, write_geotiff_i16, write_geotiff_u16, write_geotiff_u8, GeoTiffTags,
};

/// Re-export so `remote`-feature consumers construct/parse `ObjectStore`
/// instances (e.g. an in-memory store in tests) against the exact version
/// `RemoteCogReader` links.
#[cfg(feature = "remote")]
pub use object_store;
