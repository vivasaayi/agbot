#pragma once

#include "agbot_flight_sim/GeoTerrain.hpp"

#include <cmath>
#include <cstdint>
#include <limits>
#include <optional>
#include <string>
#include <vector>

namespace agbot::terrain {

using agbot::flight_sim::GeoBounds;

// A georeferenced single-band float grid. Row 0 is the northernmost row
// (matches the ElevationComposite heightmap orientation). Cells without data
// hold quiet NaN (`Raster::nodata()`).
struct Raster {
    int width = 0;
    int height = 0;
    GeoBounds bounds;
    std::vector<float> values;

    [[nodiscard]] static float nodata() { return std::numeric_limits<float>::quiet_NaN(); }
    [[nodiscard]] static bool is_nodata(float value) { return std::isnan(value); }

    [[nodiscard]] static Raster filled(int width, int height, const GeoBounds& bounds, float value);

    [[nodiscard]] bool valid() const {
        return width > 0 && height > 0 &&
            values.size() == static_cast<std::size_t>(width) * static_cast<std::size_t>(height);
    }

    [[nodiscard]] float at(int row, int col) const {
        return values[static_cast<std::size_t>(row) * static_cast<std::size_t>(width) +
                      static_cast<std::size_t>(col)];
    }

    void set(int row, int col, float value) {
        values[static_cast<std::size_t>(row) * static_cast<std::size_t>(width) +
               static_cast<std::size_t>(col)] = value;
    }

    // Bilinear sample in geographic coordinates. Returns nullopt outside the
    // bounds or when every contributing cell is nodata.
    [[nodiscard]] std::optional<float> sample_at(double latitude, double longitude) const;
};

// FNV1a-64 over the raster payload bytes plus dimensions; used for
// determinism assertions (same config + inputs => identical hash).
[[nodiscard]] std::uint64_t raster_hash(const Raster& raster);

// An elevation estimate plus per-cell confidence in [0, 1].
struct HeightField {
    Raster elevation;
    Raster confidence;
    std::string source_algorithm;

    [[nodiscard]] bool valid() const {
        return elevation.valid() && confidence.valid() &&
            elevation.width == confidence.width && elevation.height == confidence.height;
    }
};

// Everything an elevation estimator may consume for one area of interest.
struct ImageryBundle {
    GeoBounds aoi;
    double target_gsd_m = 10.0;
    // Optional explicit output grid; when zero the grid is derived from the
    // AOI extent and target_gsd_m (clamped to a sane square resolution).
    int grid_width = 0;
    int grid_height = 0;
    std::optional<HeightField> dem_prior;
    // Optional RGB imagery for learned estimators (e.g. mono_depth_onnx).
    // Path to a PNG covering the AOI, decoded via the module PNG decoder.
    std::string rgb_image_path;

    // Resolved square grid resolution for estimators that produce AOI grids.
    [[nodiscard]] int resolved_resolution() const;
};

} // namespace agbot::terrain
