#include "agbot_config/Toml.hpp"
#include "agbot_terrain/ElevationEstimator.hpp"
#include "agbot_terrain/Fusion.hpp"
#include "agbot_terrain/MonoDepth.hpp"
#include "agbot_terrain/Png.hpp"
#include "agbot_terrain/Raster.hpp"
#include "agbot_terrain/TerrainPipeline.hpp"
#include "agbot_terrain/Validation.hpp"

#include <algorithm>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <limits>
#include <sstream>
#include <string>
#include <vector>

namespace {

namespace terrain = agbot::terrain;
namespace cfg = agbot::config;
namespace fsim = agbot::flight_sim;

int failures = 0;

void expect(bool condition, const std::string& label) {
    if (condition) {
        std::cout << "PASS " << label << "\n";
    } else {
        std::cout << "FAIL " << label << "\n";
        ++failures;
    }
}

terrain::GeoBounds test_bounds() {
    terrain::GeoBounds bounds;
    bounds.min_latitude = 40.70;
    bounds.min_longitude = -74.02;
    bounds.max_latitude = 40.82;
    bounds.max_longitude = -73.93;
    return bounds;
}

// Analytic surface: a smooth tilted plane plus a broad sine bump.
float analytic_elevation(const terrain::GeoBounds& bounds, double latitude, double longitude) {
    const double u = (longitude - bounds.min_longitude) /
        (bounds.max_longitude - bounds.min_longitude);
    const double v = (latitude - bounds.min_latitude) /
        (bounds.max_latitude - bounds.min_latitude);
    return static_cast<float>(
        50.0 + 120.0 * u + 80.0 * v + 15.0 * std::sin(u * 3.14159) * std::sin(v * 3.14159));
}

terrain::Raster analytic_raster(const terrain::GeoBounds& bounds, int resolution) {
    terrain::Raster raster =
        terrain::Raster::filled(resolution, resolution, bounds, 0.0f);
    for (int row = 0; row < resolution; ++row) {
        for (int col = 0; col < resolution; ++col) {
            const double v = static_cast<double>(row) / static_cast<double>(resolution - 1);
            const double u = static_cast<double>(col) / static_cast<double>(resolution - 1);
            const double latitude = bounds.max_latitude - v *
                (bounds.max_latitude - bounds.min_latitude);
            const double longitude = bounds.min_longitude + u *
                (bounds.max_longitude - bounds.min_longitude);
            raster.set(row, col, analytic_elevation(bounds, latitude, longitude));
        }
    }
    return raster;
}

void test_raster_sampling() {
    terrain::GeoBounds bounds = test_bounds();
    terrain::Raster raster = terrain::Raster::filled(2, 2, bounds, 0.0f);
    raster.set(0, 0, 10.0f); // north-west
    raster.set(0, 1, 20.0f); // north-east
    raster.set(1, 0, 30.0f); // south-west
    raster.set(1, 1, 40.0f); // south-east

    const auto center = raster.sample_at(
        (bounds.min_latitude + bounds.max_latitude) / 2.0,
        (bounds.min_longitude + bounds.max_longitude) / 2.0);
    expect(center.has_value() && std::abs(*center - 25.0f) < 1e-4f, "raster bilinear center");

    const auto nw = raster.sample_at(bounds.max_latitude, bounds.min_longitude);
    expect(nw.has_value() && std::abs(*nw - 10.0f) < 1e-4f, "raster north-west corner");

    const auto outside = raster.sample_at(bounds.max_latitude + 1.0, bounds.min_longitude);
    expect(!outside.has_value(), "raster sample outside bounds is nullopt");

    raster.set(0, 0, terrain::Raster::nodata());
    const auto near_nodata = raster.sample_at(
        (bounds.min_latitude + bounds.max_latitude) / 2.0,
        (bounds.min_longitude + bounds.max_longitude) / 2.0);
    expect(near_nodata.has_value() && std::abs(*near_nodata - 30.0f) < 1e-4f,
           "raster nodata excluded from bilinear weights");
}

void test_registry() {
    const auto& registry = terrain::estimator_registry();
    expect(registry.contains("dem_fusion"), "registry has dem_fusion");
    expect(registry.contains("synthetic_detail"), "registry has synthetic_detail");
    expect(registry.contains("mono_depth_onnx"), "registry has mono_depth_onnx");

    terrain::ImageryBundle bundle;
    bundle.aoi = test_bounds();
    const auto mono = registry.create("mono_depth_onnx");
    expect(mono != nullptr && !mono->accepts(bundle),
           "mono_depth_onnx rejects bundles without RGB imagery");
#if !defined(AGBOT_TERRAIN_HAS_ONNX)
    if (mono != nullptr) {
        const auto result = mono->estimate(bundle, {});
        expect(!result.ok && result.error == "onnx_runtime_unavailable",
               "onnx stub reports reason-coded error");
    }
#endif
}

void test_dem_prior_roundtrip() {
    terrain::ImageryBundle bundle;
    bundle.aoi = test_bounds();
    bundle.grid_width = 48;
    bundle.grid_height = 48;
    terrain::HeightField prior;
    prior.elevation = analytic_raster(bundle.aoi, 96);
    prior.confidence = terrain::Raster::filled(96, 96, bundle.aoi, 1.0f);
    prior.source_algorithm = "analytic_fixture";
    bundle.dem_prior = prior;

    const auto estimator = terrain::estimator_registry().create("dem_fusion");
    cfg::ParamTable params;
    params["source"] = cfg::ParamValue("prior");
    params["resample"] = cfg::ParamValue("bilinear");
    const auto result = estimator->estimate(bundle, params);
    expect(result.ok, "dem_fusion prior estimate succeeds");
    if (!result.ok) {
        std::cout << "  error: " << result.error << "\n";
        return;
    }
    expect(result.field.elevation.width == 48 && result.field.elevation.height == 48,
           "dem_fusion prior produces requested grid");

    float max_error = 0.0f;
    for (int row = 0; row < 48; ++row) {
        for (int col = 0; col < 48; ++col) {
            const double v = static_cast<double>(row) / 47.0;
            const double u = static_cast<double>(col) / 47.0;
            const double latitude = bundle.aoi.max_latitude - v *
                (bundle.aoi.max_latitude - bundle.aoi.min_latitude);
            const double longitude = bundle.aoi.min_longitude + u *
                (bundle.aoi.max_longitude - bundle.aoi.min_longitude);
            const float expected = analytic_elevation(bundle.aoi, latitude, longitude);
            max_error = std::max(max_error,
                std::abs(result.field.elevation.at(row, col) - expected));
        }
    }
    expect(max_error < 0.5f, "dem_fusion prior roundtrip within 0.5 m of analytic surface");
}

void test_dem_clamp() {
    terrain::ImageryBundle bundle;
    bundle.aoi = test_bounds();
    bundle.grid_width = 16;
    bundle.grid_height = 16;
    terrain::HeightField prior;
    prior.elevation = terrain::Raster::filled(16, 16, bundle.aoi, 20.0f);
    prior.elevation.set(3, 3, -700.0f);  // coastal bathymetry artifact
    prior.elevation.set(4, 4, 900.0f);   // spike artifact
    prior.confidence = terrain::Raster::filled(16, 16, bundle.aoi, 1.0f);
    prior.source_algorithm = "analytic_fixture";
    bundle.dem_prior = prior;

    const auto estimator = terrain::estimator_registry().create("dem_fusion");
    cfg::ParamTable params;
    params["source"] = cfg::ParamValue("prior");
    params["resample"] = cfg::ParamValue("nearest");
    params["clamp_min_m"] = cfg::ParamValue(-5.0);
    params["clamp_max_m"] = cfg::ParamValue(500.0);
    const auto result = estimator->estimate(bundle, params);
    expect(result.ok, "dem_fusion clamp estimate succeeds");
    if (!result.ok) {
        return;
    }
    float min_value = std::numeric_limits<float>::max();
    float max_value = std::numeric_limits<float>::lowest();
    for (const float value : result.field.elevation.values) {
        if (!terrain::Raster::is_nodata(value)) {
            min_value = std::min(min_value, value);
            max_value = std::max(max_value, value);
        }
    }
    expect(min_value >= -5.0f, "clamp_min_m floors bathymetry");
    expect(max_value <= 500.0f, "clamp_max_m caps spikes");

    // Without clamp params the extremes pass through untouched.
    cfg::ParamTable no_clamp;
    no_clamp["source"] = cfg::ParamValue("prior");
    no_clamp["resample"] = cfg::ParamValue("nearest");
    const auto raw = estimator->estimate(bundle, no_clamp);
    float raw_min = std::numeric_limits<float>::max();
    for (const float value : raw.field.elevation.values) {
        if (!terrain::Raster::is_nodata(value)) {
            raw_min = std::min(raw_min, value);
        }
    }
    expect(raw.ok && raw_min < -600.0f, "clamp off by default");
}

void test_synthetic_detail_determinism() {
    terrain::ImageryBundle bundle;
    bundle.aoi = test_bounds();
    bundle.grid_width = 32;
    bundle.grid_height = 32;
    const auto estimator = terrain::estimator_registry().create("synthetic_detail");
    cfg::ParamTable params;
    params["amplitude_m"] = cfg::ParamValue(3.0);
    params["seed"] = cfg::ParamValue(42);
    const auto first = estimator->estimate(bundle, params);
    const auto second = estimator->estimate(bundle, params);
    expect(first.ok && second.ok, "synthetic_detail estimates succeed");
    expect(terrain::raster_hash(first.field.elevation) ==
           terrain::raster_hash(second.field.elevation),
           "synthetic_detail deterministic for same seed");

    params["seed"] = cfg::ParamValue(43);
    const auto different = estimator->estimate(bundle, params);
    expect(different.ok && terrain::raster_hash(different.field.elevation) !=
           terrain::raster_hash(first.field.elevation),
           "synthetic_detail differs for different seed");

    float min_value = 1e9f;
    float max_value = -1e9f;
    for (const float value : first.field.elevation.values) {
        min_value = std::min(min_value, value);
        max_value = std::max(max_value, value);
    }
    expect(min_value >= -3.0f && max_value <= 3.0f && (max_value - min_value) > 0.5f,
           "synthetic_detail amplitude bounded and non-flat");
    expect(std::abs(first.field.confidence.values[0] - 0.3f) < 1e-6f,
           "synthetic_detail default confidence 0.3");
}

terrain::FusionLayer constant_layer(
    const terrain::GeoBounds& bounds, int resolution, float value, float confidence, double weight) {
    terrain::FusionLayer layer;
    layer.field.elevation = terrain::Raster::filled(resolution, resolution, bounds, value);
    layer.field.confidence = terrain::Raster::filled(resolution, resolution, bounds, confidence);
    layer.field.source_algorithm = "constant";
    layer.weight = weight;
    return layer;
}

void test_fusion_dem_locked() {
    const terrain::GeoBounds bounds = test_bounds();
    terrain::FusionLayer base = constant_layer(bounds, 8, 100.0f, 1.0f, 1.0);
    base.field.elevation.set(3, 3, terrain::Raster::nodata());
    const terrain::FusionLayer fill = constant_layer(bounds, 8, 42.0f, 0.4f, 1.0);
    const auto fused = terrain::FusionEngine::dem_locked({ base, fill });
    expect(fused.ok, "dem_locked fuses");
    expect(std::abs(fused.field.elevation.at(0, 0) - 100.0f) < 1e-6f,
           "dem_locked keeps base where present");
    expect(std::abs(fused.field.elevation.at(3, 3) - 42.0f) < 1e-6f,
           "dem_locked fills base voids from later layers");
}

void test_fusion_confidence_weighted() {
    const terrain::GeoBounds bounds = test_bounds();
    const terrain::FusionLayer a = constant_layer(bounds, 4, 10.0f, 1.0f, 1.0);
    const terrain::FusionLayer b = constant_layer(bounds, 4, 20.0f, 0.5f, 2.0);
    const auto fused = terrain::FusionEngine::confidence_weighted({ a, b });
    expect(fused.ok, "confidence_weighted fuses");
    // Weights: 1.0*1.0 = 1 and 2.0*0.5 = 1 -> mean of 10 and 20 = 15.
    expect(std::abs(fused.field.elevation.at(2, 2) - 15.0f) < 1e-4f,
           "confidence_weighted hand-computed value");
}

void test_fusion_detail_injection() {
    const terrain::GeoBounds bounds = test_bounds();
    const int resolution = 32;

    // Base: linear ramp (invariant under symmetric box blur away from edges).
    terrain::FusionLayer base = constant_layer(bounds, resolution, 0.0f, 1.0f, 1.0);
    for (int row = 0; row < resolution; ++row) {
        for (int col = 0; col < resolution; ++col) {
            base.field.elevation.set(row, col, static_cast<float>(col) * 2.0f);
        }
    }
    // Detail: constant offset -> highpass is exactly zero, must not leak in.
    const terrain::FusionLayer flat_detail = constant_layer(bounds, resolution, 500.0f, 0.3f, 1.0);
    const auto fused = terrain::FusionEngine::detail_injection({ base, flat_detail }, 0.8, 2);
    expect(fused.ok, "detail_injection fuses");
    if (fused.ok) {
        float max_interior_error = 0.0f;
        for (int row = 8; row < resolution - 8; ++row) {
            for (int col = 8; col < resolution - 8; ++col) {
                max_interior_error = std::max(max_interior_error,
                    std::abs(fused.field.elevation.at(row, col) -
                             base.field.elevation.at(row, col)));
            }
        }
        expect(max_interior_error < 1e-3f,
               "detail_injection preserves base low frequencies (flat detail adds nothing)");
    }

    // Alternating detail: injected energy must scale with lambda.
    terrain::FusionLayer bumpy = constant_layer(bounds, resolution, 0.0f, 0.3f, 1.0);
    for (int row = 0; row < resolution; ++row) {
        for (int col = 0; col < resolution; ++col) {
            bumpy.field.elevation.set(row, col, ((row + col) % 2 == 0) ? 1.0f : -1.0f);
        }
    }
    const auto injected = terrain::FusionEngine::detail_injection({ base, bumpy }, 0.5, 2);
    expect(injected.ok, "detail_injection with bumpy detail fuses");
    if (injected.ok) {
        const int row = resolution / 2;
        const int col = resolution / 2;
        const float delta =
            injected.field.elevation.at(row, col) - fused.field.elevation.at(row, col);
        const float expected = 0.5f * bumpy.field.elevation.at(row, col);
        expect(std::abs(delta - expected) < 0.05f,
               "detail_injection injects lambda-scaled highpass detail");
    }
}

void test_validation_metrics() {
    const terrain::GeoBounds bounds = test_bounds();
    terrain::Raster estimate = terrain::Raster::filled(2, 2, bounds, 0.0f);
    terrain::Raster reference = terrain::Raster::filled(2, 2, bounds, 0.0f);
    // Errors: +1, -1, +2, 0.
    estimate.values = { 11.0f, 9.0f, 12.0f, 10.0f };
    reference.values = { 10.0f, 10.0f, 10.0f, 10.0f };
    const auto metrics = terrain::compute_metrics(estimate, reference);
    expect(metrics.sample_count == 4, "metrics sample count");
    expect(std::abs(metrics.rmse - std::sqrt(6.0 / 4.0)) < 1e-9, "metrics rmse");
    expect(std::abs(metrics.mae - 1.0) < 1e-9, "metrics mae");
    expect(std::abs(metrics.bias - 0.5) < 1e-9, "metrics bias");
    expect(std::abs(metrics.max_abs - 2.0) < 1e-9, "metrics max_abs");
    expect(std::abs(metrics.pct_within_1m - 75.0) < 1e-9, "metrics pct_within_1m");
    expect(std::abs(metrics.pct_within_5m - 100.0) < 1e-9, "metrics pct_within_5m");
    // Sorted errors [-1, 0, 1, 2] -> median 0.5; |e - 0.5| = [1.5, .5, .5, 1.5]
    // -> median 1.0 -> nmad = 1.4826.
    expect(std::abs(metrics.nmad - 1.4826) < 1e-9, "metrics nmad");

    estimate.values[1] = terrain::Raster::nodata();
    const auto partial = terrain::compute_metrics(estimate, reference);
    expect(partial.sample_count == 3, "metrics skip nodata cells");
}

void test_validation_json() {
    terrain::ValidationReport report;
    report.ok = true;
    report.reference_name = "dem_fusion";
    report.metrics.sample_count = 4;
    report.metrics.rmse = 1.224745;
    report.metrics.pct_within_1m = 75.0;
    report.param_hash = 0x1234ABCDULL;
    report.fused_raster_hash = 0xFEEDBEEFULL;

    const std::filesystem::path path =
        std::filesystem::path("out") / "terrain" / "validation.json";
    std::string error;
    expect(terrain::write_validation_json(path, report, &error), "validation json written");
    std::ifstream stream(path);
    std::stringstream first;
    first << stream.rdbuf();
    expect(first.str().find("\"rmse_m\": 1.224745") != std::string::npos,
           "validation json contains rmse");
    expect(first.str().find("\"param_hash\": \"000000001234abcd\"") != std::string::npos,
           "validation json fixed-width hex param hash");

    expect(terrain::write_validation_json(path, report, &error), "validation json rewritten");
    std::ifstream stream2(path);
    std::stringstream second;
    second << stream2.rdbuf();
    expect(first.str() == second.str() && !first.str().empty(),
           "validation json byte-identical across runs");
}

void test_inflate_stored_and_png_synthetic() {
    // Hand-built zlib stream with one stored block: bytes 1..4.
    const std::vector<std::uint8_t> payload = { 1, 2, 3, 4 };
    std::vector<std::uint8_t> stream = { 0x78, 0x01 }; // CMF/FLG (78 01 % 31 == 0)
    stream.push_back(0x01); // BFINAL=1, BTYPE=00
    stream.push_back(0x04); stream.push_back(0x00); // LEN
    stream.push_back(0xFB); stream.push_back(0xFF); // NLEN
    stream.insert(stream.end(), payload.begin(), payload.end());
    std::uint32_t s1 = 1;
    std::uint32_t s2 = 0;
    for (const std::uint8_t byte : payload) {
        s1 = (s1 + byte) % 65521u;
        s2 = (s2 + s1) % 65521u;
    }
    const std::uint32_t adler = (s2 << 16) | s1;
    stream.push_back(static_cast<std::uint8_t>(adler >> 24));
    stream.push_back(static_cast<std::uint8_t>(adler >> 16));
    stream.push_back(static_cast<std::uint8_t>(adler >> 8));
    stream.push_back(static_cast<std::uint8_t>(adler));

    const auto inflated = terrain::inflate_zlib_stream(stream.data(), stream.size());
    expect(inflated.ok && inflated.bytes == payload, "zlib stored-block inflate roundtrip");

    std::vector<std::uint8_t> corrupted = stream;
    corrupted.back() ^= 0xFF;
    const auto bad = terrain::inflate_zlib_stream(corrupted.data(), corrupted.size());
    expect(!bad.ok && bad.error == "zlib_adler32_mismatch", "adler mismatch detected");
}

void test_real_tile_decode() {
    const std::filesystem::path tiles_dir =
        std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out" / "elevation_tiles";
    std::filesystem::path tile_path;
    if (std::filesystem::exists(tiles_dir)) {
        for (const auto& entry : std::filesystem::recursive_directory_iterator(tiles_dir)) {
            if (entry.is_regular_file() && entry.path().extension() == ".png") {
                tile_path = entry.path();
                break;
            }
        }
    }
    if (tile_path.empty()) {
        std::cout << "SKIP real tile decode (no cached tiles under "
                  << tiles_dir.string() << ")\n";
        return;
    }
    const terrain::PngImage png = terrain::decode_png_rgba_file(tile_path);
    expect(png.ok, "real cached tile PNG decodes (" + tile_path.string() + ")");
    if (!png.ok) {
        std::cout << "  error: " << png.error << "\n";
        return;
    }
    expect(png.width == 256 && png.height == 256, "real tile is 256x256");

    // Parse z/x/y from the path tail to build the tile coordinate.
    const int y = std::stoi(tile_path.stem().string());
    const int x = std::stoi(tile_path.parent_path().filename().string());
    const int z = std::stoi(tile_path.parent_path().parent_path().filename().string());
    const auto tile = fsim::elevation_tile_from_terrarium_rgba(
        fsim::TileCoordinate { z, x, y }, png.width, png.height, png.rgba);
    expect(tile.has_value(), "real tile converts to elevation tile");
    if (tile.has_value()) {
        expect(tile->min_elevation_m > -500.0f && tile->max_elevation_m < 9000.0f &&
               tile->max_elevation_m >= tile->min_elevation_m,
               "real tile elevation range sane (" +
               std::to_string(tile->min_elevation_m) + ".." +
               std::to_string(tile->max_elevation_m) + " m)");
    }
}

void test_affine_fit() {
    // Known line y = 2.5x - 40 with three gross outliers; the sigma-clipped
    // fit must recover the exact coefficients and reject the outliers.
    std::vector<float> x;
    std::vector<float> y;
    for (int i = 0; i < 100; ++i) {
        const float xv = static_cast<float>(i) * 0.1f;
        x.push_back(xv);
        y.push_back(2.5f * xv - 40.0f);
    }
    y[10] += 500.0f;
    y[50] -= 300.0f;
    y[80] += 250.0f;
    const auto fit = terrain::fit_affine_sigma_clipped(x, y);
    expect(fit.ok, "affine fit succeeds");
    expect(std::abs(fit.a - 2.5) < 1e-3, "affine fit recovers slope a=2.5");
    expect(std::abs(fit.b + 40.0) < 1e-2, "affine fit recovers intercept b=-40");
    expect(fit.inliers == 97, "affine fit rejects the 3 outliers");
    expect(fit.sigma < 1e-3, "affine fit inlier sigma near zero on exact line");

    const auto empty = terrain::fit_affine_sigma_clipped({}, {});
    expect(!empty.ok, "affine fit rejects empty input");
    const auto degenerate = terrain::fit_affine_sigma_clipped(
        { 1.0f, 1.0f, 1.0f }, { 2.0f, 3.0f, 4.0f });
    expect(!degenerate.ok, "affine fit rejects constant x (degenerate)");
    const auto mismatched = terrain::fit_affine_sigma_clipped({ 1.0f }, { 1.0f, 2.0f });
    expect(!mismatched.ok, "affine fit rejects mismatched sizes");
}

#if defined(AGBOT_TERRAIN_HAS_ONNX)

// --- Minimal PNG writer (RGB8, filter 0, stored-deflate) for test imagery ---

std::uint32_t crc32_bytes(const std::uint8_t* data, std::size_t size, std::uint32_t crc) {
    crc = ~crc;
    for (std::size_t i = 0; i < size; ++i) {
        crc ^= data[i];
        for (int bit = 0; bit < 8; ++bit) {
            crc = (crc >> 1) ^ (0xEDB88320u & (~(crc & 1u) + 1u));
        }
    }
    return ~crc;
}

void append_u32_be(std::vector<std::uint8_t>& out, std::uint32_t value) {
    out.push_back(static_cast<std::uint8_t>(value >> 24));
    out.push_back(static_cast<std::uint8_t>(value >> 16));
    out.push_back(static_cast<std::uint8_t>(value >> 8));
    out.push_back(static_cast<std::uint8_t>(value));
}

void append_chunk(
    std::vector<std::uint8_t>& out, const char* type, const std::vector<std::uint8_t>& payload) {
    append_u32_be(out, static_cast<std::uint32_t>(payload.size()));
    std::vector<std::uint8_t> body(type, type + 4);
    body.insert(body.end(), payload.begin(), payload.end());
    out.insert(out.end(), body.begin(), body.end());
    append_u32_be(out, crc32_bytes(body.data(), body.size(), 0));
}

// rgb: tightly packed RGB8 rows, top row first.
bool write_test_png_rgb(
    const std::filesystem::path& path, int width, int height,
    const std::vector<std::uint8_t>& rgb) {
    std::vector<std::uint8_t> raw; // filter byte 0 + scanline, per row
    raw.reserve(static_cast<std::size_t>(height) * (1 + static_cast<std::size_t>(width) * 3));
    for (int row = 0; row < height; ++row) {
        raw.push_back(0);
        const std::size_t offset = static_cast<std::size_t>(row) *
            static_cast<std::size_t>(width) * 3;
        raw.insert(raw.end(), rgb.begin() + static_cast<std::ptrdiff_t>(offset),
                   rgb.begin() + static_cast<std::ptrdiff_t>(offset +
                       static_cast<std::size_t>(width) * 3));
    }
    // zlib wrapper with stored deflate blocks.
    std::vector<std::uint8_t> zlib = { 0x78, 0x01 };
    std::size_t position = 0;
    while (position < raw.size()) {
        const std::size_t block = std::min<std::size_t>(65535, raw.size() - position);
        const bool final_block = position + block == raw.size();
        zlib.push_back(final_block ? 0x01 : 0x00);
        zlib.push_back(static_cast<std::uint8_t>(block & 0xFF));
        zlib.push_back(static_cast<std::uint8_t>(block >> 8));
        zlib.push_back(static_cast<std::uint8_t>(~block & 0xFF));
        zlib.push_back(static_cast<std::uint8_t>((~block >> 8) & 0xFF));
        zlib.insert(zlib.end(), raw.begin() + static_cast<std::ptrdiff_t>(position),
                    raw.begin() + static_cast<std::ptrdiff_t>(position + block));
        position += block;
    }
    std::uint32_t s1 = 1;
    std::uint32_t s2 = 0;
    for (const std::uint8_t byte : raw) {
        s1 = (s1 + byte) % 65521u;
        s2 = (s2 + s1) % 65521u;
    }
    append_u32_be(zlib, (s2 << 16) | s1);

    std::vector<std::uint8_t> file = { 0x89, 'P', 'N', 'G', 0x0D, 0x0A, 0x1A, 0x0A };
    std::vector<std::uint8_t> ihdr;
    append_u32_be(ihdr, static_cast<std::uint32_t>(width));
    append_u32_be(ihdr, static_cast<std::uint32_t>(height));
    ihdr.push_back(8); // bit depth
    ihdr.push_back(2); // color type RGB
    ihdr.push_back(0);
    ihdr.push_back(0);
    ihdr.push_back(0);
    append_chunk(file, "IHDR", ihdr);
    append_chunk(file, "IDAT", zlib);
    append_chunk(file, "IEND", {});

    std::filesystem::create_directories(path.parent_path());
    std::ofstream stream(path, std::ios::binary);
    if (!stream) {
        return false;
    }
    stream.write(reinterpret_cast<const char*>(file.data()),
                 static_cast<std::streamsize>(file.size()));
    return static_cast<bool>(stream);
}

std::string depth_model_path() {
    return std::string(AGBOT_FLIGHT_SIM_SOURCE_DIR) +
        "/data/models/depth_anything_v2_small.onnx";
}

void test_mono_depth_reason_codes() {
    const auto estimator = terrain::estimator_registry().create("mono_depth_onnx");
    terrain::ImageryBundle bundle;
    bundle.aoi = test_bounds();
    bundle.grid_width = 16;
    bundle.grid_height = 16;

    cfg::ParamTable bogus;
    bogus["model_path"] = cfg::ParamValue("/nonexistent/depth_model.onnx");
    const auto missing_model = estimator->estimate(bundle, bogus);
    expect(!missing_model.ok && missing_model.error.rfind("model_missing", 0) == 0,
           "mono_depth reports model_missing for bogus path");

    if (!std::filesystem::exists(depth_model_path())) {
        std::cout << "SKIP mono_depth reason codes needing the real model ("
                  << depth_model_path() << " absent; run tools/fetch_depth_model.sh)\n";
        return;
    }
    const auto no_rgb = estimator->estimate(bundle, {});
    expect(!no_rgb.ok && no_rgb.error == "rgb_missing",
           "mono_depth reports rgb_missing without imagery");
    expect(!estimator->accepts(bundle), "mono_depth accepts() false without imagery");

    bundle.rgb_image_path = "out/terrain/does_not_matter.png";
    const auto no_anchor = estimator->estimate(bundle, {});
    expect(!no_anchor.ok && no_anchor.error == "anchor_missing",
           "mono_depth reports anchor_missing without dem_prior");
}

void test_mono_depth_synthetic_end_to_end() {
    if (!std::filesystem::exists(depth_model_path())) {
        std::cout << "SKIP mono_depth synthetic end-to-end (model absent: "
                  << depth_model_path() << ")\n";
        return;
    }
    // Deterministic gradient scene with a bright square "structure".
    const int width = 96;
    const int height = 96;
    std::vector<std::uint8_t> rgb(static_cast<std::size_t>(width) * height * 3);
    for (int row = 0; row < height; ++row) {
        for (int col = 0; col < width; ++col) {
            const std::size_t i = (static_cast<std::size_t>(row) * width + col) * 3;
            const bool structure = row >= 32 && row < 64 && col >= 32 && col < 64;
            rgb[i + 0] = static_cast<std::uint8_t>(structure ? 230 : col * 2);
            rgb[i + 1] = static_cast<std::uint8_t>(structure ? 220 : row * 2);
            rgb[i + 2] = static_cast<std::uint8_t>(structure ? 210 : 96);
        }
    }
    const std::filesystem::path png_path =
        std::filesystem::path("out") / "terrain" / "mono_depth_gradient.png";
    expect(write_test_png_rgb(png_path, width, height, rgb), "synthetic RGB PNG written");
    const terrain::PngImage decoded = terrain::decode_png_rgba_file(png_path);
    expect(decoded.ok && decoded.width == width, "synthetic RGB PNG decodes back");

    terrain::ImageryBundle bundle;
    bundle.aoi = test_bounds();
    bundle.grid_width = 48;
    bundle.grid_height = 48;
    bundle.rgb_image_path = png_path.string();
    terrain::HeightField prior;
    prior.elevation = analytic_raster(bundle.aoi, 96);
    prior.confidence = terrain::Raster::filled(96, 96, bundle.aoi, 1.0f);
    prior.source_algorithm = "analytic_fixture";
    bundle.dem_prior = prior;

    const auto estimator = terrain::estimator_registry().create("mono_depth_onnx");
    expect(estimator->accepts(bundle), "mono_depth accepts RGB + dem_prior bundle");
    cfg::ParamTable params;
    params["input_size"] = cfg::ParamValue(252); // multiple of 14, fast on CPU
    const auto first = estimator->estimate(bundle, params);
    expect(first.ok, "mono_depth synthetic estimate succeeds" +
           (first.ok ? "" : " (" + first.error + ")"));
    if (!first.ok) {
        return;
    }
    expect(first.field.elevation.width == 48 && first.field.elevation.height == 48,
           "mono_depth honors requested grid");
    bool all_finite = true;
    float min_elev = std::numeric_limits<float>::max();
    float max_elev = std::numeric_limits<float>::lowest();
    for (const float value : first.field.elevation.values) {
        if (!std::isfinite(value)) {
            all_finite = false;
        } else {
            min_elev = std::min(min_elev, value);
            max_elev = std::max(max_elev, value);
        }
    }
    expect(all_finite, "mono_depth elevation values all finite");
    bool confidence_in_range = true;
    for (const float value : first.field.confidence.values) {
        if (!(value >= 0.05f && value <= 0.9f)) {
            confidence_in_range = false;
        }
    }
    expect(confidence_in_range, "mono_depth confidence within [0.05, 0.9]");
    // Anchored output must live in the DEM prior's ballpark (50..265 m).
    expect(min_elev > -200.0f && max_elev < 600.0f,
           "mono_depth anchored to DEM prior range (" +
           std::to_string(min_elev) + ".." + std::to_string(max_elev) + " m)");

    const auto second = estimator->estimate(bundle, params);
    expect(second.ok && terrain::raster_hash(first.field.elevation) ==
           terrain::raster_hash(second.field.elevation),
           "mono_depth deterministic across two runs");
}

void test_mono_depth_map_tile_smoke() {
    if (!std::filesystem::exists(depth_model_path())) {
        std::cout << "SKIP mono_depth map-tile smoke (model absent)\n";
        return;
    }
    // Prefer the deepest-zoom cached map tile whose AOI has real DEM
    // coverage in the z12/z13 elevation cache.
    const std::filesystem::path map_tiles =
        std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out" / "map_tiles";
    if (!std::filesystem::exists(map_tiles)) {
        std::cout << "SKIP mono_depth map-tile smoke (no cached map tiles)\n";
        return;
    }
    std::vector<std::filesystem::path> candidates;
    for (const auto& entry : std::filesystem::recursive_directory_iterator(map_tiles)) {
        if (entry.is_regular_file() && entry.path().extension() == ".png") {
            candidates.push_back(entry.path());
        }
    }
    std::sort(candidates.begin(), candidates.end(),
        [](const std::filesystem::path& a, const std::filesystem::path& b) {
            const auto zoom_of = [](const std::filesystem::path& p) {
                return std::stoi(p.parent_path().parent_path().filename().string());
            };
            const int za = zoom_of(a);
            const int zb = zoom_of(b);
            if (za != zb) {
                return za > zb;
            }
            return a.string() < b.string();
        });

    const auto dem_estimator = terrain::estimator_registry().create("dem_fusion");
    const auto mono = terrain::estimator_registry().create("mono_depth_onnx");
    for (const std::filesystem::path& tile_path : candidates) {
        const int y = std::stoi(tile_path.stem().string());
        const int x = std::stoi(tile_path.parent_path().filename().string());
        const int z = std::stoi(tile_path.parent_path().parent_path().filename().string());
        if (z < 12) {
            break; // sorted descending; low zooms span too much terrain
        }
        terrain::ImageryBundle bundle;
        bundle.aoi = fsim::TileCoordinate { z, x, y }.bounds();
        bundle.grid_width = 64;
        bundle.grid_height = 64;

        cfg::ParamTable dem_params;
        dem_params["zoom"] = cfg::ParamValue(12);
        dem_params["void_fill"] = cfg::ParamValue("idw");
        const auto dem = dem_estimator->estimate(bundle, dem_params);
        if (!dem.ok) {
            continue;
        }
        std::size_t trusted = 0;
        for (const float value : dem.field.confidence.values) {
            if (value >= 0.9f) {
                ++trusted;
            }
        }
        if (trusted < dem.field.confidence.values.size()) {
            continue; // needs full real DEM coverage to be a meaningful anchor
        }

        bundle.dem_prior = dem.field;
        bundle.rgb_image_path = tile_path.string();
        const auto result = mono->estimate(bundle, {});
        expect(result.ok, "mono_depth map-tile smoke estimate succeeds (" +
               tile_path.string() + ")" + (result.ok ? "" : " error=" + result.error));
        if (!result.ok) {
            return;
        }
        const auto metrics =
            terrain::compute_metrics(result.field.elevation, dem.field.elevation);
        expect(metrics.sample_count == 64 * 64,
               "mono_depth map-tile smoke covers full grid");
        expect(metrics.rmse < 30.0,
               "mono_depth anchored within 30 m RMSE of DEM (rmse=" +
               std::to_string(metrics.rmse) + " m)");
        // Consistency: re-fitting mono output against the DEM must be ~identity.
        const auto identity = terrain::fit_affine_sigma_clipped(
            result.field.elevation.values, dem.field.elevation.values, 1, 2.0);
        std::cout << "  map-tile smoke: tile z" << z << "/" << x << "/" << y
                  << " rmse=" << metrics.rmse << " m bias=" << metrics.bias
                  << " m refit a=" << identity.a << " b=" << identity.b << "\n";
        return;
    }
    std::cout << "SKIP mono_depth map-tile smoke (no map tile with full DEM coverage)\n";
}

#endif // AGBOT_TERRAIN_HAS_ONNX

void test_pipeline_determinism() {
    const char* kConfig = R"toml(
[pipeline]
target_gsd_m = 20.0
resolution = 40
aoi = { min_lat = 40.70, min_lon = -74.02, max_lat = 40.82, max_lon = -73.93 }

[[layer]]
algorithm = "dem_fusion"
weight = 1.0
  [layer.params]
  source = "terrarium"
  void_fill = "idw"

[[layer]]
algorithm = "synthetic_detail"
weight = 1.0
  [layer.params]
  amplitude_m = 2.0
  octaves = 3
  frequency = 5.0
  seed = 7

[fusion]
method = "detail_injection"
lambda = 0.5
cutoff_cells = 2

[validation]
enabled = true
reference_layer = 0
)toml";
    const auto parsed = cfg::parse_toml(kConfig);
    expect(parsed.ok, "pipeline config parses");
    if (!parsed.ok) {
        return;
    }
    const auto first = terrain::run_terrain_pipeline(parsed.root);
    const auto second = terrain::run_terrain_pipeline(parsed.root);
    expect(first.ok, "pipeline run succeeds" + (first.ok ? "" : " (" + first.error + ")"));
    if (!first.ok || !second.ok) {
        return;
    }
    expect(first.param_hash == cfg::param_hash(parsed.root), "pipeline records param hash");
    expect(terrain::raster_hash(first.fused.elevation) ==
           terrain::raster_hash(second.fused.elevation),
           "pipeline deterministic: identical fused raster hash");
    expect(first.fused.elevation.width == 40 && first.fused.elevation.height == 40,
           "pipeline honors configured resolution");
    expect(first.validation.ok && first.validation.metrics.sample_count == 1600,
           "pipeline validation ran over full grid");

