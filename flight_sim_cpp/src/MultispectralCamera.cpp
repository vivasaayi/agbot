#include "agbot_flight_sim/MultispectralCamera.hpp"

#include <algorithm>
#include <cmath>
#include <iomanip>
#include <limits>
#include <sstream>
#include <string_view>
#include <utility>

namespace agbot::flight_sim {
namespace {

constexpr double kPi = 3.14159265358979323846;

double deg_to_rad(double degrees) {
    return degrees * kPi / 180.0;
}

double clamp01(double value) {
    return std::clamp(value, 0.0, 1.0);
}

std::string escape_json(std::string_view value) {
    std::ostringstream output;
    for (const char c : value) {
        switch (c) {
            case '"':
                output << "\\\"";
                break;
            case '\\':
                output << "\\\\";
                break;
            case '\n':
                output << "\\n";
                break;
            case '\r':
                output << "\\r";
                break;
            case '\t':
                output << "\\t";
                break;
            default:
                output << c;
                break;
        }
    }
    return output.str();
}

MultispectralCapture no_coverage(GeoCoordinate center, std::string reason) {
    MultispectralCapture capture;
    capture.status = "no_coverage";
    capture.reason = std::move(reason);
    capture.center = center;
    return capture;
}

GeoBounds bounds_for_camera_footprint(
    const GeoCoordinate& center,
    const GeoCoordinate& origin,
    double half_width_m,
    double half_height_m) {
    const Vec3 center_local = local_from_geo(center, origin);
    const std::array<GeoCoordinate, 4> corners {
        geo_from_local({center_local.x - half_width_m, center_local.y, center_local.z - half_height_m}, origin),
        geo_from_local({center_local.x + half_width_m, center_local.y, center_local.z - half_height_m}, origin),
        geo_from_local({center_local.x - half_width_m, center_local.y, center_local.z + half_height_m}, origin),
        geo_from_local({center_local.x + half_width_m, center_local.y, center_local.z + half_height_m}, origin),
    };

    GeoBounds bounds {
        std::numeric_limits<double>::infinity(),
        std::numeric_limits<double>::infinity(),
        -std::numeric_limits<double>::infinity(),
        -std::numeric_limits<double>::infinity(),
    };
    for (const GeoCoordinate& corner : corners) {
        bounds.min_latitude = std::min(bounds.min_latitude, corner.latitude);
        bounds.max_latitude = std::max(bounds.max_latitude, corner.latitude);
        bounds.min_longitude = std::min(bounds.min_longitude, corner.longitude);
        bounds.max_longitude = std::max(bounds.max_longitude, corner.longitude);
    }
    return bounds;
}

MultispectralSpatialRef spatial_ref_for_bounds(const GeoBounds& bounds, int width, int height) {
    MultispectralSpatialRef spatial_ref;
    spatial_ref.georeferenced = width > 0 && height > 0
        && bounds.max_longitude > bounds.min_longitude
        && bounds.max_latitude > bounds.min_latitude;
    spatial_ref.extent = bounds;
    if (!spatial_ref.georeferenced) {
        return spatial_ref;
    }

    spatial_ref.resolution_x = (bounds.max_longitude - bounds.min_longitude) / static_cast<double>(width);
    spatial_ref.resolution_y = (bounds.max_latitude - bounds.min_latitude) / static_cast<double>(height);
    spatial_ref.geo_transform = {
        bounds.min_longitude,
        spatial_ref.resolution_x,
        0.0,
        bounds.max_latitude,
        0.0,
        -spatial_ref.resolution_y,
    };
    return spatial_ref;
}

double vegetation_index_pattern(double u, double v, double altitude_m) {
    const double row_crop = std::sin((u * 12.0 + altitude_m * 0.005) * kPi);
    const double canopy = std::cos((v * 7.0 - altitude_m * 0.003) * kPi);
    return clamp01(0.55 + 0.25 * row_crop + 0.15 * canopy);
}

float band_reflectance(std::string_view band, double vegetation, double u, double v) {
    const double soil = 1.0 - vegetation;
    double value = 0.0;
    if (band == "NIR") {
        value = 0.42 + vegetation * 0.45 + soil * 0.04;
    } else if (band == "Green") {
        value = 0.18 + vegetation * 0.20 + soil * 0.08;
    } else if (band == "Blue") {
        value = 0.06 + vegetation * 0.06 + soil * 0.05;
    } else {
        value = 0.15 + vegetation * 0.08 + soil * 0.18;
    }
    value += 0.015 * std::sin((u + v) * 2.0 * kPi);
    return static_cast<float>(clamp01(value));
}

MultispectralBandImage build_band(
    std::string name,
    const MultispectralSpatialRef& spatial_ref,
    const MultispectralCameraConfig& config,
    double altitude_m) {
    MultispectralBandImage image;
    image.name = std::move(name);
    image.width = config.width;
    image.height = config.height;
    image.spatial_ref = spatial_ref;
    image.reflectance.reserve(static_cast<std::size_t>(config.width * config.height));
    image.min_reflectance = std::numeric_limits<float>::infinity();
    image.max_reflectance = -std::numeric_limits<float>::infinity();

    for (int row = 0; row < config.height; ++row) {
        for (int column = 0; column < config.width; ++column) {
            const double u = (static_cast<double>(column) + 0.5) / static_cast<double>(config.width);
            const double v = (static_cast<double>(row) + 0.5) / static_cast<double>(config.height);
            const double vegetation = vegetation_index_pattern(u, v, altitude_m);
            const float value = band_reflectance(image.name, vegetation, u, v);
            image.reflectance.push_back(value);
            image.min_reflectance = std::min(image.min_reflectance, value);
            image.max_reflectance = std::max(image.max_reflectance, value);
        }
    }
    return image;
}

} // namespace

std::optional<GeoCoordinate> MultispectralSpatialRef::coordinate_for_pixel(double column, double row) const {
    if (!georeferenced || resolution_x <= 0.0 || resolution_y <= 0.0) {
        return std::nullopt;
    }
    return GeoCoordinate {
        geo_transform[3] + (row + 0.5) * geo_transform[5],
        geo_transform[0] + (column + 0.5) * geo_transform[1],
        0.0,
    };
}

std::optional<std::array<double, 2>> MultispectralSpatialRef::pixel_for_coordinate(
    const GeoCoordinate& coordinate) const {
    if (!georeferenced || resolution_x <= 0.0 || resolution_y <= 0.0) {
        return std::nullopt;
    }
    if (coordinate.longitude < extent.min_longitude || coordinate.longitude > extent.max_longitude
        || coordinate.latitude < extent.min_latitude || coordinate.latitude > extent.max_latitude) {
        return std::nullopt;
    }
    return std::array<double, 2> {
        ((coordinate.longitude - geo_transform[0]) / geo_transform[1]) - 0.5,
        ((coordinate.latitude - geo_transform[3]) / geo_transform[5]) - 0.5,
    };
}

float MultispectralBandImage::sample(int column, int row) const {
    if (column < 0 || row < 0 || column >= width || row >= height) {
        return 0.0f;
    }
    return reflectance[static_cast<std::size_t>(row * width + column)];
}

bool MultispectralCapture::ok() const {
    return status == "ok";
}

std::string MultispectralCapture::to_json() const {
    std::ostringstream output;
    output << std::fixed << std::setprecision(6)
           << "{\"status\":\"" << escape_json(status) << "\""
           << ",\"reason\":\"" << escape_json(reason) << "\""
           << ",\"center\":{\"latitude\":" << center.latitude
           << ",\"longitude\":" << center.longitude
           << ",\"altitude\":" << center.altitude_m << "}"
           << ",\"spatial_ref\":{\"georeferenced\":" << (spatial_ref.georeferenced ? "true" : "false")
           << ",\"crs\":\"" << escape_json(spatial_ref.crs) << "\""
           << ",\"bbox\":{\"min_lon\":" << spatial_ref.extent.min_longitude
           << ",\"min_lat\":" << spatial_ref.extent.min_latitude
           << ",\"max_lon\":" << spatial_ref.extent.max_longitude
           << ",\"max_lat\":" << spatial_ref.extent.max_latitude << "}"
           << ",\"geo_transform\":[";
    for (std::size_t index = 0; index < spatial_ref.geo_transform.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        output << spatial_ref.geo_transform[index];
    }
    output << "],\"resolution\":{\"x\":" << spatial_ref.resolution_x
           << ",\"y\":" << spatial_ref.resolution_y << "}},\"bands\":[";
    for (std::size_t index = 0; index < bands.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        const auto& band = bands[index];
        output << "{\"name\":\"" << escape_json(band.name) << "\""
               << ",\"width\":" << band.width
               << ",\"height\":" << band.height
               << ",\"min_reflectance\":" << band.min_reflectance
               << ",\"max_reflectance\":" << band.max_reflectance << "}";
    }
    output << "]}";
    return output.str();
}

MultispectralCapture capture_multispectral_bands(
    const DroneState& camera_state,
    const Mission& mission,
    const ElevationComposite& terrain,
    const MultispectralCameraConfig& config) {
    if (config.width <= 0 || config.height <= 0 || config.bands.empty()) {
        return no_coverage({}, "invalid multispectral camera configuration");
    }
    if (!mission.home_geo.has_value()) {
        return no_coverage({}, "mission has no georeferenced origin");
    }

    const GeoCoordinate center = geo_from_local(camera_state.position, *mission.home_geo);
    const auto terrain_sample = terrain.sample_at(center);
    if (!terrain_sample.has_value() || terrain_sample->state == TerrainTileState::Missing) {
        return no_coverage(center, "camera pose is outside terrain tile coverage");
    }

    const double altitude_agl_m = std::max(1.0, camera_state.position.y - static_cast<double>(terrain_sample->elevation_m));
    const double half_width_m = std::max(
        config.minimum_footprint_m * 0.5,
        altitude_agl_m * std::tan(deg_to_rad(config.horizontal_fov_deg) * 0.5));
    const double half_height_m = std::max(
        config.minimum_footprint_m * 0.5,
        altitude_agl_m * std::tan(deg_to_rad(config.vertical_fov_deg) * 0.5));

    MultispectralCapture capture;
    capture.center = center;
    capture.spatial_ref = spatial_ref_for_bounds(
        bounds_for_camera_footprint(center, *mission.home_geo, half_width_m, half_height_m),
        config.width,
        config.height);
    if (!capture.spatial_ref.georeferenced) {
        return no_coverage(center, "camera footprint could not be georeferenced");
    }

    for (const std::string& band : config.bands) {
        capture.bands.push_back(build_band(band, capture.spatial_ref, config, altitude_agl_m));
    }
    return capture;
}

} // namespace agbot::flight_sim
