#pragma once

#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_flight_sim/SceneSynthesis.hpp"
#include "agbot_flight_sim/Vec3.hpp"

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

namespace agbot::flight_sim {

struct RayTracedCameraConfig {
    int width = 16;
    int height = 12;
    double horizontal_fov_deg = 60.0;
    double vertical_fov_deg = 45.0;
    double max_range_m = 250.0;
};

struct RayTracedPixel {
    int x = 0;
    int y = 0;
    double depth_m = 0.0;
    Vec3 world_position_m;
    std::string class_name;
    std::string object_id;
    std::uint64_t object_seed = 0;
};

struct RayTracedFrame {
    std::string status;
    std::string reason;
    int width = 0;
    int height = 0;
    double timestamp_s = 0.0;
    Vec3 pose;
    std::string scene_hash;
    std::vector<RayTracedPixel> pixels;
    std::string frame_hash;

    [[nodiscard]] const RayTracedPixel& pixel_at(int x, int y) const;
    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] RayTracedFrame raytrace_camera_frame(
    const DroneState& state,
    const TerrainProfile& profile,
    const SceneSynthesisManifest& scene,
    RayTracedCameraConfig config = {});

} // namespace agbot::flight_sim