    const auto mesh = terrain::mesh_from_heightfield(first.fused);
    expect(mesh.ok && mesh.mesh.vertices.size() == 1600 && !mesh.mesh.indices.empty(),
           "mesh bridge produces renderable mesh");
}

void test_pipeline_default_config_file() {
    const std::filesystem::path config_path =
        std::filesystem::path(AGBOT_TERRAIN_SOURCE_DIR) / "configs" / "default_terrain.toml";
    const auto result = terrain::run_terrain_pipeline_file(config_path);
    expect(result.ok, "default_terrain.toml pipeline runs" +
           (result.ok ? "" : " (" + result.error + ")"));
    if (result.ok) {
        expect(std::filesystem::exists("out/terrain/validation.json"),
               "default pipeline wrote out/terrain/validation.json");
    }
}

} // namespace

int main() {
    test_raster_sampling();
    test_registry();
    test_dem_prior_roundtrip();
    test_dem_clamp();
    test_synthetic_detail_determinism();
    test_fusion_dem_locked();
    test_fusion_confidence_weighted();
    test_fusion_detail_injection();
    test_validation_metrics();
    test_validation_json();
    test_inflate_stored_and_png_synthetic();
    test_real_tile_decode();
    test_affine_fit();
#if defined(AGBOT_TERRAIN_HAS_ONNX)
    test_mono_depth_reason_codes();
    test_mono_depth_synthetic_end_to_end();
    test_mono_depth_map_tile_smoke();
#else
    std::cout << "SKIP mono_depth ONNX tests (built without ONNX Runtime)\n";
#endif
    test_pipeline_determinism();
    test_pipeline_default_config_file();

    if (failures > 0) {
        std::cout << failures << " failure(s)\n";
        return 1;
    }
    std::cout << "all terrain tests passed\n";
    return 0;
}
