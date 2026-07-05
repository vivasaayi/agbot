//! Lossless JP2 round-trip (satellite pipeline batch 24): fixtures are
//! encoded by the reference OpenJPEG encoder (`test_util::write_jp2_gray`,
//! the same pure-Rust `openjp2` port the decode path links), so the decode
//! wrapper is exercised against conformant codestreams — the format
//! Sentinel-2 band files actually contain.
//!
//! Requires the fixture feature: `cargo test -p raster_io --features test-util`.
#![cfg(feature = "test-util")]

use raster_io::{read_jp2_gray, test_util::write_jp2_gray, RasterIoError};

#[test]
fn lossless_roundtrip_recovers_exact_dn() {
    // Sentinel-like DN values including the 0 fill and a full-range peak.
    let values: Vec<u16> = (0..64)
        .map(|i| match i {
            0 => 0,
            63 => 65535,
            i => (i * 500) as u16,
        })
        .collect();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let path = tmp.path().join("band.j2k");
    write_jp2_gray(&path, 8, 8, &values);

    let decoded = read_jp2_gray(&path).expect("decodes");
    assert_eq!((decoded.width, decoded.height), (8, 8));
    assert_eq!(
        decoded.values, values,
        "lossless 5/3 must round-trip DN exactly"
    );
}

#[test]
fn constant_band_roundtrips() {
    // Degenerate flat rasters (a common fixture shape) must survive too.
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let path = tmp.path().join("flat.j2k");
    write_jp2_gray(&path, 4, 4, &[3000u16; 16]);
    let decoded = read_jp2_gray(&path).expect("decodes");
    assert_eq!(decoded.values, vec![3000u16; 16]);
}

#[test]
fn truncated_codestream_is_a_decode_error() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let path = tmp.path().join("band.j2k");
    write_jp2_gray(&path, 8, 8, &[1000u16; 64]);
    let bytes = std::fs::read(&path).expect("read");
    let cut = tmp.path().join("truncated.j2k");
    std::fs::write(&cut, &bytes[..bytes.len() / 2]).expect("write");
    assert!(matches!(
        read_jp2_gray(&cut),
        Err(RasterIoError::Decode { .. })
    ));
}
