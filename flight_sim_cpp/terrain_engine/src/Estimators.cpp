#include "agbot_terrain/ElevationEstimator.hpp"

#include "agbot_terrain/Fusion.hpp"
#include "agbot_terrain/MonoDepth.hpp"
#include "agbot_terrain/Png.hpp"

#include <algorithm>
#include <cmath>
#include <filesystem>
#include <limits>
#include <sstream>

#if defined(AGBOT_TERRAIN_HAS_ONNX)
#include <onnxruntime_cxx_api.h>

#include <array>
#include <cstddef>
#include <memory>
#include <mutex>
#include <unordered_map>
#endif

#ifndef AGBOT_FLIGHT_SIM_SOURCE_DIR
#define AGBOT_FLIGHT_SIM_SOURCE_DIR "."
#endif

namespace agbot::terrain {
namespace {

namespace fs = agbot::flight_sim;
namespace cfg = agbot::config;

std::uint64_t fnv1a_mix(std::uint64_t hash, std::uint64_t value) {
    for (int i = 0; i < 8; ++i) {
        hash ^= (value >> (i * 8)) & 0xFFu;
        hash *= 1099511628211ULL;
    }
    return hash;
}

// Deterministic lattice noise value in [0, 1) from integer coordinates.
float lattice_noise(std::int64_t ix, std::int64_t iy, std::uint64_t seed) {
    std::uint64_t hash = 1469598103934665603ULL;
    hash = fnv1a_mix(hash, seed);
    hash = fnv1a_mix(hash, static_cast<std::uint64_t>(ix));
    hash = fnv1a_mix(hash, static_cast<std::uint64_t>(iy));
    return static_cast<float>(hash >> 40) / static_cast<float>(1u << 24);
}

float smoothstep(float t) {
    return t * t * (3.0f - 2.0f * t);
}

float value_noise(double x, double y, std::uint64_t seed) {
    const std::int64_t ix = static_cast<std::int64_t>(std::floor(x));
    const std::int64_t iy = static_cast<std::int64_t>(std::floor(y));
    const float tx = smoothstep(static_cast<float>(x - std::floor(x)));
    const float ty = smoothstep(static_cast<float>(y - std::floor(y)));
    const float v00 = lattice_noise(ix, iy, seed);
    const float v10 = lattice_noise(ix + 1, iy, seed);
    const float v01 = lattice_noise(ix, iy + 1, seed);
    const float v11 = lattice_noise(ix + 1, iy + 1, seed);
    const float top = v00 + (v10 - v00) * tx;
    const float bottom = v01 + (v11 - v01) * tx;
    return top + (bottom - top) * ty;
}

// Fill nodata cells by inverse-distance weighting over valid cells within an
// expanding window (small grids, so a bounded search is fine).
void void_fill_idw(Raster& raster) {
    if (!raster.valid()) {
        return;
    }
    const Raster source = raster;
    const int max_radius = std::max(raster.width, raster.height);
    for (int row = 0; row < raster.height; ++row) {
        for (int col = 0; col < raster.width; ++col) {
            if (!Raster::is_nodata(source.at(row, col))) {
                continue;
            }
            double weight_sum = 0.0;
            double value_sum = 0.0;
            for (int radius = 1; radius <= max_radius && weight_sum == 0.0; ++radius) {
                const int r0 = std::max(0, row - radius);
                const int r1 = std::min(raster.height - 1, row + radius);
                const int c0 = std::max(0, col - radius);
                const int c1 = std::min(raster.width - 1, col + radius);
                for (int r = r0; r <= r1; ++r) {
                    for (int c = c0; c <= c1; ++c) {
                        const float value = source.at(r, c);
                        if (Raster::is_nodata(value)) {
                            continue;
                        }
                        const double dr = static_cast<double>(r - row);
                        const double dc = static_cast<double>(c - col);
                        const double distance_sq = dr * dr + dc * dc;
                        const double weight = 1.0 / distance_sq;
                        weight_sum += weight;
                        value_sum += weight * static_cast<double>(value);
                    }
                }
            }
            if (weight_sum > 0.0) {
                raster.set(row, col, static_cast<float>(value_sum / weight_sum));
            }
        }
    }
}

// Resample a source raster onto the AOI grid (nearest or bilinear).
Raster resample_to_grid(
    const Raster& source,
    const GeoBounds& aoi,
    int resolution,
    const std::string& resample) {
    Raster output = Raster::filled(resolution, resolution, aoi, Raster::nodata());
    const double lat_span = aoi.max_latitude - aoi.min_latitude;
    const double lon_span = aoi.max_longitude - aoi.min_longitude;
    for (int row = 0; row < resolution; ++row) {
        for (int col = 0; col < resolution; ++col) {
            const double v = resolution == 1
                ? 0.0 : static_cast<double>(row) / static_cast<double>(resolution - 1);
            const double u = resolution == 1
                ? 0.0 : static_cast<double>(col) / static_cast<double>(resolution - 1);
            const double latitude = aoi.max_latitude - v * lat_span;
            const double longitude = aoi.min_longitude + u * lon_span;
            if (resample == "nearest") {
                const double src_lat_span = source.bounds.max_latitude - source.bounds.min_latitude;
                const double src_lon_span = source.bounds.max_longitude - source.bounds.min_longitude;
                if (src_lat_span <= 0.0 || src_lon_span <= 0.0 ||
                    latitude < source.bounds.min_latitude || latitude > source.bounds.max_latitude ||
                    longitude < source.bounds.min_longitude ||
                    longitude > source.bounds.max_longitude) {
                    continue;
                }
                const int src_col = std::clamp(
                    static_cast<int>(std::lround((longitude - source.bounds.min_longitude) /
                        src_lon_span * static_cast<double>(source.width - 1))),
                    0, source.width - 1);
                const int src_row = std::clamp(
                    static_cast<int>(std::lround((source.bounds.max_latitude - latitude) /
                        src_lat_span * static_cast<double>(source.height - 1))),
                    0, source.height - 1);
                output.set(row, col, source.at(src_row, src_col));
            } else {
                const auto sampled = source.sample_at(latitude, longitude);
                if (sampled.has_value()) {
                    output.set(row, col, *sampled);
                }
            }
        }
    }
    return output;
}

// ---------------------------------------------------------------------------
// dem_fusion — cached Terrarium tile compositing via the existing GeoTerrain
// path, or resampling of a supplied DEM prior ("source" param).
// ---------------------------------------------------------------------------
class DemFusionEstimator final : public ElevationEstimator {
public:
    [[nodiscard]] std::string name() const override { return "dem_fusion"; }

