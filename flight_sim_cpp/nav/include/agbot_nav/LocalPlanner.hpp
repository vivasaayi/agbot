#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_nav/NavTypes.hpp"
#include "agbot_vehicles/ParamRegistry.hpp"
#include "agbot_vehicles/VehicleTypes.hpp"

#include <memory>
#include <string>
#include <vector>

namespace agbot::nav {

struct LocalPlan {
    bool ok = false;
    std::string reason;
    double v_cmd = 0.0;      // longitudinal speed target (m/s)
    double steer_cmd = 0.0;  // front wheel angle target (rad)
    Trajectory trajectory;   // predicted rollout in the world frame
};

class ILocalPlanner {
public:
    virtual ~ILocalPlanner() = default;
    // tracked_objects are dynamic obstacles with constant-velocity estimates
    // (from an ITracker); planners predict each object forward along its
    // velocity and cost/avoid the predicted positions. The default empty
    // list keeps the static-world behavior bit-identical.
    virtual LocalPlan compute(
        const Costmap& costmap,
        const Path& global_path,
        const agbot::vehicles::EntityState& state,
        const agbot::vehicles::VehicleLimits& limits,
        const Vec3& goal,
        const std::vector<TrackedObject>& tracked_objects = {}) = 0;
    [[nodiscard]] virtual std::string name() const = 0;
};

// Pure pursuit on the global path with regulated speed: slows for curvature
// (curvature_gain) and obstacle proximity (costmap cost at the robot cell),
// and decelerates toward the goal. Params: lookahead_m, cruise_speed_mps,
// curvature_gain, obstacle_slow_start (cost 0..254 where slowing begins),
// goal_slow_gain, min_speed_mps, horizon_s, rollout_dt_s.
//
// Dynamic-obstacle guard (simpler than the sampling planners' predictive
// critic): the robot's straight-ahead constant-speed motion and every
// tracked object's constant-velocity extrapolation are sampled over
// max_prediction_s; when the minimum predicted surface separation
// (center distance minus robot_radius_m + object radius) drops below
// stop_distance_m, the commanded speed scales linearly toward a full stop
// (zero at predicted contact). Params: robot_radius_m, stop_distance_m,
// max_prediction_s.
class PurePursuitPlanner final : public ILocalPlanner {
public:
    PurePursuitPlanner() = default;
    explicit PurePursuitPlanner(const agbot::config::ParamTable& params);

    LocalPlan compute(
        const Costmap& costmap,
        const Path& global_path,
        const agbot::vehicles::EntityState& state,
        const agbot::vehicles::VehicleLimits& limits,
        const Vec3& goal,
        const std::vector<TrackedObject>& tracked_objects = {}) override;

    [[nodiscard]] std::string name() const override { return "pure_pursuit"; }

private:
    double lookahead_m_ = 3.0;
    double cruise_speed_mps_ = 3.0;
    double curvature_gain_ = 1.5;
    double obstacle_slow_start_ = 80.0;
    double goal_slow_gain_ = 0.8;
    double min_speed_mps_ = 0.4;
    double horizon_s_ = 1.5;
    double rollout_dt_s_ = 0.1;
    double robot_radius_m_ = 0.5;
    double stop_distance_m_ = 2.0;
    double max_prediction_s_ = 3.0;
};

// Dynamic-window planner: deterministic (v, steer) sampling grid, kinematic
// bicycle rollouts, weighted critics (obstacle cost, path alignment, goal
// distance, speed). Rollouts touching a lethal cell are rejected. Params:
// v_samples, steer_samples, horizon_s, rollout_dt_s, cruise_speed_mps,
// lethal_threshold, w_obstacle, w_path, w_goal, w_speed, min_speed_mps,
// goal_slow_gain.
//
// Dynamic-obstacle critic: at rollout step t each tracked object is
// extrapolated to position + velocity * min(t, max_prediction_s); rollouts
// entering the hard margin (robot_radius_m + object radius +
// dynamic_margin_m) are rejected like lethal cells, and surviving rollouts
// pay w_dynamic * exp(-d^2 / (2 * dynamic_sigma_m^2)) on the closest
// predicted approach. Params: w_dynamic, dynamic_margin_m, dynamic_sigma_m,
// max_prediction_s, robot_radius_m.
class DwaPlanner final : public ILocalPlanner {
public:
    DwaPlanner() = default;
    explicit DwaPlanner(const agbot::config::ParamTable& params);

    LocalPlan compute(
        const Costmap& costmap,
        const Path& global_path,
        const agbot::vehicles::EntityState& state,
        const agbot::vehicles::VehicleLimits& limits,
        const Vec3& goal,
        const std::vector<TrackedObject>& tracked_objects = {}) override;

    [[nodiscard]] std::string name() const override { return "dwa"; }

private:
    int v_samples_ = 5;
    int steer_samples_ = 9;
    double horizon_s_ = 1.6;
    double rollout_dt_s_ = 0.1;
    double cruise_speed_mps_ = 3.0;
    std::uint8_t lethal_threshold_ = 200;
    double w_obstacle_ = 2.0;
    double w_path_ = 1.2;
    double w_goal_ = 0.6;
    double w_speed_ = 0.4;
    double min_speed_mps_ = 0.3;
    double goal_slow_gain_ = 0.8;
    double w_dynamic_ = 3.0;
    double dynamic_margin_m_ = 0.3;
    double dynamic_sigma_m_ = 1.2;
    double max_prediction_s_ = 3.0;
    double robot_radius_m_ = 0.5;
};

using LocalPlannerRegistry = agbot::vehicles::ParamRegistry<ILocalPlanner>;
[[nodiscard]] const LocalPlannerRegistry& default_local_planner_registry();

} // namespace agbot::nav
