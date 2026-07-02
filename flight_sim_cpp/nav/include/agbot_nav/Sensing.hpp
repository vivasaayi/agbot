#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_nav/NavTypes.hpp"
#include "agbot_vehicles/ParamRegistry.hpp"
#include "agbot_vehicles/VehicleTypes.hpp"

#include <memory>
#include <string>
#include <vector>

namespace agbot::nav {

struct SensorFrame {
    std::string status = "ok";
    std::string reason;
    double stamp_s = 0.0;
    int width = 0;
    int height = 0;
    // Row-major per-pixel range in meters; <= 0 means no return (dropout or
    // beyond max range).
    std::vector<double> depth_m;
    // World-frame point cloud of valid returns.
    PointCloud cloud;
};

class ISensor {
public:
    virtual ~ISensor() = default;
    virtual SensorFrame sense(
        const NavWorld& world,
        const agbot::vehicles::EntityState& state,
        double time_s) = 0;
    [[nodiscard]] virtual std::string name() const = 0;
};

// Forward-facing depth camera ray-traced against the NavWorld: exact
// ray-vs-prism intersection for every SceneObject footprint (extruded to its
// height), exact ray-vs-cylinder intersection for every DynamicAgent
// (classes kClassPedestrian/kClassVehicle, object ids
// kDynamicObjectIdBase + agent id, distinct from static prism ids), plus the
// flat ground plane. This is the ground-vehicle adaptation of
// agbot::flight_sim::raytrace_camera_frame, whose published API is a nadir
// footprint sampler and cannot express a yawed/pitched forward mount.
//
// Params: width, height, horizontal_fov_deg, vertical_fov_deg, max_range_m,
// mount_yaw_rad, mount_pitch_rad (positive pitches down), mount_height_m,
// range_noise_a (sigma = a * z^2, deterministic seeded RNG), dropout_pct,
// seed.
class DepthCameraSensor final : public ISensor {
public:
    DepthCameraSensor() = default;
    explicit DepthCameraSensor(const agbot::config::ParamTable& params);

    SensorFrame sense(
        const NavWorld& world,
        const agbot::vehicles::EntityState& state,
        double time_s) override;

    [[nodiscard]] std::string name() const override { return "depth_camera"; }

private:
    int width_ = 32;
    int height_ = 24;
    double horizontal_fov_deg_ = 90.0;
    double vertical_fov_deg_ = 60.0;
    double max_range_m_ = 40.0;
    double mount_yaw_rad_ = 0.0;
    double mount_pitch_rad_ = 0.0;
    double mount_height_m_ = 0.5;
    double range_noise_a_ = 0.0;
    double dropout_pct_ = 0.0;
    std::uint64_t seed_ = 1;
    std::uint64_t frame_index_ = 0;
};

using SensorRegistry = agbot::vehicles::ParamRegistry<ISensor>;
[[nodiscard]] const SensorRegistry& default_sensor_registry();

} // namespace agbot::nav
