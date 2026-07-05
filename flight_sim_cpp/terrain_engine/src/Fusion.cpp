#include "agbot_terrain/Fusion.hpp"

#include <algorithm>
#include <cmath>

namespace agbot::terrain {
namespace {

bool grids_match(const std::vector<FusionLayer>& layers) {
    for (const FusionLayer& layer : layers) {
        if (!layer.field.valid() ||
            layer.field.elevation.width != layers.front().field.elevation.width ||
            layer.field.elevation.height != layers.front().field.elevation.height) {
            return false;
        }
    }
    return true;
}

// One separable box pass of the given radius; nodata excluded and preserved.
Raster box_blur_pass(const Raster& input, int radius) {
    Raster horizontal = input;
    for (int row = 0; row < input.height; ++row) {
        for (int col = 0; col < input.width; ++col) {
            double sum = 0.0;
            double count = 0.0;
            for (int k = -radius; k <= radius; ++k) {
                const int c = std::clamp(col + k, 0, input.width - 1);
                const float value = input.at(row, c);
                if (!Raster::is_nodata(value)) {
                    sum += static_cast<double>(value);
                    count += 1.0;
                }
            }
            horizontal.set(row, col,
                count > 0.0 ? static_cast<float>(sum / count) : Raster::nodata());
        }
    }
    Raster output = horizontal;
    for (int row = 0; row < input.height; ++row) {
        for (int col = 0; col < input.width; ++col) {
            double sum = 0.0;
            double count = 0.0;
            for (int k = -radius; k <= radius; ++k) {
                const int r = std::clamp(row + k, 0, input.height - 1);
                const float value = horizontal.at(r, col);
                if (!Raster::is_nodata(value)) {
                    sum += static_cast<double>(value);
                    count += 1.0;
                }
            }
            output.set(row, col,
                count > 0.0 ? static_cast<float>(sum / count) : Raster::nodata());
        }
    }
    return output;
}

Raster lowpass(const Raster& input, int cutoff_cells) {
    if (cutoff_cells <= 0) {
        return input;
    }
    // Three box passes approximate a Gaussian of sigma ~= cutoff_cells.
    Raster result = box_blur_pass(input, cutoff_cells);
    result = box_blur_pass(result, cutoff_cells);
    result = box_blur_pass(result, cutoff_cells);
    return result;
}

} // namespace

Raster box_blur(const Raster& input, int radius) {
    if (!input.valid() || radius <= 0) {
        return input;
    }
    return box_blur_pass(input, radius);
}

Raster gaussian_blur(const Raster& input, double sigma) {
    if (!input.valid() || sigma <= 0.0) {
        return input;
    }
    const int radius = std::max(1, static_cast<int>(std::lround(sigma)));
    Raster result = box_blur_pass(input, radius);
    result = box_blur_pass(result, radius);
    result = box_blur_pass(result, radius);
    return result;
}

FusionResult FusionEngine::dem_locked(const std::vector<FusionLayer>& layers) {
    FusionResult result;
    if (layers.empty()) {
        result.error = "fusion_no_layers";
        return result;
    }
    if (!grids_match(layers)) {
        result.error = "fusion_grid_mismatch";
        return result;
    }
    result.field = layers.front().field;
    result.field.source_algorithm = "fusion:dem_locked";
    for (std::size_t i = 0; i < result.field.elevation.values.size(); ++i) {
        if (!Raster::is_nodata(result.field.elevation.values[i])) {
            continue;
        }
        for (std::size_t layer = 1; layer < layers.size(); ++layer) {
            const float candidate = layers[layer].field.elevation.values[i];
            if (!Raster::is_nodata(candidate)) {
                result.field.elevation.values[i] = candidate;
                result.field.confidence.values[i] =
                    layers[layer].field.confidence.values[i] * 0.5f;
                break;
            }
        }
    }
    result.ok = true;
    return result;
}

FusionResult FusionEngine::confidence_weighted(const std::vector<FusionLayer>& layers) {
    FusionResult result;
    if (layers.empty()) {
        result.error = "fusion_no_layers";
        return result;
    }
    if (!grids_match(layers)) {
        result.error = "fusion_grid_mismatch";
        return result;
    }
    const Raster& reference = layers.front().field.elevation;
    result.field.elevation =
        Raster::filled(reference.width, reference.height, reference.bounds, Raster::nodata());
    result.field.confidence =
        Raster::filled(reference.width, reference.height, reference.bounds, 0.0f);
    result.field.source_algorithm = "fusion:confidence_weighted";
    for (std::size_t i = 0; i < reference.values.size(); ++i) {
        double weight_sum = 0.0;
        double value_sum = 0.0;
        double confidence_sum = 0.0;
        for (const FusionLayer& layer : layers) {
            const float value = layer.field.elevation.values[i];
            if (Raster::is_nodata(value)) {
                continue;
            }
            const double weight =
                layer.weight * static_cast<double>(layer.field.confidence.values[i]);
            if (weight <= 0.0) {
                continue;
            }
            weight_sum += weight;
            value_sum += weight * static_cast<double>(value);
            confidence_sum += weight * static_cast<double>(layer.field.confidence.values[i]);
        }
        if (weight_sum > 0.0) {
            result.field.elevation.values[i] = static_cast<float>(value_sum / weight_sum);
            result.field.confidence.values[i] = static_cast<float>(confidence_sum / weight_sum);
        }
    }
    result.ok = true;
    return result;
}

FusionResult FusionEngine::detail_injection(
    const std::vector<FusionLayer>& layers,
    double lambda,
    int cutoff_cells) {
    FusionResult result;
    if (layers.empty()) {
        result.error = "fusion_no_layers";
        return result;
    }
    if (!grids_match(layers)) {
        result.error = "fusion_grid_mismatch";
        return result;
    }
    const HeightField& base = layers.front().field;
    result.field.elevation = lowpass(base.elevation, cutoff_cells);
    result.field.confidence = base.confidence;
    result.field.source_algorithm = "fusion:detail_injection";
    for (std::size_t layer = 1; layer < layers.size(); ++layer) {
        const Raster& detail = layers[layer].field.elevation;
        const Raster detail_low = lowpass(detail, cutoff_cells);
        const double scale = lambda * layers[layer].weight;
        for (std::size_t i = 0; i < detail.values.size(); ++i) {
            const float detail_value = detail.values[i];
            const float detail_low_value = detail_low.values[i];
            float& fused = result.field.elevation.values[i];
            if (Raster::is_nodata(fused) || Raster::is_nodata(detail_value) ||
                Raster::is_nodata(detail_low_value)) {
                continue;
            }
            fused += static_cast<float>(
                scale * (static_cast<double>(detail_value) - static_cast<double>(detail_low_value)));
        }
    }
    result.ok = true;
    return result;
}

FusionResult FusionEngine::fuse(
    const std::string& method,
    const std::vector<FusionLayer>& layers,
    const agbot::config::ParamTable& params) {
    if (method == "dem_locked") {
        return dem_locked(layers);
    }
    if (method == "confidence_weighted") {
        return confidence_weighted(layers);
    }
    if (method == "detail_injection") {
        const double lambda = agbot::config::double_or(params, "lambda", 1.0);
        const int cutoff_cells =
            static_cast<int>(agbot::config::integer_or(params, "cutoff_cells", 4));
        return detail_injection(layers, lambda, cutoff_cells);
    }
    FusionResult result;
    result.error = "fusion_unknown_method:" + method;
    return result;
}

} // namespace agbot::terrain