    [[nodiscard]] bool accepts(const ImageryBundle& bundle) const override {
        return bundle.aoi.max_latitude > bundle.aoi.min_latitude &&
            bundle.aoi.max_longitude > bundle.aoi.min_longitude;
    }

    [[nodiscard]] EstimateResult estimate(
        const ImageryBundle& bundle,
        const cfg::ParamTable& params) const override {
        const std::string source = cfg::string_or(params, "source", "terrarium");
        if (source == "prior") {
            return estimate_from_prior(bundle, params);
        }
        if (source == "terrarium") {
            return estimate_from_tiles(bundle, params);
        }
        EstimateResult result;
        result.error = "dem_fusion_unknown_source:" + source;
        return result;
    }

private:
    [[nodiscard]] EstimateResult estimate_from_prior(
        const ImageryBundle& bundle,
        const cfg::ParamTable& params) const {
        EstimateResult result;
        if (!bundle.dem_prior.has_value() || !bundle.dem_prior->valid()) {
            result.error = "dem_prior_missing";
            return result;
        }
        const int resolution = bundle.resolved_resolution();
        const std::string resample = cfg::string_or(params, "resample", "bilinear");
        Raster elevation =
            resample_to_grid(bundle.dem_prior->elevation, bundle.aoi, resolution, resample);
        finalize(elevation, params);
        result.field.confidence = Raster::filled(resolution, resolution, bundle.aoi, 1.0f);
        for (std::size_t i = 0; i < elevation.values.size(); ++i) {
            if (Raster::is_nodata(elevation.values[i])) {
                result.field.confidence.values[i] = 0.0f;
            }
        }
        result.field.elevation = std::move(elevation);
        result.field.source_algorithm = name();
        result.ok = true;
        return result;
    }

