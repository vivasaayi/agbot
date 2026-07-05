#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_nav/NavTypes.hpp"
#include "agbot_vehicles/ParamRegistry.hpp"

#include <array>
#include <cstdint>
#include <string>
#include <vector>

namespace agbot::nav {

// One per-frame dynamic-obstacle detection: the XZ centroid of a cluster of
// dynamic-class sensor points.
struct Detection {
    Vec3 position;                       // y = 0
    double radius_m = 0.3;               // max cluster extent from centroid
    std::uint32_t class_id = kClassObstacle;
    std::uint32_t object_id = 0;         // sensor object id (0 in fallback)
    std::size_t point_count = 0;
};

// Cluster the dynamic-class points (pedestrian/vehicle) of a sensor cloud
// into detections. use_object_ids = true groups by the sensor's per-point
// object id (ground-truth association path, requires cloud.object_ids);
// use_object_ids = false runs a deterministic greedy distance clustering
// (points join the first cluster whose running centroid is within
// cluster_distance_m, in cloud order), exercising the tracker realistically.
// Detections are ordered by object id / cluster creation order.
[[nodiscard]] std::vector<Detection> cluster_dynamic_detections(
    const PointCloud& cloud,
    bool use_object_ids,
    double cluster_distance_m);

class ITracker {
public:
    virtual ~ITracker() = default;
    // Advance the tracker to time_s with this frame's detections; returns the
    // confirmed tracks (age >= min_hits equivalent for the strategy).
    virtual std::vector<TrackedObject> update(
        const std::vector<Detection>& detections, double time_s) = 0;
    [[nodiscard]] virtual std::string name() const = 0;
};

// Greedy nearest-neighbor multi-object tracker with one 4-state linear
// Kalman filter (x, z, vx, vz; constant-velocity model) per track.
// Association: all track/detection pairs within gate_m are sorted by
// distance (ties by track id, then detection index) and assigned greedily.
// Track lifecycle: born tentative on an unassigned detection, reported once
// it has min_hits total hits, coasted on a miss (predict only), dropped
// after max_missed consecutive misses. Fully deterministic.
//
// Params: gate_m, min_hits, max_missed, q_pos (position process noise,
// m^2/s), q_vel (velocity process noise, (m/s)^2/s), r_pos (measurement
// sigma, m), init_vel_sigma (initial velocity sigma, m/s).
class GreedyNnTracker final : public ITracker {
public:
    GreedyNnTracker() = default;
    explicit GreedyNnTracker(const agbot::config::ParamTable& params);

    std::vector<TrackedObject> update(
        const std::vector<Detection>& detections, double time_s) override;

    [[nodiscard]] std::string name() const override { return "greedy_nn"; }

private:
    struct Track {
        std::uint32_t id = 0;
        std::array<double, 4> x{};   // px, pz, vx, vz
        std::array<double, 16> P{};  // row-major 4x4 covariance
        double radius_m = 0.3;
        std::uint32_t class_id = kClassObstacle;
        int hits = 0;
        int missed = 0;
        int age = 0;
    };

    double gate_m_ = 2.5;
    int min_hits_ = 2;
    int max_missed_ = 5;
    double q_pos_ = 0.3;
    double q_vel_ = 2.0;
    double r_pos_ = 0.2;
    double init_vel_sigma_ = 3.0;

    std::vector<Track> tracks_;
    std::uint32_t next_id_ = 1;
    double last_time_s_ = 0.0;
    bool has_time_ = false;
};

using TrackerRegistry = agbot::vehicles::ParamRegistry<ITracker>;
[[nodiscard]] const TrackerRegistry& default_tracker_registry();

} // namespace agbot::nav
