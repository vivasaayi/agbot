//! Network-free tests for the remote COG backend (feature `remote`).
//!
//! Fixture COGs are built by a hand-rolled tiled-TIFF writer (the `tiff` 0.10
//! encoder only writes striped images) and served from
//! `object_store::memory::InMemory`, which exercises the exact ranged-read
//! path used against HTTP/S3. Fetch-count assertions use the reader's own
//! `RemoteFetchMetrics` instrumentation.
#![cfg(feature = "remote")]

use std::sync::Arc;

use object_store::memory::InMemory;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStoreExt, PutPayload};
use raster_io::{
    write_geotiff_u16, GeoTiffReader, GeoTiffTags, RasterBand, RasterDtype, RasterIoError,
    RasterWindow, RemoteCogReader,
};

// ---------------------------------------------------------------------------
// Tiled GeoTIFF fixture builder
// ---------------------------------------------------------------------------

/// Pixel payload for a fixture, in the dtype under test.
#[derive(Clone)]
enum Pixels {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
}

impl Pixels {
    fn bits_per_sample(&self) -> u16 {
        match self {
            Pixels::U8(_) => 8,
            Pixels::U16(_) => 16,
            Pixels::F32(_) => 32,
        }
    }

    /// TIFF SampleFormat: 1 = unsigned int, 3 = IEEE float.
    fn sample_format(&self) -> u16 {
        match self {
            Pixels::U8(_) | Pixels::U16(_) => 1,
            Pixels::F32(_) => 3,
        }
    }