    [[nodiscard]] EstimateResult estimate_from_tiles(
        const ImageryBundle& bundle,
        const cfg::ParamTable& params) const {
        EstimateResult result;
        const int resolution = bundle.resolved_resolution();
        const double radius_m = std::max(bundle.aoi.width_m(), bundle.aoi.height_m()) / 2.0;
        int zoom = static_cast<int>(cfg::integer_or(params, "zoom", 0));
        if (zoom <= 0) {
            zoom = fs::zoom_for_radius_m(std::max(radius_m, 1.0));
        }
        zoom = std::clamp(zoom, 1, 15);
        const std::string tiles_dir = cfg::string_or(
            params, "tiles_dir", std::string(AGBOT_FLIGHT_SIM_SOURCE_DIR) + "/out/elevation_tiles");

        const std::vector<fs::TileCoordinate> expected = fs::tiles_for_bounds(bundle.aoi, zoom);
        std::vector<fs::ElevationTile> tiles;
        tiles.reserve(expected.size());
        std::size_t decode_failures = 0;
        for (const fs::TileCoordinate& coordinate : expected) {
            std::ostringstream path;
            path << tiles_dir << '/' << coordinate.z << '/' << coordinate.x << '/'
                 << coordinate.y << ".png";
            if (!std::filesystem::exists(path.str())) {
                continue;
            }
            const PngImage png = decode_png_rgba_file(path.str());
            if (!png.ok) {
                ++decode_failures;
                continue;
            }
            auto tile = fs::elevation_tile_from_terrarium_rgba(
                coordinate, png.width, png.height, png.rgba);
            if (tile.has_value()) {
                tiles.push_back(std::move(*tile));
            }
        }

        const fs::ElevationComposite composite =
            fs::composite_elevation_with_state(tiles, bundle.aoi, resolution, expected);

        Raster elevation;
        elevation.width = resolution;
        elevation.height = resolution;
        elevation.bounds = bundle.aoi;
        elevation.values = composite.heightmap;
        finalize(elevation, params);

        // Per-cell confidence from the per-tile composite state: real tiles
        // are trusted; fallback/missing coverage gets low confidence.
        Raster confidence = Raster::filled(resolution, resolution, bundle.aoi, 0.1f);
        const double lat_span = bundle.aoi.max_latitude - bundle.aoi.min_latitude;
        const double lon_span = bundle.aoi.max_longitude - bundle.aoi.min_longitude;
        for (int row = 0; row < resolution; ++row) {
            for (int col = 0; col < resolution; ++col) {
                const double v = resolution == 1
                    ? 0.0 : static_cast<double>(row) / static_cast<double>(resolution - 1);
                const double u = resolution == 1
                    ? 0.0 : static_cast<double>(col) / static_cast<double>(resolution - 1);
                const fs::GeoCoordinate coordinate {
                    bundle.aoi.max_latitude - v * lat_span,
                    bundle.aoi.min_longitude + u * lon_span,
                    0.0,
                };
                const fs::TileCoordinate cell_tile = fs::tile_for_geo(coordinate, zoom);
                for (const fs::TerrainTileStatus& status : composite.tile_states) {
                    if (status.coordinate.z == cell_tile.z &&
                        status.coordinate.x == cell_tile.x &&
                        status.coordinate.y == cell_tile.y) {
                        confidence.set(row, col,
                            status.state == fs::TerrainTileState::Available ? 1.0f : 0.1f);
                        break;
                    }
                }
            }
        }

        if (decode_failures > 0 && tiles.empty()) {
            result.error = "dem_fusion_all_tiles_failed_decode";
            return result;
        }
        result.field.elevation = std::move(elevation);
        result.field.confidence = std::move(confidence);
        result.field.source_algorithm = name();
        result.ok = true;
        return result;
    }

    static void finalize(Raster& elevation, const cfg::ParamTable& params) {
        const std::string void_fill = cfg::string_or(params, "void_fill", "none");
        if (void_fill == "idw") {
            void_fill_idw(elevation);
        }
        // Optional elevation clamp. Terrarium tiles carry coarse ocean
        // bathymetry (hundreds of meters below sea level near coasts);
        // city-scale worlds usually clamp water to sea level instead.
        const double clamp_min = cfg::double_or(params, "clamp_min_m",
            -std::numeric_limits<double>::infinity());
        const double clamp_max = cfg::double_or(params, "clamp_max_m",
            std::numeric_limits<double>::infinity());
        if (std::isfinite(clamp_min) || std::isfinite(clamp_max)) {
            for (float& value : elevation.values) {
                if (!Raster::is_nodata(value)) {
                    value = static_cast<float>(
                        std::clamp(static_cast<double>(value), clamp_min, clamp_max));
                }
            }
        }
        const double sigma = cfg::double_or(params, "smoothing_sigma", 0.0);
        if (sigma > 0.0) {
            elevation = gaussian_blur(elevation, sigma);
        }
    }
};

// ---------------------------------------------------------------------------
// synthetic_detail — deterministic fractal value noise standing in for the
// learned mono-depth detail layer so fusion is exercisable everywhere.
// ---------------------------------------------------------------------------
class SyntheticDetailEstimator final : public ElevationEstimator {
public:
    [[nodiscard]] std::string name() const override { return "synthetic_detail"; }

