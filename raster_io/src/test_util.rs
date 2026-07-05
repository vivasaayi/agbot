//! Deterministic tiled-GeoTIFF fixture builder (feature `test-util`).
//!
//! Promoted from `tests/remote_cog.rs` (batch 6) so downstream crates
//! (geo_hub's satellite-derivation tests) can build in-memory COG fixtures
//! and serve them from `object_store::memory::InMemory` without touching the
//! network. The `tiff` 0.10 encoder only writes striped images, so tiled
//! fixtures are hand-serialized here: classic little-endian TIFF, one IFD,
//! optional DEFLATE (zlib) tile compression, GeoTIFF georeferencing tags.

/// Pixel payload for a fixture, in the dtype under test.
#[derive(Clone)]
pub enum Pixels {
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

/// Specification of one tiled GeoTIFF fixture.
pub struct FixtureSpec {
    pub width: u32,
    pub height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    pub pixels: Pixels,
    /// true = DEFLATE (zlib, TIFF compression 8); false = uncompressed (1).
    pub deflate: bool,
    /// Zero bytes inserted between consecutive tile blobs in the file, to
    /// test that non-contiguous tile ranges are NOT coalesced.
    pub tile_gap: usize,
    /// Projected CRS EPSG code written to the GeoKey directory.
    pub epsg: u16,
    /// GDAL-order geotransform; must be north-up (no rotation terms).
    pub geo_transform: [f64; 6],
    /// `GDAL_NODATA` ASCII tag payload, e.g. `"0"`.
    pub nodata: Option<String>,
}

impl FixtureSpec {
    /// The historical fixture defaults used by the raster_io remote tests:
    /// UTM 43N (EPSG:32643), 10 m pixels anchored at (500000, 4300000),
    /// nodata 0.
    pub fn with_default_georef(
        width: u32,
        height: u32,
        tile_width: u32,
        tile_height: u32,
        pixels: Pixels,
        deflate: bool,
        tile_gap: usize,
    ) -> Self {
        Self {
            width,
            height,
            tile_width,
            tile_height,
            pixels,
            deflate,
            tile_gap,
            epsg: 32643,
            geo_transform: [500_000.0, 10.0, 0.0, 4_300_000.0, 0.0, -10.0],
            nodata: Some("0".to_string()),
        }
    }
}

/// Build a classic little-endian tiled GeoTIFF byte stream.
///
/// Layout: 8-byte header | IFD (entry count + 12-byte entries + next-IFD=0)
/// | external value area (values > 4 bytes) | tile blobs (row-major,
/// optionally gap-separated). Every offset is computed up front because the
/// external-area size is known once the entry list is fixed.
pub fn build_tiled_geotiff(spec: &FixtureSpec) -> Vec<u8> {
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
            TagValue::Double(vec![spec.geo_transform[1], -spec.geo_transform[5], 0.0]),
        ),
        // ModelTiepointTag [i, j, k, x, y, z]: raster origin -> model origin
        (
            33922,
            TagValue::Double(vec![
                0.0,
                0.0,
                0.0,
                spec.geo_transform[0],
                spec.geo_transform[3],
                0.0,
            ]),
        ),
        // GeoKeyDirectory: version 1.1.0, 3 keys — projected model type,
        // pixel-is-area raster type, projected CRS EPSG.
        (
            34735,
            TagValue::Short(vec![
                1, 1, 0, 3, 1024, 0, 1, 1, // GTModelTypeGeoKey = Projected
                1025, 0, 1, 1, // GTRasterTypeGeoKey = PixelIsArea
                3072, 0, 1, spec.epsg, // ProjectedCSTypeGeoKey
            ]),
        ),
    ];
    if let Some(nodata) = &spec.nodata {
        entries.push((42113, TagValue::Ascii(nodata.clone()))); // GDAL_NODATA
    }

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
// JP2 fixture encoder (satellite pipeline batch 24)
// ---------------------------------------------------------------------------

/// Write a single-component unsigned 16-bit lossless J2K codestream, encoded
/// by the reference OpenJPEG encoder (the same pure-Rust `openjp2` port the
/// production decode path uses), so fixtures are conformant codestreams —
/// exactly what Sentinel-2 band files contain.
///
/// Panics on encoder failure: this is a test fixture builder, not an API.
pub fn write_jp2_gray(path: &std::path::Path, width: u32, height: u32, values: &[u16]) {
    use openjp2::image::{opj_image_cmptparm_t, opj_image_create, opj_image_destroy};
    use openjp2::openjpeg::*;
    use std::ffi::CString;

    assert_eq!(
        values.len(),
        width as usize * height as usize,
        "values must be width*height"
    );
    assert!(
        width.min(height) >= 4,
        "encoder fixture needs at least 4x4 for 2 decomposition levels"
    );

    unsafe {
        let mut params: opj_cparameters_t = std::mem::zeroed();
        opj_set_default_encoder_parameters(&mut params);
        params.numresolution = 3;
        params.tcp_numlayers = 1;
        params.tcp_rates[0] = 0.0; // lossless
        params.cp_disto_alloc = 1;
        assert_eq!(params.irreversible, 0, "default must be reversible 5/3");

        let mut cmpt: opj_image_cmptparm_t = std::mem::zeroed();
        cmpt.dx = 1;
        cmpt.dy = 1;
        cmpt.w = width;
        cmpt.h = height;
        cmpt.prec = 16;
        cmpt.bpp = 16;
        cmpt.sgnd = 0;
        let image = opj_image_create(1, &mut cmpt, OPJ_CLRSPC_GRAY);
        assert!(!image.is_null(), "opj_image_create");
        (*image).x0 = 0;
        (*image).y0 = 0;
        (*image).x1 = width;
        (*image).y1 = height;
        let comps = (*image).comps_mut().expect("image components");
        let data = comps[0].data_mut().expect("component buffer");
        for (dst, src) in data.iter_mut().zip(values) {
            *dst = i32::from(*src);
        }

        let codec = opj_create_compress(OPJ_CODEC_J2K);
        assert!(!codec.is_null(), "opj_create_compress");
        assert_eq!(opj_setup_encoder(codec, &mut params, image), 1, "setup");
        let fname = CString::new(path.to_str().expect("utf-8 fixture path")).expect("no NUL");
        let stream = opj_stream_create_default_file_stream(fname.as_ptr(), 0);
        assert!(!stream.is_null(), "output stream");
        assert_eq!(
            opj_start_compress(codec, image, stream),
            1,
            "start_compress"
        );
        assert_eq!(opj_encode(codec, stream), 1, "encode");
        assert_eq!(opj_end_compress(codec, stream), 1, "end_compress");
        opj_stream_destroy(stream);
        opj_destroy_codec(codec);
        opj_image_destroy(image);
    }
}
