#pragma once

#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/GeoTerrain.hpp"

#include <array>
#include <optional>
#include <string>
#include <vector>

namespace agbot::flight_sim {

struct MultispectralCameraConfig {
    int width = 16;
    int height = 12;
    double horizontal_fov_deg = 60.0;
    double vertical_fov_deg = 45.0;
    double minimum_footprint_m = 4.0;
    std::vector<std::string> bands {"Red", "Green", "Blue", "NIR"};
};

struct MultispectralSpatialRef {
    bool georeferenced = false;
    std::string crs = "EPSG:4326";
    GeoBounds extent;
    std::array<double, 6> geo_transform {};
    double resolution_x = 0.0;
    double resolution_y = 0.0;

    [[nodiscard]] std::optional<GeoCoordinate> coordinate_for_pixel(double column, double row) const;
    [[nodiscard]] std::optional<std::array<double, 2>> pixel_for_coordinate(
        const GeoCoordinate& coordinate) const;
};

struct MultispectralBandImage {
    std::string name;
    int width = 0;
    int height = 0;
    std::vector<float> reflectance;
    MultispectralSpatialRef spatial_ref;
    float min_reflectance = 0.0f;
    float max_reflectance = 0.0f;

    [[nodiscard]] float sample(int column, int row) const;
};

struct MultispectralCapture {
    std::string status = "ok";
    std::string reason;
    GeoCoordinate center;
    MultispectralSpatialRef spatial_ref;
    std::vector<MultispectralBandImage> bands;

    [[nodiscard]] bool ok() const;
    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] MultispectralCapture capture_multispectral_bands(
    const DroneState& camera_state,
    const Mission& mission,
    const ElevationComposite& terrain,
    const MultispectralCameraConfig& config = {});

} // namespace agbot::flight_sim