    [[nodiscard]] bool accepts(const ImageryBundle& bundle) const override {
        return bundle.aoi.max_latitude > bundle.aoi.min_latitude &&
            bundle.aoi.max_longitude > bundle.aoi.min_longitude;
    }

    [[nodiscard]] EstimateResult estimate(
        const ImageryBundle& bundle,
        const cfg::ParamTable& params) const override {
        EstimateResult result;
        const int resolution = bundle.resolved_resolution();
        const double amplitude_m = cfg::double_or(params, "amplitude_m", 2.0);
        const int octaves =
            std::clamp(static_cast<int>(cfg::integer_or(params, "octaves", 4)), 1, 12);
        const double frequency = std::max(cfg::double_or(params, "frequency", 4.0), 1e-6);
        const std::uint64_t seed =
            static_cast<std::uint64_t>(cfg::integer_or(params, "seed", 1337));
        const double confidence_value =
            std::clamp(cfg::double_or(params, "confidence", 0.3), 0.0, 1.0);

        Raster elevation = Raster::filled(resolution, resolution, bundle.aoi, 0.0f);
        for (int row = 0; row < resolution; ++row) {
            for (int col = 0; col < resolution; ++col) {
                const double ny = resolution == 1
                    ? 0.0 : static_cast<double>(row) / static_cast<double>(resolution - 1);
                const double nx = resolution == 1
                    ? 0.0 : static_cast<double>(col) / static_cast<double>(resolution - 1);
                double sum = 0.0;
                double gain = 1.0;
                double gain_total = 0.0;
                double octave_frequency = frequency;
                for (int octave = 0; octave < octaves; ++octave) {
                    const std::uint64_t octave_seed =
                        seed * 1099511628211ULL + static_cast<std::uint64_t>(octave);
                    sum += gain * static_cast<double>(
                        value_noise(nx * octave_frequency, ny * octave_frequency, octave_seed));
                    gain_total += gain;
                    gain *= 0.5;
                    octave_frequency *= 2.0;
                }
                const double normalized = sum / gain_total; // [0, 1)
                elevation.set(row, col,
                    static_cast<float>((normalized - 0.5) * 2.0 * amplitude_m));
            }
        }
        result.field.elevation = std::move(elevation);
        result.field.confidence = Raster::filled(
            resolution, resolution, bundle.aoi, static_cast<float>(confidence_value));
        result.field.source_algorithm = name();
        result.ok = true;
        return result;
    }
};

// ---------------------------------------------------------------------------
// mono_depth_onnx — compile-gated. Without ONNX Runtime this registers a stub
// so pipelines can name the layer and get a reason-coded error; the real
// implementation drops in behind the same interface when
// AGBOT_TERRAIN_HAS_ONNX is defined by the build.
// ---------------------------------------------------------------------------
#if defined(AGBOT_TERRAIN_HAS_ONNX)

std::string default_depth_model_path() {
    return std::string(AGBOT_FLIGHT_SIM_SOURCE_DIR) +
        "/data/models/depth_anything_v2_small.onnx";
}

// Read a 3-element float array param (e.g. norm_mean) with a fallback.
std::array<float, 3> vec3_or(
    const cfg::ParamTable& params, const std::string& key, std::array<float, 3> fallback) {
    const cfg::ParamArray* array = cfg::find_array(params, key);
    if (array == nullptr || array->size() != 3) {
        return fallback;
    }
    std::array<float, 3> out = fallback;
    for (std::size_t i = 0; i < 3; ++i) {
        if (!(*array)[i].is_number()) {
            return fallback;
        }
        out[i] = static_cast<float>((*array)[i].as_double());
    }
    return out;
}

// Bilinear sample of one channel of a tightly packed RGBA image at fractional
// pixel coordinates (clamped to the image).
float sample_rgba_channel(
    const PngImage& image, double x, double y, int channel) {
    const double cx = std::clamp(x, 0.0, static_cast<double>(image.width - 1));
    const double cy = std::clamp(y, 0.0, static_cast<double>(image.height - 1));
    const int x0 = static_cast<int>(std::floor(cx));
    const int y0 = static_cast<int>(std::floor(cy));
    const int x1 = std::min(x0 + 1, image.width - 1);
    const int y1 = std::min(y0 + 1, image.height - 1);
    const float tx = static_cast<float>(cx - x0);
    const float ty = static_cast<float>(cy - y0);
    const auto pixel = [&](int px, int py) {
        const std::size_t index =
            (static_cast<std::size_t>(py) * static_cast<std::size_t>(image.width) +
             static_cast<std::size_t>(px)) * 4u + static_cast<std::size_t>(channel);
        return static_cast<float>(image.rgba[index]);
    };
    const float top = pixel(x0, y0) + (pixel(x1, y0) - pixel(x0, y0)) * tx;
    const float bottom = pixel(x0, y1) + (pixel(x1, y1) - pixel(x0, y1)) * tx;
    return top + (bottom - top) * ty;
}

// Bilinear sample of a row-major float plane (height x width) at fractional
// coordinates, clamped.
float sample_plane(
    const std::vector<float>& plane, int width, int height, double x, double y) {
    const double cx = std::clamp(x, 0.0, static_cast<double>(width - 1));
    const double cy = std::clamp(y, 0.0, static_cast<double>(height - 1));
    const int x0 = static_cast<int>(std::floor(cx));
    const int y0 = static_cast<int>(std::floor(cy));
    const int x1 = std::min(x0 + 1, width - 1);
    const int y1 = std::min(y0 + 1, height - 1);
    const float tx = static_cast<float>(cx - x0);
    const float ty = static_cast<float>(cy - y0);
    const auto value = [&](int px, int py) {
        return plane[static_cast<std::size_t>(py) * static_cast<std::size_t>(width) +
                     static_cast<std::size_t>(px)];
    };
    const float top = value(x0, y0) + (value(x1, y0) - value(x0, y0)) * tx;
    const float bottom = value(x0, y1) + (value(x1, y1) - value(x0, y1)) * tx;
    return top + (bottom - top) * ty;
}

// Cached ORT session so repeated estimates (fusion layers, determinism
// re-runs) do not reload the ~100 MB model each call.
struct OrtSessionCache {
    std::mutex mutex;
    std::string model_path;
    std::string execution_provider; // provider actually active
    std::unique_ptr<Ort::Session> session;
};

Ort::Env& ort_env() {
    static Ort::Env env(ORT_LOGGING_LEVEL_ERROR, "agbot_terrain");
    return env;
}

class MonoDepthOnnxEstimator final : public ElevationEstimator {
public:
    [[nodiscard]] std::string name() const override { return "mono_depth_onnx"; }

