//! Remote Cloud-Optimized GeoTIFF (COG) reading over HTTP/S3 range requests.
//!
//! Feature: `remote`. Backend: `async-tiff` 0.3 (Development Seed) +
//! `object_store` 0.13.
//!
//! # Dependency decision (batch 4b, 2026-07)
//!
//! `async-tiff` 0.3 was re-evaluated for the remote path (it was rejected in
//! batch 2 only because it decodes **tiled** TIFFs while our local fixtures
//! are striped). Real COGs — the Sentinel-2 archive on
//! `sentinel-cogs.s3.us-west-2.amazonaws.com` and Planetary Computer assets —
//! are tiled by definition, so that limitation does not apply here. Source
//! review of the vendored 0.3.0 crate confirmed everything this backend
//! needs: `ImageFileDirectory` exposes `model_pixel_scale`/`model_tiepoint`
//! (tags 33550/33922), `geo_key_directory` (34735, with `epsg_code()`),
//! `gdal_nodata` (42113), tile layout + `fetch_tile(s)` over an
//! `AsyncFileReader`, and `Tile::decode` handles DEFLATE (zlib), LZW, ZSTD
//! and uncompressed data for u8/u16/f32 with endianness fix-up. Hand-rolling
//! a COG parser was therefore unnecessary.
//!
//! The `remote` feature is **off by default**: the sync local path used by
//! `imagery_processor` keeps zero async/network dependencies, and only
//! consumers that actually stream satellite COGs opt in.
//!
//! # Evidence instrumentation
//!
//! Every byte fetched from object storage flows through a counting
//! `AsyncFileReader` wrapper, so `RemoteCogReader::fetch_metrics()` reports
//! cumulative range-request and byte counts (deterministic-evidence
//! doctrine). Batched tile fetches coalesce exactly contiguous/overlapping
//! byte ranges into single requests before hitting the store.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_tiff::decoder::DecoderRegistry;
use async_tiff::error::AsyncTiffResult;
use async_tiff::metadata::cache::ReadaheadMetadataCache;
use async_tiff::metadata::TiffMetadataReader;
use async_tiff::reader::{AsyncFileReader, ObjectReader};
use async_tiff::tags::SampleFormat;
use async_tiff::{ImageFileDirectory, TypedArray};
use bytes::Bytes;
use object_store::path::Path as ObjectPath;
use object_store::ObjectStore;
use shared::schemas::RasterSpatialRef;

use crate::geotiff::{
    geo_transform_from_tags, parse_gdal_nodata, spatial_ref_from_info, validate_window,
    GeoTiffInfo, RasterBand, RasterDtype, RasterWindow,
};
use crate::RasterIoError;

/// GeoTIFF "user-defined" sentinel; not a real EPSG code.
const GEOKEY_USER_DEFINED: u16 = 32767;

/// Cumulative object-store fetch evidence for a [`RemoteCogReader`].
///
/// Counters are cumulative over the reader's lifetime (opening the file
/// counts too); callers assert per-read costs by diffing snapshots.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemoteFetchMetrics {
    /// Number of range requests issued to the object store.
    pub range_requests: u64,
    /// Total bytes returned by those requests.
    pub bytes_fetched: u64,
}

/// Tile layout of the full-resolution IFD of a COG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CogTileLayout {
    pub tile_width: u32,
    pub tile_height: u32,
    pub tiles_across: u32,
    pub tiles_down: u32,
}

/// `AsyncFileReader` wrapper that counts range requests/bytes and coalesces
/// exactly contiguous or overlapping ranges in batched fetches.
#[derive(Debug, Clone)]
struct CountingReader {
    inner: ObjectReader,
    range_requests: Arc<AtomicU64>,
    bytes_fetched: Arc<AtomicU64>,
}

impl CountingReader {
    fn new(inner: ObjectReader) -> Self {
        Self {
            inner,
            range_requests: Arc::new(AtomicU64::new(0)),
            bytes_fetched: Arc::new(AtomicU64::new(0)),
        }
    }

    fn metrics(&self) -> RemoteFetchMetrics {
        RemoteFetchMetrics {
            range_requests: self.range_requests.load(Ordering::Relaxed),
            bytes_fetched: self.bytes_fetched.load(Ordering::Relaxed),
        }
    }
}

#[async_trait::async_trait]
impl AsyncFileReader for CountingReader {
    async fn get_bytes(&self, range: Range<u64>) -> AsyncTiffResult<Bytes> {
        let bytes = self.inner.get_bytes(range).await?;
        self.range_requests.fetch_add(1, Ordering::Relaxed);
        self.bytes_fetched
            .fetch_add(bytes.len() as u64, Ordering::Relaxed);
        Ok(bytes)
    }

