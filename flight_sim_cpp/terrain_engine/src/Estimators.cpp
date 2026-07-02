#include "agbot_terrain/ElevationEstimator.hpp"

#include "agbot_terrain/Fusion.hpp"
#include "agbot_terrain/Png.hpp"

#include <algorithm>
#include <cmath>
#include <filesystem>
#include <limits>
#include <sstream>

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
#error "mono_depth_onnx real implementation not yet wired; add OnnxMonoDepth here"
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