    [[nodiscard]] bool accepts(const ImageryBundle& bundle) const override {
        // Params are not available here; check against the default model
        // location and the bundle contract (RGB imagery + metric anchor).
        return std::filesystem::exists(default_depth_model_path()) &&
            !bundle.rgb_image_path.empty() &&
            bundle.dem_prior.has_value() && bundle.dem_prior->valid();
    }

    [[nodiscard]] EstimateResult estimate(
        const ImageryBundle& bundle,
        const cfg::ParamTable& params) const override {
        EstimateResult result;
        const std::string model_path =
            cfg::string_or(params, "model_path", default_depth_model_path());
        if (!std::filesystem::exists(model_path)) {
            result.error = "model_missing:" + model_path;
            return result;
        }
        if (bundle.rgb_image_path.empty()) {
            result.error = "rgb_missing";
            return result;
        }
        const std::string metric_mode =
            cfg::string_or(params, "metric_mode", "relative_affine_fit");
        const std::string scale_anchor = cfg::string_or(params, "scale_anchor", "dem_prior");
        if (metric_mode != "relative_affine_fit") {
            result.error = "unknown_metric_mode:" + metric_mode;
            return result;
        }
        if (scale_anchor != "dem_prior") {
            result.error = "unknown_scale_anchor:" + scale_anchor;
            return result;
        }
        if (!bundle.dem_prior.has_value() || !bundle.dem_prior->valid()) {
            result.error = "anchor_missing";
            return result;
        }

        const PngImage rgb = decode_png_rgba_file(bundle.rgb_image_path);
        if (!rgb.ok || rgb.width <= 0 || rgb.height <= 0) {
            result.error = "rgb_decode_failed:" + rgb.error;
            return result;
        }

        // Model input side; Depth-Anything-V2 patch size is 14, keep the
        // square side a multiple of it (default 518 = 14 * 37).
        int input_size = static_cast<int>(cfg::integer_or(params, "input_size", 518));
        input_size = std::clamp(input_size, 70, 1918);
        input_size = (input_size / 14) * 14;
        const bool invert = cfg::bool_or(params, "invert", true);
        const std::array<float, 3> mean =
            vec3_or(params, "norm_mean", { 0.485f, 0.456f, 0.406f });
        const std::array<float, 3> stddev =
            vec3_or(params, "norm_std", { 0.229f, 0.224f, 0.225f });
        const std::string execution_provider =
            cfg::string_or(params, "execution_provider", "cpu");

        // Letterbox geometry: scale the image into the square model input,
        // pad the borders with the (normalized-zero) channel means.
        const double scale = std::min(
            static_cast<double>(input_size) / rgb.width,
            static_cast<double>(input_size) / rgb.height);
        const int content_w = std::max(1, static_cast<int>(std::lround(rgb.width * scale)));
        const int content_h = std::max(1, static_cast<int>(std::lround(rgb.height * scale)));
        const int offset_x = (input_size - content_w) / 2;
        const int offset_y = (input_size - content_h) / 2;

        const std::size_t plane = static_cast<std::size_t>(input_size) *
            static_cast<std::size_t>(input_size);
        std::vector<float> input_tensor(plane * 3u, 0.0f);
        for (int channel = 0; channel < 3; ++channel) {
            float* dst = input_tensor.data() + static_cast<std::size_t>(channel) * plane;
            for (int row = 0; row < input_size; ++row) {
                for (int col = 0; col < input_size; ++col) {
                    float value01;
                    if (row < offset_y || row >= offset_y + content_h ||
                        col < offset_x || col >= offset_x + content_w) {
                        value01 = mean[static_cast<std::size_t>(channel)];
                    } else {
                        const double src_x = content_w == 1 ? 0.0
                            : static_cast<double>(col - offset_x) /
                              static_cast<double>(content_w - 1) * (rgb.width - 1);
                        const double src_y = content_h == 1 ? 0.0
                            : static_cast<double>(row - offset_y) /
                              static_cast<double>(content_h - 1) * (rgb.height - 1);
                        value01 = sample_rgba_channel(rgb, src_x, src_y, channel) / 255.0f;
                    }
                    dst[static_cast<std::size_t>(row) * static_cast<std::size_t>(input_size) +
                        static_cast<std::size_t>(col)] =
                        (value01 - mean[static_cast<std::size_t>(channel)]) /
                        stddev[static_cast<std::size_t>(channel)];
                }
            }
        }

        // Inference. ORT throws; keep exceptions inside this boundary.
        std::vector<float> depth;
        int depth_w = 0;
        int depth_h = 0;
        std::string active_provider = "cpu";
        try {
            static OrtSessionCache cache;
            std::lock_guard<std::mutex> lock(cache.mutex);
            if (cache.session == nullptr || cache.model_path != model_path ||
                cache.execution_provider != execution_provider) {
                Ort::SessionOptions options;
                options.SetGraphOptimizationLevel(ORT_ENABLE_ALL);
                std::string provider = "cpu";
                if (execution_provider == "coreml") {
                    try {
                        std::unordered_map<std::string, std::string> coreml_options;
                        options.AppendExecutionProvider("CoreML", coreml_options);
                        provider = "coreml";
                    } catch (const std::exception&) {
                        options = Ort::SessionOptions();
                        options.SetGraphOptimizationLevel(ORT_ENABLE_ALL);
                        provider = "cpu";
                    }
                }
                try {
                    cache.session = std::make_unique<Ort::Session>(
                        ort_env(), model_path.c_str(), options);
                } catch (const std::exception& error) {
                    cache.session.reset();
                    result.error = std::string("ort_init_failed:") + error.what();
                    return result;
                }
                cache.model_path = model_path;
                cache.execution_provider = execution_provider;
            }
            active_provider =
                cache.execution_provider == "coreml" ? "coreml" : "cpu";
            Ort::Session& session = *cache.session;

            Ort::AllocatorWithDefaultOptions allocator;
            const auto input_name = session.GetInputNameAllocated(0, allocator);
            const auto output_name = session.GetOutputNameAllocated(0, allocator);
            const std::array<std::int64_t, 4> input_shape = {
                1, 3, static_cast<std::int64_t>(input_size),
                static_cast<std::int64_t>(input_size)
            };
            Ort::MemoryInfo memory_info =
                Ort::MemoryInfo::CreateCpu(OrtArenaAllocator, OrtMemTypeDefault);
            Ort::Value input_value = Ort::Value::CreateTensor<float>(
                memory_info, input_tensor.data(), input_tensor.size(),
                input_shape.data(), input_shape.size());

            const char* input_names[] = { input_name.get() };
            const char* output_names[] = { output_name.get() };
            auto outputs = session.Run(
                Ort::RunOptions{ nullptr }, input_names, &input_value, 1, output_names, 1);
            if (outputs.empty() || !outputs.front().IsTensor()) {
                result.error = "inference_failed:no_tensor_output";
                return result;
            }
            const auto info = outputs.front().GetTensorTypeAndShapeInfo();
            const std::vector<std::int64_t> shape = info.GetShape();
            if (shape.size() < 2) {
                result.error = "inference_failed:unexpected_output_rank";
                return result;
            }
            depth_h = static_cast<int>(shape[shape.size() - 2]);
            depth_w = static_cast<int>(shape[shape.size() - 1]);
            const std::size_t expected =
                static_cast<std::size_t>(depth_h) * static_cast<std::size_t>(depth_w);
            if (depth_h <= 0 || depth_w <= 0 || info.GetElementCount() != expected) {
                result.error = "inference_failed:unexpected_output_shape";
                return result;
            }
            const float* data = outputs.front().GetTensorData<float>();
            depth.assign(data, data + expected);
        } catch (const std::exception& error) {
            result.error = std::string("inference_failed:") + error.what();
            return result;
        } catch (...) {
            result.error = "inference_failed:unknown";
            return result;
        }

        // Un-letterbox the relative depth back over the AOI grid. The output
        // plane spans the padded square; sample only the content rectangle.
        const int resolution = bundle.resolved_resolution();
        Raster relative = Raster::filled(resolution, resolution, bundle.aoi, Raster::nodata());
        const double out_scale_x = depth_w == 1 || input_size == 1
            ? 0.0 : static_cast<double>(depth_w - 1) / static_cast<double>(input_size - 1);
        const double out_scale_y = depth_h == 1 || input_size == 1
            ? 0.0 : static_cast<double>(depth_h - 1) / static_cast<double>(input_size - 1);
        for (int row = 0; row < resolution; ++row) {
            for (int col = 0; col < resolution; ++col) {
                const double v = resolution == 1
                    ? 0.0 : static_cast<double>(row) / static_cast<double>(resolution - 1);
                const double u = resolution == 1
                    ? 0.0 : static_cast<double>(col) / static_cast<double>(resolution - 1);
                const double letter_x = offset_x + u * (content_w - 1);
                const double letter_y = offset_y + v * (content_h - 1);
                const float raw = sample_plane(
                    depth, depth_w, depth_h, letter_x * out_scale_x, letter_y * out_scale_y);
                relative.set(row, col, invert ? -raw : raw);
            }
        }

        // Anchor: affine fit against the DEM prior resampled onto this grid.
        const Raster prior = resample_to_grid(
            bundle.dem_prior->elevation, bundle.aoi, resolution, "bilinear");
        std::vector<float> xs;
        std::vector<float> ys;
        xs.reserve(relative.values.size());
        ys.reserve(relative.values.size());
        for (std::size_t i = 0; i < relative.values.size(); ++i) {
            if (!Raster::is_nodata(relative.values[i]) &&
                !Raster::is_nodata(prior.values[i])) {
                xs.push_back(relative.values[i]);
                ys.push_back(prior.values[i]);
            }
        }
        const AffineFit fit = fit_affine_sigma_clipped(xs, ys, 3, 2.0);
        if (!fit.ok) {
            result.error = "affine_fit_failed";
            return result;
        }

        Raster elevation = Raster::filled(resolution, resolution, bundle.aoi, Raster::nodata());
        Raster confidence = Raster::filled(resolution, resolution, bundle.aoi, 0.05f);
        const double residual_scale = fit.sigma > 1e-9 ? 3.0 * fit.sigma : 1.0;
        for (std::size_t i = 0; i < relative.values.size(); ++i) {
            if (Raster::is_nodata(relative.values[i])) {
                continue;
            }
            const double metric = fit.a * static_cast<double>(relative.values[i]) + fit.b;
            elevation.values[i] = static_cast<float>(metric);
            if (!Raster::is_nodata(prior.values[i])) {
                const double residual = std::abs(metric - static_cast<double>(prior.values[i]));
                const double normalized = std::min(residual / residual_scale, 1.0);
                confidence.values[i] = static_cast<float>(
                    std::clamp(0.5 * (1.0 - normalized), 0.05, 0.9));
            }
        }

        result.field.elevation = std::move(elevation);
        result.field.confidence = std::move(confidence);
        result.field.source_algorithm = name();
        result.ok = true;
        (void)active_provider;
        return result;
    }
};
#else
class MonoDepthOnnxEstimator final : public ElevationEstimator {
public:
    [[nodiscard]] std::string name() const override { return "mono_depth_onnx"; }

