//! Network-free tests for the remote COG backend (feature `remote`).
//!
//! Fixture COGs are built by `raster_io::test_util::build_tiled_geotiff`
//! (the `tiff` 0.10 encoder only writes striped images) and served from
//! `object_store::memory::InMemory`, which exercises the exact ranged-read
//! path used against HTTP/S3. Fetch-count assertions use the reader's own
//! `RemoteFetchMetrics` instrumentation.
//!
//! Requires both features: `cargo test -p raster_io --features remote,test-util`.
#![cfg(all(feature = "remote", feature = "test-util"))]

use std::sync::Arc;

use object_store::memory::InMemory;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStoreExt, PutPayload};
use raster_io::test_util::{build_tiled_geotiff, FixtureSpec, Pixels};
use raster_io::{
    write_geotiff_u16, GeoTiffReader, GeoTiffTags, RasterBand, RasterDtype, RasterIoError,
    RasterWindow, RemoteCogReader,
};

/// Standard georeferencing for all fixtures: UTM 43N, 10 m pixels anchored
/// at (500000, 4300000), nodata 0.
const FIXTURE_EPSG: u16 = 32643;
const FIXTURE_TRANSFORM: [f64; 6] = [500_000.0, 10.0, 0.0, 4_300_000.0, 0.0, -10.0];

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
    FixtureSpec::with_default_georef(
        32,
        32,
        16,
        16,
        Pixels::U16(sequential_u16(32, 32)),
        deflate,
        tile_gap,
    )
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
    let spec = FixtureSpec::with_default_georef(
        40,
        28,
        16,
        16,
        Pixels::U16(sequential_u16(40, 28)),
        true,
        0,
    );
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
    let spec =
        FixtureSpec::with_default_georef(32, 32, 16, 16, Pixels::U8(pixels.clone()), false, 0);
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
    let spec =
        FixtureSpec::with_default_georef(32, 32, 16, 16, Pixels::F32(pixels.clone()), true, 0);
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
