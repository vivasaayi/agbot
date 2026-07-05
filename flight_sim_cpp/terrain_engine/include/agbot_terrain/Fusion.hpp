#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_terrain/Raster.hpp"

#include <string>
#include <vector>

namespace agbot::terrain {

// One fusion input: an estimated heightfield plus its configured weight,
// ordered as the [[layer]] entries appear in configuration.
struct FusionLayer {
    HeightField field;
    double weight = 1.0;
};

struct FusionResult {
    bool ok = false;
    std::string error; // e.g. "fusion_no_layers", "fusion_grid_mismatch"
    HeightField field;
};

// Separable low-pass filters used by detail injection and DEM smoothing.
// Nodata cells are excluded from the kernel (weight-normalized).
[[nodiscard]] Raster box_blur(const Raster& input, int radius);
[[nodiscard]] Raster gaussian_blur(const Raster& input, double sigma);

class FusionEngine {
public:
    // Base layer wins wherever it has data; later layers only fill voids.
    [[nodiscard]] static FusionResult dem_locked(const std::vector<FusionLayer>& layers);

    // Per-cell weighted mean with weight = layer.weight * confidence(cell).
    [[nodiscard]] static FusionResult confidence_weighted(const std::vector<FusionLayer>& layers);

    // h = lowpass(dem) + lambda * sum_i weight_i * highpass(detail_i)
    // where highpass(x) = x - lowpass(x); lowpass is a separable box blur of
    // radius cutoff_cells applied three times (Gaussian approximation).
    [[nodiscard]] static FusionResult detail_injection(
        const std::vector<FusionLayer>& layers,
        double lambda,
        int cutoff_cells);

    // Dispatch by method name using [fusion] params (lambda, cutoff_cells).
    [[nodiscard]] static FusionResult fuse(
        const std::string& method,
        const std::vector<FusionLayer>& layers,
        const agbot::config::ParamTable& params);
};

} // namespace agbot::terrain