    [[nodiscard]] bool accepts(const ImageryBundle& bundle) const override {
        (void)bundle;
        return false;
    }

    [[nodiscard]] EstimateResult estimate(
        const ImageryBundle& bundle,
        const cfg::ParamTable& params) const override {
        (void)bundle;
        (void)params;
        EstimateResult result;
        result.error = "onnx_runtime_unavailable";
        return result;
    }
};
#endif

} // namespace

AffineFit fit_affine_sigma_clipped(
    const std::vector<float>& x,
    const std::vector<float>& y,
    int rounds,
    double sigma_clip) {
    AffineFit fit;
    if (x.size() != y.size() || x.size() < 2 || rounds < 1) {
        return fit;
    }
    std::vector<bool> keep(x.size(), true);
    for (std::size_t i = 0; i < x.size(); ++i) {
        keep[i] = std::isfinite(x[i]) && std::isfinite(y[i]);
    }
    double a = 0.0;
    double b = 0.0;
    double sigma = 0.0;
    int inliers = 0;
    for (int round = 0; round < rounds; ++round) {
        double sx = 0.0;
        double sy = 0.0;
        double sxx = 0.0;
        double sxy = 0.0;
        int n = 0;
        for (std::size_t i = 0; i < x.size(); ++i) {
            if (!keep[i]) {
                continue;
            }
            const double xv = static_cast<double>(x[i]);
            const double yv = static_cast<double>(y[i]);
            sx += xv;
            sy += yv;
            sxx += xv * xv;
            sxy += xv * yv;
            ++n;
        }
        if (n < 2) {
            return fit;
        }
        const double denom = static_cast<double>(n) * sxx - sx * sx;
        if (std::abs(denom) < 1e-12) {
            return fit; // degenerate: all x identical
        }
        a = (static_cast<double>(n) * sxy - sx * sy) / denom;
        b = (sy - a * sx) / static_cast<double>(n);
        double residual_sq = 0.0;
        for (std::size_t i = 0; i < x.size(); ++i) {
            if (!keep[i]) {
                continue;
            }
            const double r = static_cast<double>(y[i]) -
                (a * static_cast<double>(x[i]) + b);
            residual_sq += r * r;
        }
        sigma = std::sqrt(residual_sq / static_cast<double>(n));
        inliers = n;
        // Skip clipping once residuals are numerically negligible (sub-µm in
        // elevation terms); clipping at 2σ of float rounding noise would
        // discard perfectly good samples.
        if (round + 1 < rounds && sigma > 1e-6) {
            const double threshold = sigma_clip * sigma;
            for (std::size_t i = 0; i < x.size(); ++i) {
                if (!keep[i]) {
                    continue;
                }
                const double r = std::abs(static_cast<double>(y[i]) -
                    (a * static_cast<double>(x[i]) + b));
                if (r > threshold) {
                    keep[i] = false;
                }
            }
        }
    }
    fit.ok = true;
    fit.a = a;
    fit.b = b;
    fit.sigma = sigma;
    fit.inliers = inliers;
    return fit;
}

cfg::StrategyRegistry<ElevationEstimator>& estimator_registry() {
    static cfg::StrategyRegistry<ElevationEstimator> registry;
    static const bool registered = [] {
        registry.register_factory("dem_fusion", [] {
            return std::unique_ptr<ElevationEstimator>(new DemFusionEstimator());
        });
        registry.register_factory("synthetic_detail", [] {
            return std::unique_ptr<ElevationEstimator>(new SyntheticDetailEstimator());
        });
        registry.register_factory("mono_depth_onnx", [] {
            return std::unique_ptr<ElevationEstimator>(new MonoDepthOnnxEstimator());
        });
        return true;
    }();
    (void)registered;
    return registry;
}

} // namespace agbot::terrain
