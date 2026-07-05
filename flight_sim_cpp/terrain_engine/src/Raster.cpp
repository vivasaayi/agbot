#include "agbot_terrain/Raster.hpp"

#include <algorithm>
#include <cstring>

namespace agbot::terrain {

Raster Raster::filled(int width, int height, const GeoBounds& bounds, float value) {
    Raster raster;
    raster.width = width;
    raster.height = height;
    raster.bounds = bounds;
    raster.values.assign(
        static_cast<std::size_t>(width) * static_cast<std::size_t>(height), value);
    return raster;
}

std::optional<float> Raster::sample_at(double latitude, double longitude) const {
    if (!valid()) {
        return std::nullopt;
    }
    const double lat_span = bounds.max_latitude - bounds.min_latitude;
    const double lon_span = bounds.max_longitude - bounds.min_longitude;
    if (lat_span <= 0.0 || lon_span <= 0.0) {
        return std::nullopt;
    }
    if (latitude < bounds.min_latitude || latitude > bounds.max_latitude ||
        longitude < bounds.min_longitude || longitude > bounds.max_longitude) {
        return std::nullopt;
    }

    // Row 0 = north edge; cell centers at grid corners (resolution-1 spans).
    const double u = (longitude - bounds.min_longitude) / lon_span;
    const double v = (bounds.max_latitude - latitude) / lat_span;
    const double fx = u * static_cast<double>(width - 1);
    const double fy = v * static_cast<double>(height - 1);
    const int x0 = std::clamp(static_cast<int>(fx), 0, width - 1);
    const int y0 = std::clamp(static_cast<int>(fy), 0, height - 1);
    const int x1 = std::min(x0 + 1, width - 1);
    const int y1 = std::min(y0 + 1, height - 1);
    const double tx = fx - static_cast<double>(x0);
    const double ty = fy - static_cast<double>(y0);

    const float v00 = at(y0, x0);
    const float v10 = at(y0, x1);
    const float v01 = at(y1, x0);
    const float v11 = at(y1, x1);

    double weight_sum = 0.0;
    double value_sum = 0.0;
    const auto accumulate = [&](float value, double weight) {
        if (!is_nodata(value) && weight > 0.0) {
            weight_sum += weight;
            value_sum += static_cast<double>(value) * weight;
        }
    };
    accumulate(v00, (1.0 - tx) * (1.0 - ty));
    accumulate(v10, tx * (1.0 - ty));
    accumulate(v01, (1.0 - tx) * ty);
    accumulate(v11, tx * ty);
    if (weight_sum <= 0.0) {
        // Exact corner hits with zero-weight neighbors still count.
        if (tx == 0.0 && ty == 0.0 && !is_nodata(v00)) {
            return v00;
        }
        return std::nullopt;
    }
    return static_cast<float>(value_sum / weight_sum);
}

std::uint64_t raster_hash(const Raster& raster) {
    std::uint64_t hash = 1469598103934665603ULL; // FNV1a-64 offset basis
    const auto mix_bytes = [&hash](const void* data, std::size_t size) {
        const auto* bytes = static_cast<const std::uint8_t*>(data);
        for (std::size_t i = 0; i < size; ++i) {
            hash ^= bytes[i];
            hash *= 1099511628211ULL;
        }
    };
    const std::int32_t dims[2] = { raster.width, raster.height };
    mix_bytes(dims, sizeof(dims));
    mix_bytes(raster.values.data(), raster.values.size() * sizeof(float));
    return hash;
}

int ImageryBundle::resolved_resolution() const {
    if (grid_width > 0 && grid_width == grid_height) {
        return grid_width;
    }
    const double span_m = std::max(aoi.width_m(), aoi.height_m());
    const double gsd = target_gsd_m > 0.0 ? target_gsd_m : 10.0;
    const int resolution = static_cast<int>(span_m / gsd) + 1;
    return std::clamp(resolution, 16, 1024);
}

} // namespace agbot::terrain