    async fn get_byte_ranges(&self, ranges: Vec<Range<u64>>) -> AsyncTiffResult<Vec<Bytes>> {
        let merged = coalesce_ranges(&ranges);
        let mut fetched = Vec::with_capacity(merged.len());
        for merged_range in &merged {
            // Routed through `get_bytes` so counting lives in one place.
            fetched.push((
                merged_range.clone(),
                self.get_bytes(merged_range.clone()).await?,
            ));
        }
        ranges
            .iter()
            .map(|range| {
                let (merged_range, bytes) = fetched
                    .iter()
                    .find(|(merged_range, _)| {
                        merged_range.start <= range.start && range.end <= merged_range.end
                    })
                    .expect("every requested range is covered by a coalesced fetch");
                let offset = (range.start - merged_range.start) as usize;
                Ok(bytes.slice(offset..offset + (range.end - range.start) as usize))
            })
            .collect()
    }
}

/// Merge byte ranges that touch or overlap once sorted by start offset.
/// COG tiles adjacent in the file (the common layout) collapse into one
/// request; disjoint ranges stay separate — no over-reading of gap bytes.
fn coalesce_ranges(ranges: &[Range<u64>]) -> Vec<Range<u64>> {
    let mut sorted: Vec<Range<u64>> = ranges.iter().filter(|r| r.end > r.start).cloned().collect();
    sorted.sort_by_key(|range| (range.start, range.end));
    let mut merged: Vec<Range<u64>> = Vec::with_capacity(sorted.len());
    for range in sorted {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// Async reader for a single-band Cloud-Optimized GeoTIFF on object storage.
///
/// Exposes the same metadata surface as the local [`crate::GeoTiffReader`]
/// (`GeoTiffInfo`, `spatial_ref()`, typed `RasterBand` reads) but fetches
/// only the byte ranges needed: header + first IFD on open (one ~32 KiB
/// readahead request for typical COGs), then exactly the tiles overlapping
/// each requested window.
#[derive(Debug)]
pub struct RemoteCogReader {
    /// Object location, kept for reason-coded errors.
    location: PathBuf,
    reader: CountingReader,
    ifd: ImageFileDirectory,
    info: GeoTiffInfo,
    layout: CogTileLayout,
    decoders: DecoderRegistry,
}

impl RemoteCogReader {
    /// Open a COG at `location` inside `store`, reading only the TIFF header
    /// and the first (full-resolution) IFD. Overview IFDs are not read.
    pub async fn open(store: Arc<dyn ObjectStore>, location: &str) -> Result<Self, RasterIoError> {
        let path = PathBuf::from(location);
        let open_error = |message: String| RasterIoError::Open {
            path: path.clone(),
            message,
        };

        let object_path = ObjectPath::parse(location).map_err(|err| open_error(err.to_string()))?;
        let reader = CountingReader::new(ObjectReader::new(store, object_path));
        // Readahead cache: one 32 KiB initial fetch covers header + first IFD
        // for typical COGs; larger metadata grows the fetch geometrically.
        let cache = ReadaheadMetadataCache::new(reader.clone());
        let mut metadata = TiffMetadataReader::try_open(&cache)
            .await
            .map_err(|err| open_error(err.to_string()))?;
        let ifd = metadata
            .read_next_ifd(&cache)
            .await
            .map_err(|err| open_error(err.to_string()))?
            .ok_or_else(|| open_error("TIFF contains no IFD".to_string()))?;

        Self::from_parts(path, reader, ifd)
    }

    /// Convenience: open a COG from a plain URL, e.g.
    /// `https://sentinel-cogs.s3.us-west-2.amazonaws.com/.../B04.tif`.
    /// HTTP(S) URLs are supported; other schemes require constructing the
    /// matching `ObjectStore` and calling [`Self::open`].
    pub async fn from_url(url: &str) -> Result<Self, RasterIoError> {
        let open_error = |message: String| RasterIoError::Open {
            path: PathBuf::from(url),
            message,
        };
        let parsed = url::Url::parse(url).map_err(|err| open_error(err.to_string()))?;
        let (store, object_path) =
            object_store::parse_url(&parsed).map_err(|err| open_error(err.to_string()))?;
        Self::open(Arc::from(store), object_path.as_ref()).await
    }

    fn from_parts(
        location: PathBuf,
        reader: CountingReader,
        ifd: ImageFileDirectory,
    ) -> Result<Self, RasterIoError> {
        let samples_per_pixel = ifd.samples_per_pixel();
        if samples_per_pixel != 1 {
            return Err(RasterIoError::MultiBandUnsupported {
                path: location,
                samples_per_pixel,
            });
        }

        let bits_per_sample = ifd.bits_per_sample().first().copied().unwrap_or(1);
        let sample_format = ifd
            .sample_format()
            .first()
            .copied()
            .unwrap_or(SampleFormat::Uint);
        let dtype = match (sample_format, bits_per_sample) {
            (SampleFormat::Uint, 8) => RasterDtype::U8,
            (SampleFormat::Uint, 16) => RasterDtype::U16,
            (SampleFormat::Float, 32) => RasterDtype::F32,
            (format, bits) => {
                return Err(RasterIoError::UnsupportedDtype {
                    path: location,
                    detail: format!("sample_format={format:?}, bits_per_sample={bits}"),
                })
            }
        };

        let (tile_width, tile_height, tile_counts) = match (
            ifd.tile_width(),
            ifd.tile_height(),
            ifd.tile_count(),
            ifd.tile_offsets(),
        ) {
            (Some(width), Some(height), Some(counts), Some(_)) => (width, height, counts),
            _ => return Err(RasterIoError::NotTiled { path: location }),
        };
        let layout = CogTileLayout {
            tile_width,
            tile_height,
            tiles_across: tile_counts.0 as u32,
            tiles_down: tile_counts.1 as u32,
        };

        let epsg = ifd
            .geo_key_directory()
            .and_then(|keys| keys.epsg_code())
            .filter(|code| *code != 0 && *code != GEOKEY_USER_DEFINED)
            .map(u32::from);
        let geo_transform = geo_transform_from_tags(ifd.model_pixel_scale(), ifd.model_tiepoint());
        let nodata = ifd.gdal_nodata().and_then(parse_gdal_nodata);

        let info = GeoTiffInfo {
            width: ifd.image_width(),
            height: ifd.image_height(),
            dtype,
            epsg,
            geo_transform,
            nodata,
        };

        Ok(Self {
            location,
            reader,
            ifd,
            info,
            layout,
            decoders: DecoderRegistry::default(),
        })
    }

    /// Object location this reader was opened on.
    pub fn location(&self) -> &std::path::Path {
        &self.location
    }

    /// Structural + georeferencing metadata (same shape as the local reader).
    pub fn info(&self) -> &GeoTiffInfo {
        &self.info
    }

    /// Tile layout of the full-resolution IFD.
    pub fn tile_layout(&self) -> CogTileLayout {
        self.layout
    }

    /// Cumulative fetch evidence (range requests + bytes) for this reader.
    pub fn fetch_metrics(&self) -> RemoteFetchMetrics {
        self.reader.metrics()
    }

    /// Full georeferencing as the workspace-standard `RasterSpatialRef`,
    /// validated through `shared::schemas::assert_raster_spatial_ref`.
    pub fn spatial_ref(&self) -> Result<RasterSpatialRef, RasterIoError> {
        spatial_ref_from_info(&self.info, &self.location)
    }

    /// Read the whole band. Intended for small rasters (fetches every tile).
    pub async fn read_band(&self) -> Result<RasterBand, RasterIoError> {
        self.read_window(RasterWindow {
            x: 0,
            y: 0,
            width: self.info.width,
            height: self.info.height,
        })
        .await
    }

    /// Read a rectangular window (row-major within the window), fetching and
    /// decoding only the tiles that overlap it. Adjacent tile byte ranges are
    /// coalesced into single range requests.
    pub async fn read_window(&self, window: RasterWindow) -> Result<RasterBand, RasterIoError> {
        validate_window(window, self.info.width, self.info.height)?;

        let tile_width = self.layout.tile_width as usize;
        let tile_height = self.layout.tile_height as usize;
        let first_tile_x = window.x as usize / tile_width;
        let last_tile_x = (window.x + window.width - 1) as usize / tile_width;
        let first_tile_y = window.y as usize / tile_height;
        let last_tile_y = (window.y + window.height - 1) as usize / tile_height;

        let mut tile_indices = Vec::new();
        for tile_y in first_tile_y..=last_tile_y {
            for tile_x in first_tile_x..=last_tile_x {
                tile_indices.push((tile_x, tile_y));
            }
        }

        let tiles = self
            .ifd
            .fetch_tiles(&tile_indices, &self.reader)
            .await
            .map_err(|err| RasterIoError::Decode {
                path: self.location.clone(),
                message: err.to_string(),
            })?;

        let pixel_count = window.width as usize * window.height as usize;
        let mut band = match self.info.dtype {
            RasterDtype::U8 => RasterBand::U8(vec![0; pixel_count]),
            RasterDtype::U16 => RasterBand::U16(vec![0; pixel_count]),
            RasterDtype::F32 => RasterBand::F32(vec![0.0; pixel_count]),
        };

        for tile in tiles {
            let (tile_x, tile_y) = (tile.x(), tile.y());
            let array = tile
                .decode(&self.decoders)
                .map_err(|err| RasterIoError::Decode {
                    path: self.location.clone(),
                    message: err.to_string(),
                })?;
            // Decoded tiles are always the full padded tile size; edge
            // padding is cropped by the window copy below.
            match (array.data(), &mut band) {
                (TypedArray::UInt8(values), RasterBand::U8(out)) => copy_tile_into_window(
                    out,
                    values,
                    window,
                    tile_x,
                    tile_y,
                    tile_width,
                    tile_height,
                ),
                (TypedArray::UInt16(values), RasterBand::U16(out)) => copy_tile_into_window(
                    out,
                    values,
                    window,
                    tile_x,
                    tile_y,
                    tile_width,
                    tile_height,
                ),
                (TypedArray::Float32(values), RasterBand::F32(out)) => copy_tile_into_window(
                    out,
                    values,
                    window,
                    tile_x,
                    tile_y,
                    tile_width,
                    tile_height,
                ),
                (other, _) => {
                    return Err(RasterIoError::UnsupportedDtype {
                        path: self.location.clone(),
                        detail: format!(
                            "tile ({tile_x},{tile_y}) decoded as {other:?} but band dtype is {}",
                            self.info.dtype.name()
                        ),
                    })
                }
            }
        }

        Ok(band)
    }
}

/// Copy the intersection of a decoded tile with `window` into the window
/// buffer. `tile_values` is the full padded tile (`tile_width` x
/// `tile_height`, row-major); `dst` is the window buffer (row-major).
#[allow(clippy::too_many_arguments)]
fn copy_tile_into_window<T: Copy>(
    dst: &mut [T],
    tile_values: &[T],
    window: RasterWindow,
    tile_x: usize,
    tile_y: usize,
    tile_width: usize,
    tile_height: usize,
) {
    // Tile origin in image pixel coordinates.
    let tile_origin_x = tile_x * tile_width;
    let tile_origin_y = tile_y * tile_height;
    let window_x = window.x as usize;
    let window_y = window.y as usize;

    let start_x = window_x.max(tile_origin_x);
    let end_x = (window_x + window.width as usize).min(tile_origin_x + tile_width);
    let start_y = window_y.max(tile_origin_y);
    let end_y = (window_y + window.height as usize).min(tile_origin_y + tile_height);
    if start_x >= end_x || start_y >= end_y {
        return;
    }

    for image_y in start_y..end_y {
        let src_start = (image_y - tile_origin_y) * tile_width + (start_x - tile_origin_x);
        let dst_start = (image_y - window_y) * window.width as usize + (start_x - window_x);
        let run = end_x - start_x;
        dst[dst_start..dst_start + run].copy_from_slice(&tile_values[src_start..src_start + run]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesce_merges_contiguous_and_overlapping_ranges_only() {
        // Contiguous tiles in file order collapse into one request.
        assert_eq!(coalesce_ranges(&[0..10, 10..20, 20..25]), vec![0..25],);
        // Overlap merges; a gap does not.
        assert_eq!(
            coalesce_ranges(&[0..12, 10..20, 30..40]),
            vec![0..20, 30..40],
        );
        // Unsorted input and empty ranges are handled.
        assert_eq!(
            coalesce_ranges(&[30..40, 5..5, 0..10, 10..20]),
            vec![0..20, 30..40],
        );
        assert_eq!(coalesce_ranges(&[]), Vec::<Range<u64>>::new());
    }

    #[test]
    fn copy_tile_into_window_crops_tile_padding_and_offsets() {
        // 4x4 tile at tile index (1, 0) => image pixels x 4..8, y 0..4.
        // Window x=3..7, y=1..3 (width 4, height 2) overlaps its left half.
        let tile: Vec<u16> = (0..16).collect();
        let window = RasterWindow {
            x: 3,
            y: 1,
            width: 4,
            height: 2,
        };
        let mut dst = vec![u16::MAX; 8];
        copy_tile_into_window(&mut dst, &tile, window, 1, 0, 4, 4);
        // Window columns 3 (not in tile), 4..7 => tile columns 0..3 of rows 1..3.
        assert_eq!(dst, vec![u16::MAX, 4, 5, 6, u16::MAX, 8, 9, 10],);
    }
}
