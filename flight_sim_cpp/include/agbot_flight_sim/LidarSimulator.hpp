#pragma once

#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/GeoTerrain.hpp"

#include <cstdint>
#include <string>
#include <vector>

namespace agbot::flight_sim {

struct LidarRaycastConfig {
    bool enabled = true;
    std::string profile_name = "sim_lidar_a3";
    std::string sensor_id = "sim_lidar_a3";
    std::uint32_t horizontal_samples = 36;
    std::uint32_t vertical_samples = 3;
    double vertical_fov_deg = 60.0;
    double max_range_m = 80.0;
    double range_noise_m = 0.0;
    std::uint8_t min_quality = 10;
    std::uint8_t max_quality = 100;
};

struct LidarPoint {
    std::string timestamp;
    double angle_deg = 0.0;
    double distance_mm = 0.0;
    std::uint8_t quality = 0;
    double range_m = 0.0;
    Vec3 position_m;
    Vec3 direction;
    std::uint32_t ring = 0;
    std::uint32_t azimuth_index = 0;

    [[nodiscard]] std::string to_json() const;
};

struct LidarScan {
    std::string timestamp;
    std::string scan_id;
    std::string sensor_id;
    std::string frame_id = "body_ned";
    std::string status = "ok";
    std::uint64_t seed = 0;
    std::uint64_t step = 0;
    Vec3 sensor_position_m;
    std::vector<LidarPoint> points;

    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] LidarScan raycast_lidar_scan(
    const DroneState& state,
    const TerrainMesh& terrain,
    const LidarRaycastConfig& config,
    std::uint64_t seed,
    std::uint64_t step);

[[nodiscard]] TerrainMesh build_lidar_flat_terrain_for_mission(
    const Mission& mission,
    int resolution = 32,
    double padding_m = 40.0);

[[nodiscard]] std::string lidar_config_json(const LidarRaycastConfig& config);

} // namespace agbot::flight_sim