    /// Little-endian bytes of the pixel at `(x, y)`, or zeroed padding bytes
    /// when the coordinate falls outside the image (TIFF tiles are always
    /// stored at full tile size, padded past the image edge).
    fn le_bytes_at(&self, x: u32, y: u32, width: u32, height: u32, out: &mut Vec<u8>) {
        let inside = x < width && y < height;
        let index = (y * width + x) as usize;
        match self {
            Pixels::U8(values) => {
                out.push(if inside { values[index] } else { 0 });
            }
            Pixels::U16(values) => {
                let value = if inside { values[index] } else { 0 };
                out.extend_from_slice(&value.to_le_bytes());
            }
            Pixels::F32(values) => {
                let value = if inside { values[index] } else { 0.0 };
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
}

/// A TIFF IFD entry value in one of the field types the fixture needs.
enum TagValue {
    Short(Vec<u16>),  // type 3, 2 bytes each
    Long(Vec<u32>),   // type 4, 4 bytes each
    Double(Vec<f64>), // type 12, 8 bytes each
    Ascii(String),    // type 2, 1 byte each incl. terminating NUL
}

impl TagValue {
    fn type_id(&self) -> u16 {
        match self {
            TagValue::Short(_) => 3,
            TagValue::Long(_) => 4,
            TagValue::Double(_) => 12,
            TagValue::Ascii(_) => 2,
        }
    }

    fn count(&self) -> u32 {
        match self {
            TagValue::Short(values) => values.len() as u32,
            TagValue::Long(values) => values.len() as u32,
            TagValue::Double(values) => values.len() as u32,
            TagValue::Ascii(text) => text.len() as u32 + 1, // + NUL
        }
    }

    fn byte_len(&self) -> usize {
        match self {
            TagValue::Short(values) => values.len() * 2,
            TagValue::Long(values) => values.len() * 4,
            TagValue::Double(values) => values.len() * 8,
            TagValue::Ascii(text) => text.len() + 1,
        }
    }

    fn write_le(&self, out: &mut Vec<u8>) {
        match self {
            TagValue::Short(values) => {
                for value in values {
                    out.extend_from_slice(&value.to_le_bytes());
                }
            }
            TagValue::Long(values) => {
                for value in values {
                    out.extend_from_slice(&value.to_le_bytes());
                }
            }
            TagValue::Double(values) => {
                for value in values {
                    out.extend_from_slice(&value.to_le_bytes());
                }
            }
            TagValue::Ascii(text) => {
                out.extend_from_slice(text.as_bytes());
                out.push(0);
            }
        }
    }
}

struct FixtureSpec {
    width: u32,
    height: u32,
    tile_width: u32,
    tile_height: u32,
    pixels: Pixels,
    /// true = DEFLATE (zlib, TIFF compression 8); false = uncompressed (1).
    deflate: bool,
    /// Zero bytes inserted between consecutive tile blobs in the file, to
    /// test that non-contiguous tile ranges are NOT coalesced.
    tile_gap: usize,
}

/// Standard georeferencing for all fixtures: UTM 43N, 10 m pixels anchored
/// at (500000, 4300000), nodata 0.
const FIXTURE_EPSG: u16 = 32643;
const FIXTURE_TRANSFORM: [f64; 6] = [500_000.0, 10.0, 0.0, 4_300_000.0, 0.0, -10.0];

/// Build a classic little-endian tiled GeoTIFF byte stream.
///
/// Layout: 8-byte header | IFD (entry count + 12-byte entries + next-IFD=0)
/// | external value area (values > 4 bytes) | tile blobs (row-major,
/// optionally gap-separated). Every offset is computed up front because the
/// external-area size is known once the entry list is fixed.
fn build_tiled_geotiff(spec: &FixtureSpec) -> Vec<u8> {
    let tiles_across = spec.width.div_ceil(spec.tile_width);
    let tiles_down = spec.height.div_ceil(spec.tile_height);

    // 1. Serialize each tile (full padded tile size), compressing if asked.
    let mut tile_blobs: Vec<Vec<u8>> = Vec::new();
    for tile_y in 0..tiles_down {
        for tile_x in 0..tiles_across {
            let mut raw = Vec::new();
            for row in 0..spec.tile_height {
                for col in 0..spec.tile_width {
                    spec.pixels.le_bytes_at(
                        tile_x * spec.tile_width + col,
                        tile_y * spec.tile_height + row,
                        spec.width,
                        spec.height,
                        &mut raw,
                    );
                }
            }
            tile_blobs.push(if spec.deflate {
                // TIFF compression 8 ("Adobe deflate") is a zlib stream.
                use flate2::write::ZlibEncoder;
                use std::io::Write;
                let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(&raw).expect("zlib write");
                encoder.finish().expect("zlib finish")
            } else {
                raw
            });
        }
    }

    // 2. IFD entries, ascending tag order (required by the TIFF spec).
    // Tile offsets are placeholders until the data base offset is known.
    let tile_byte_counts: Vec<u32> = tile_blobs.iter().map(|blob| blob.len() as u32).collect();
    let mut entries: Vec<(u16, TagValue)> = vec![
        (256, TagValue::Long(vec![spec.width])),  // ImageWidth
        (257, TagValue::Long(vec![spec.height])), // ImageLength
        (258, TagValue::Short(vec![spec.pixels.bits_per_sample()])), // BitsPerSample
        (259, TagValue::Short(vec![if spec.deflate { 8 } else { 1 }])), // Compression
        (262, TagValue::Short(vec![1])),          // Photometric: BlackIsZero
        (277, TagValue::Short(vec![1])),          // SamplesPerPixel
        (322, TagValue::Long(vec![spec.tile_width])), // TileWidth
        (323, TagValue::Long(vec![spec.tile_height])), // TileLength
        (324, TagValue::Long(vec![0; tile_blobs.len()])), // TileOffsets (patched below)
        (325, TagValue::Long(tile_byte_counts.clone())), // TileByteCounts
        (339, TagValue::Short(vec![spec.pixels.sample_format()])), // SampleFormat
        // ModelPixelScaleTag [sx, sy, sz]
        (
            33550,
            TagValue::Double(vec![FIXTURE_TRANSFORM[1], -FIXTURE_TRANSFORM[5], 0.0]),
        ),
        // ModelTiepointTag [i, j, k, x, y, z]: raster origin -> model origin
        (
            33922,
            TagValue::Double(vec![
                0.0,
                0.0,
                0.0,
                FIXTURE_TRANSFORM[0],
                FIXTURE_TRANSFORM[3],
                0.0,
            ]),
        ),
        // GeoKeyDirectory: version 1.1.0, 3 keys — projected model type,
        // pixel-is-area raster type, projected CRS EPSG.
        (
            34735,
            TagValue::Short(vec![
                1,
                1,
                0,
                3,
                1024,
                0,
                1,
                1, // GTModelTypeGeoKey = Projected
                1025,
                0,
                1,
                1, // GTRasterTypeGeoKey = PixelIsArea
                3072,
                0,
                1,
                FIXTURE_EPSG, // ProjectedCSTypeGeoKey
            ]),
        ),
        (42113, TagValue::Ascii("0".to_string())), // GDAL_NODATA
    ];

    // 3. Compute the layout: header | IFD | external values | tile data.
    let ifd_offset: u32 = 8;
    let ifd_len = 2 + entries.len() * 12 + 4;
    let ext_base = ifd_offset as usize + ifd_len;
    let ext_len: usize = entries
        .iter()
        .map(|(_, value)| {
            if value.byte_len() > 4 {
                value.byte_len()
            } else {
                0
            }
        })
        .sum();
    let tile_base = ext_base + ext_len;

    // Patch real tile offsets now that the data base is known.
    let mut running = tile_base as u32;
    let mut tile_offsets = Vec::with_capacity(tile_blobs.len());
    for blob in &tile_blobs {
        tile_offsets.push(running);
        running += blob.len() as u32 + spec.tile_gap as u32;
    }
    entries[8].1 = TagValue::Long(tile_offsets);

    // 4. Serialize: header.
    let mut out = Vec::new();
    out.extend_from_slice(b"II"); // little-endian
    out.extend_from_slice(&42u16.to_le_bytes()); // classic TIFF magic
    out.extend_from_slice(&ifd_offset.to_le_bytes());

    // IFD: entry count, then 12-byte entries (tag, type, count, value/offset).
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut ext_area = Vec::new();
    for (tag, value) in &entries {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&value.type_id().to_le_bytes());
        out.extend_from_slice(&value.count().to_le_bytes());
        if value.byte_len() <= 4 {
            // Inline value, zero-padded to 4 bytes.
            let mut inline = Vec::new();
            value.write_le(&mut inline);
            inline.resize(4, 0);
            out.extend_from_slice(&inline);
        } else {
            let offset = (ext_base + ext_area.len()) as u32;
            out.extend_from_slice(&offset.to_le_bytes());
            value.write_le(&mut ext_area);
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes()); // next IFD offset: none

    // External value area, then tile data (with optional gaps).
    out.extend_from_slice(&ext_area);
    for blob in &tile_blobs {
        out.extend_from_slice(blob);
        out.extend_from_slice(&vec![0u8; spec.tile_gap]);
    }
    assert_eq!(out.len(), running as usize, "builder offset bookkeeping");
    out
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Sequential u16 pixel values 0, 1, 2, ... — every pixel identifies itself.
fn sequential_u16(width: u32, height: u32) -> Vec<u16> {
    (0..width * height).map(|value| value as u16).collect()
}

fn crop_u16(values: &[u16], width: u32, window: RasterWindow) -> Vec<u16> {
    let mut out = Vec::new();
    for row in 0..window.height {
        let start = ((window.y + row) * width + window.x) as usize;
        out.extend_from_slice(&values[start..start + window.width as usize]);
    }
    out
}

async fn open_fixture(spec: &FixtureSpec) -> RemoteCogReader {
    let bytes = build_tiled_geotiff(spec);
    let store = Arc::new(InMemory::new());
    store
        .put(
            &ObjectPath::from("scenes/fixture.tif"),
            PutPayload::from(bytes),
        )
        .await
        .expect("put fixture");
    RemoteCogReader::open(store, "scenes/fixture.tif")
        .await
        .expect("open fixture COG")
}

fn u16_spec(deflate: bool, tile_gap: usize) -> FixtureSpec {
    FixtureSpec {
        width: 32,
        height: 32,
        tile_width: 16,
        tile_height: 16,
        pixels: Pixels::U16(sequential_u16(32, 32)),
        deflate,
        tile_gap,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn open_parses_metadata_with_a_single_range_request() {
    let reader = open_fixture(&u16_spec(true, 0)).await;

    let info = reader.info();
    assert_eq!((info.width, info.height), (32, 32));
    assert_eq!(info.dtype, RasterDtype::U16);
    assert_eq!(info.epsg, Some(u32::from(FIXTURE_EPSG)));
    assert_eq!(info.geo_transform, Some(FIXTURE_TRANSFORM));
    assert_eq!(info.nodata, Some(0.0));

    let layout = reader.tile_layout();
    assert_eq!(layout.tile_width, 16);
    assert_eq!(layout.tile_height, 16);
    assert_eq!((layout.tiles_across, layout.tiles_down), (2, 2));

    // The 32 KiB metadata readahead covers header + IFD of a small COG in
    // exactly one clamped range request.
    let metrics = reader.fetch_metrics();
    assert_eq!(metrics.range_requests, 1);
    assert!(metrics.bytes_fetched > 0);
}

#[tokio::test]
async fn metadata_and_spatial_ref_match_the_local_reader() {
    let remote = open_fixture(&u16_spec(true, 0)).await;

    // Same raster written through the local (striped) writer + reader.
    let local_path = std::env::temp_dir().join("raster_io_remote_parity.tif");
    write_geotiff_u16(
        &local_path,
        32,
        32,
        &sequential_u16(32, 32),
        &GeoTiffTags {
            epsg: Some(u32::from(FIXTURE_EPSG)),
            geo_transform: Some(FIXTURE_TRANSFORM),
            nodata: Some(0.0),
        },
    )
    .expect("write local parity raster");
    let mut local = GeoTiffReader::open(&local_path).expect("open local parity raster");

    assert_eq!(remote.info(), local.info());
    assert_eq!(
        remote.spatial_ref().expect("remote spatial ref"),
        local.spatial_ref().expect("local spatial ref")
    );
    assert_eq!(
        remote.read_band().await.expect("remote band"),
        local.read_band().expect("local band")
    );
}

#[tokio::test]
async fn full_band_read_matches_known_pixels_with_deflate() {
    let reader = open_fixture(&u16_spec(true, 0)).await;
    let band = reader.read_band().await.expect("read band");
    assert_eq!(band, RasterBand::U16(sequential_u16(32, 32)));
}

#[tokio::test]
async fn single_tile_window_fetches_exactly_one_tile_range() {
    let reader = open_fixture(&u16_spec(true, 0)).await;
    let before = reader.fetch_metrics();

    // Fully inside tile (0, 0).
    let window = RasterWindow {
        x: 2,
        y: 3,
        width: 5,
        height: 4,
    };
    let band = reader.read_window(window).await.expect("window read");
    assert_eq!(
        band,
        RasterBand::U16(crop_u16(&sequential_u16(32, 32), 32, window))
    );

    let after = reader.fetch_metrics();
    assert_eq!(after.range_requests - before.range_requests, 1);
    assert!(after.bytes_fetched > before.bytes_fetched);
}

#[tokio::test]
async fn window_spanning_four_contiguous_tiles_coalesces_into_one_request() {
    let reader = open_fixture(&u16_spec(true, 0)).await;
    let before = reader.fetch_metrics();

    // Crosses both tile boundaries: touches tiles (0,0), (1,0), (0,1), (1,1).
    let window = RasterWindow {
        x: 10,
        y: 12,
        width: 12,
        height: 10,
    };
    let band = reader.read_window(window).await.expect("window read");
    assert_eq!(
        band,
        RasterBand::U16(crop_u16(&sequential_u16(32, 32), 32, window))
    );

    // Fixture tiles are back-to-back in the file, so the four tile ranges
    // coalesce into a single range request.
    let after = reader.fetch_metrics();
    assert_eq!(after.range_requests - before.range_requests, 1);
}

#[tokio::test]
async fn window_spanning_gap_separated_tiles_fetches_each_tile_range() {
    // 16 zero bytes between tile blobs -> ranges are not contiguous.
    let reader = open_fixture(&u16_spec(true, 16)).await;
    let before = reader.fetch_metrics();

    let window = RasterWindow {
        x: 10,
        y: 12,
        width: 12,
        height: 10,
    };
    let band = reader.read_window(window).await.expect("window read");
    assert_eq!(
        band,
        RasterBand::U16(crop_u16(&sequential_u16(32, 32), 32, window))
    );

    let after = reader.fetch_metrics();
    assert_eq!(after.range_requests - before.range_requests, 4);
}

#[tokio::test]
async fn edge_window_crops_partial_tiles() {
    // 40x28 image with 16x16 tiles: right column of tiles is 8 px wide,
    // bottom row of tiles is 12 px tall — both padded in the file.
    let spec = FixtureSpec {
        width: 40,
        height: 28,
        tile_width: 16,
        tile_height: 16,
        pixels: Pixels::U16(sequential_u16(40, 28)),
        deflate: true,
        tile_gap: 0,
    };
    let reader = open_fixture(&spec).await;
    assert_eq!(
        (
            reader.tile_layout().tiles_across,
            reader.tile_layout().tiles_down
        ),
        (3, 2)
    );

    // Bottom-right corner window covering only partial tiles.
    let window = RasterWindow {
        x: 30,
        y: 20,
        width: 10,
        height: 8,
    };
    let band = reader.read_window(window).await.expect("edge window read");
    assert_eq!(
        band,
        RasterBand::U16(crop_u16(&sequential_u16(40, 28), 40, window))
    );

    // Full band also reproduces exactly (padding never leaks through).
    let full = reader.read_band().await.expect("full read");
    assert_eq!(full, RasterBand::U16(sequential_u16(40, 28)));
}

#[tokio::test]
async fn out_of_bounds_window_is_reason_coded() {
    let reader = open_fixture(&u16_spec(true, 0)).await;
    let error = reader
        .read_window(RasterWindow {
            x: 20,
            y: 0,
            width: 16,
            height: 4,
        })
        .await
        .expect_err("window exceeds width");
    assert!(matches!(error, RasterIoError::WindowOutOfBounds { .. }));

    let empty = reader
        .read_window(RasterWindow {
            x: 0,
            y: 0,
            width: 0,
            height: 4,
        })
        .await
        .expect_err("empty window");
    assert!(matches!(empty, RasterIoError::WindowOutOfBounds { .. }));
}

#[tokio::test]
async fn uncompressed_u8_band_reads_correctly() {
    let pixels: Vec<u8> = (0..32u32 * 32).map(|value| (value % 251) as u8).collect();
    let spec = FixtureSpec {
        width: 32,
        height: 32,
        tile_width: 16,
        tile_height: 16,
        pixels: Pixels::U8(pixels.clone()),
        deflate: false,
        tile_gap: 0,
    };
    let reader = open_fixture(&spec).await;
    assert_eq!(reader.info().dtype, RasterDtype::U8);
    assert_eq!(
        reader.read_band().await.expect("u8 band"),
        RasterBand::U8(pixels)
    );
}

#[tokio::test]
async fn deflate_f32_band_reads_correctly() {
    let pixels: Vec<f32> = (0..32u32 * 32)
        .map(|value| value as f32 * 0.25 - 8.0)
        .collect();
    let spec = FixtureSpec {
        width: 32,
        height: 32,
        tile_width: 16,
        tile_height: 16,
        pixels: Pixels::F32(pixels.clone()),
        deflate: true,
        tile_gap: 0,
    };
    let reader = open_fixture(&spec).await;
    assert_eq!(reader.info().dtype, RasterDtype::F32);
    assert_eq!(
        reader.read_band().await.expect("f32 band"),
        RasterBand::F32(pixels)
    );
}

#[tokio::test]
async fn striped_tiff_is_rejected_as_not_tiled() {
    // A striped GeoTIFF from the local writer must be refused by the COG
    // reader with a reason-coded error, not a decode panic.
    let local_path = std::env::temp_dir().join("raster_io_remote_striped.tif");
    write_geotiff_u16(
        &local_path,
        8,
        8,
        &sequential_u16(8, 8),
        &GeoTiffTags {
            epsg: Some(u32::from(FIXTURE_EPSG)),
            geo_transform: Some(FIXTURE_TRANSFORM),
            nodata: None,
        },
    )
    .expect("write striped tiff");
    let bytes = std::fs::read(&local_path).expect("read striped tiff");

    let store = Arc::new(InMemory::new());
    store
        .put(&ObjectPath::from("striped.tif"), PutPayload::from(bytes))
        .await
        .expect("put striped tiff");
    let error = RemoteCogReader::open(store, "striped.tif")
        .await
        .expect_err("striped tiff must be rejected");
    assert!(matches!(error, RasterIoError::NotTiled { .. }));
}

/// Manual network smoke test against the public Sentinel-2 COG archive.
///
/// Run with:
/// `cargo test -p raster_io --features remote -- --ignored sentinel_cogs`
/// The URL points at a Sentinel-2 L2A red band (B04, u16, DEFLATE, 10 m);
/// substitute any valid scene path from an Earth Search STAC result if this
/// one ever disappears.
#[tokio::test]
#[ignore = "network: hits sentinel-cogs.s3.us-west-2.amazonaws.com"]
async fn sentinel_cogs_manual_smoke() {
    let url = "https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/43/P/GQ/2023/1/S2A_43PGQ_20230106_0_L2A/B04.tif";
    let reader = RemoteCogReader::from_url(url)
        .await
        .expect("open remote COG");

    let info = reader.info();
    assert_eq!(info.dtype, RasterDtype::U16);
    assert!(
        info.width >= 10_000 && info.height >= 10_000,
        "10 m S2 band"
    );
    assert!(info.epsg.is_some(), "S2 COGs carry a UTM EPSG code");
    assert!(info.geo_transform.is_some());

    let window = RasterWindow {
        x: info.width / 2,
        y: info.height / 2,
        width: 256,
        height: 256,
    };
    let band = reader.read_window(window).await.expect("window read");
    assert_eq!(band.len(), 256 * 256);

    let metrics = reader.fetch_metrics();
    println!(
        "sentinel-cogs smoke: {} range requests, {} bytes fetched",
        metrics.range_requests, metrics.bytes_fetched
    );
    // Whole-file download would be tens of MB; ranged reads stay small.
    assert!(metrics.bytes_fetched < 8 * 1024 * 1024);
}
