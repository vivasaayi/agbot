#include "agbot_nav/LocalPlanner.hpp"

#include "agbot_nav/MppiPlanner.hpp"

#include <algorithm>
#include <cmath>
#include <limits>

namespace agbot::nav {

namespace {

using agbot::vehicles::bicycle_propagate;
using agbot::vehicles::EntityState;

std::size_t nearest_path_index(const Path& path, const Vec3& position) {
    std::size_t best = 0;
    double best_distance = std::numeric_limits<double>::infinity();
    for (std::size_t i = 0; i < path.points.size(); ++i) {
        const double distance = (path.points[i] - position).horizontal_length();
        if (distance < best_distance) {
            best_distance = distance;
            best = i;
        }
    }
    return best;
}

double distance_to_path(const Path& path, const Vec3& position) {
    if (path.points.empty()) {
        return 0.0;
    }
    double best = std::numeric_limits<double>::infinity();
    for (std::size_t i = 1; i < path.points.size(); ++i) {
        const Vec3& a = path.points[i - 1];
        const Vec3& b = path.points[i];
        const double abx = b.x - a.x;
        const double abz = b.z - a.z;
        const double denom = abx * abx + abz * abz;
        double t = 0.0;
        if (denom > 1e-12) {
            t = ((position.x - a.x) * abx + (position.z - a.z) * abz) / denom;
            t = std::clamp(t, 0.0, 1.0);
        }
        const double px = a.x + abx * t;
        const double pz = a.z + abz * t;
        const double dx = position.x - px;
        const double dz = position.z - pz;
        best = std::min(best, std::sqrt(dx * dx + dz * dz));
    }
    return best;
}

double signed_speed(const EntityState& state) {
    return state.velocity.x * std::cos(state.yaw_rad)
        + state.velocity.z * std::sin(state.yaw_rad);
}

Trajectory rollout_trajectory(
    const EntityState& state,
    double speed,
    double steer,
    double wheelbase_m,
    double horizon_s,
    double dt_s) {
    Trajectory trajectory;
    EntityState rolled = state;
    double t = 0.0;
    trajectory.points.push_back({rolled.position, rolled.yaw_rad, speed, t});
    while (t + 1e-9 < horizon_s) {
        rolled = bicycle_propagate(rolled, speed, steer, wheelbase_m, dt_s);
        t += dt_s;
        trajectory.points.push_back({rolled.position, rolled.yaw_rad, speed, t});
    }
    return trajectory;
}

} // namespace

PurePursuitPlanner::PurePursuitPlanner(const agbot::config::ParamTable& params) {
    lookahead_m_ = agbot::config::double_or(params, "lookahead_m", lookahead_m_);
    cruise_speed_mps_ = agbot::config::double_or(params, "cruise_speed_mps", cruise_speed_mps_);
    curvature_gain_ = agbot::config::double_or(params, "curvature_gain", curvature_gain_);
    obstacle_slow_start_ =
        agbot::config::double_or(params, "obstacle_slow_start", obstacle_slow_start_);
    goal_slow_gain_ = agbot::config::double_or(params, "goal_slow_gain", goal_slow_gain_);
    min_speed_mps_ = agbot::config::double_or(params, "min_speed_mps", min_speed_mps_);
    horizon_s_ = agbot::config::double_or(params, "horizon_s", horizon_s_);
    rollout_dt_s_ = agbot::config::double_or(params, "rollout_dt_s", rollout_dt_s_);
    robot_radius_m_ = agbot::config::double_or(params, "robot_radius_m", robot_radius_m_);
    stop_distance_m_ = agbot::config::double_or(params, "stop_distance_m", stop_distance_m_);
    max_prediction_s_ = agbot::config::double_or(params, "max_prediction_s", max_prediction_s_);
}

LocalPlan PurePursuitPlanner::compute(
    const Costmap& costmap,
    const Path& global_path,
    const agbot::vehicles::EntityState& state,
    const agbot::vehicles::VehicleLimits& limits,
    const Vec3& goal,
    const std::vector<TrackedObject>& tracked_objects) {
    LocalPlan plan;
    if (global_path.points.empty()) {
        plan.reason = "empty_path";
        return plan;
    }

    // Lookahead target: walk forward from the nearest waypoint.
    const std::size_t nearest = nearest_path_index(global_path, state.position);
    Vec3 target = global_path.points.back();
    double accumulated = 0.0;
    Vec3 previous = state.position;
    for (std::size_t i = nearest; i < global_path.points.size(); ++i) {
        accumulated += (global_path.points[i] - previous).horizontal_length();
        previous = global_path.points[i];
        if (accumulated >= lookahead_m_) {
            target = global_path.points[i];
            break;
        }
    }

    const double dx = target.x - state.position.x;
    const double dz = target.z - state.position.z;
    const double target_distance = std::max(0.3, std::sqrt(dx * dx + dz * dz));
    const double alpha =
        std::atan2(dz, dx) - state.yaw_rad;
    const double curvature = 2.0 * std::sin(alpha) / target_distance;
    plan.steer_cmd = std::clamp(
        std::atan(curvature * limits.wheelbase_m), -limits.max_steer_rad, limits.max_steer_rad);

    // Regulated speed.
    double speed = std::min(cruise_speed_mps_, limits.max_speed_mps);
    speed /= 1.0 + curvature_gain_ * std::abs(curvature);
    std::uint8_t robot_cost = costmap.cost_at_world(state.position.x, state.position.z);
    if (robot_cost != OccupancyGrid::kUnknown
        && static_cast<double>(robot_cost) > obstacle_slow_start_) {
        const double span = 254.0 - obstacle_slow_start_;
        const double excess = (static_cast<double>(robot_cost) - obstacle_slow_start_) / span;
        speed *= std::max(0.2, 1.0 - excess);
    }
    const double goal_distance = (goal - state.position).horizontal_length();
    speed = std::min(speed, std::max(min_speed_mps_, goal_slow_gain_ * goal_distance));

    // Dynamic-obstacle yield guard: sample the robot's straight-ahead
    // constant-speed motion against every tracked object's constant-velocity
    // extrapolation; scale the commanded speed toward a full stop when the
    // minimum predicted surface separation drops below stop_distance_m.
    if (!tracked_objects.empty()) {
        constexpr double kPredictStepS = 0.1;
        double min_separation = std::numeric_limits<double>::infinity();
        const double heading_x = std::cos(state.yaw_rad);
        const double heading_z = std::sin(state.yaw_rad);
        for (double tau = 0.0; tau <= max_prediction_s_ + 1e-9; tau += kPredictStepS) {
            const double rx = state.position.x + heading_x * speed * tau;
            const double rz = state.position.z + heading_z * speed * tau;
            for (const TrackedObject& object : tracked_objects) {
                const double px = object.position.x + object.velocity.x * tau;
                const double pz = object.position.z + object.velocity.z * tau;
                const double dx = rx - px;
                const double dz = rz - pz;
                const double separation =
                    std::sqrt(dx * dx + dz * dz) - (robot_radius_m_ + object.radius_m);
                min_separation = std::min(min_separation, separation);
            }
        }
        if (min_separation < stop_distance_m_) {
            const double scale =
                std::clamp(min_separation / std::max(1e-9, stop_distance_m_), 0.0, 1.0);
            speed *= scale;
        }
    }
    plan.v_cmd = speed;

    // Reference trajectory for the tracking controller: the global path
    // segment ahead of the robot (not a constant-steer arc, which would feed
    // its own curvature back into the controller).
    const double reference_length = std::max(lookahead_m_, plan.v_cmd * horizon_s_) + 1.0;
    double covered = 0.0;
    Vec3 last = global_path.points[nearest];
    double stamp = 0.0;
    plan.trajectory.points.push_back(
        {last, std::atan2(dz, dx), plan.v_cmd, stamp});
    for (std::size_t i = nearest + 1; i < global_path.points.size(); ++i) {
        const Vec3& point = global_path.points[i];
        const double segment = (point - last).horizontal_length();
        covered += segment;
        stamp += segment / std::max(0.1, plan.v_cmd);
        plan.trajectory.points.push_back(
            {point, std::atan2(point.z - last.z, point.x - last.x), plan.v_cmd, stamp});
        last = point;
        if (covered >= reference_length) {
            break;
        }
    }
    if (plan.trajectory.points.size() < 2) {
        plan.trajectory = rollout_trajectory(
            state, plan.v_cmd, plan.steer_cmd, limits.wheelbase_m, horizon_s_, rollout_dt_s_);
    }
    plan.ok = true;
    return plan;
}

DwaPlanner::DwaPlanner(const agbot::config::ParamTable& params) {
    v_samples_ = static_cast<int>(agbot::config::integer_or(params, "v_samples", v_samples_));
    steer_samples_ =
        static_cast<int>(agbot::config::integer_or(params, "steer_samples", steer_samples_));
    horizon_s_ = agbot::config::double_or(params, "horizon_s", horizon_s_);
    rollout_dt_s_ = agbot::config::double_or(params, "rollout_dt_s", rollout_dt_s_);
    cruise_speed_mps_ = agbot::config::double_or(params, "cruise_speed_mps", cruise_speed_mps_);
    lethal_threshold_ = static_cast<std::uint8_t>(std::clamp<std::int64_t>(
        agbot::config::integer_or(params, "lethal_threshold", lethal_threshold_), 1, 254));
    w_obstacle_ = agbot::config::double_or(params, "w_obstacle", w_obstacle_);
    w_path_ = agbot::config::double_or(params, "w_path", w_path_);
    w_goal_ = agbot::config::double_or(params, "w_goal", w_goal_);
    w_speed_ = agbot::config::double_or(params, "w_speed", w_speed_);
    min_speed_mps_ = agbot::config::double_or(params, "min_speed_mps", min_speed_mps_);
    goal_slow_gain_ = agbot::config::double_or(params, "goal_slow_gain", goal_slow_gain_);
    w_dynamic_ = agbot::config::double_or(params, "w_dynamic", w_dynamic_);
    dynamic_margin_m_ = agbot::config::double_or(params, "dynamic_margin_m", dynamic_margin_m_);
    dynamic_sigma_m_ = agbot::config::double_or(params, "dynamic_sigma_m", dynamic_sigma_m_);
    max_prediction_s_ = agbot::config::double_or(params, "max_prediction_s", max_prediction_s_);
    robot_radius_m_ = agbot::config::double_or(params, "robot_radius_m", robot_radius_m_);
}

LocalPlan DwaPlanner::compute(
    const Costmap& costmap,
    const Path& global_path,
    const agbot::vehicles::EntityState& state,
    const agbot::vehicles::VehicleLimits& limits,
    const Vec3& goal,
    const std::vector<TrackedObject>& tracked_objects) {
    LocalPlan plan;
    if (global_path.points.empty()) {
        plan.reason = "empty_path";
        return plan;
    }

    const double goal_distance = (goal - state.position).horizontal_length();
    const double v_cap = std::min({cruise_speed_mps_, limits.max_speed_mps,
                                   std::max(min_speed_mps_, goal_slow_gain_ * goal_distance)});
    const double current_speed = signed_speed(state);
    const double window = limits.max_accel_mps2 * horizon_s_;
    const double v_lo = std::max(min_speed_mps_, current_speed - window);
    const double v_hi = std::min(v_cap, current_speed + window);
    const int v_count = std::max(2, v_samples_);
    const int steer_count = std::max(3, steer_samples_);

    double best_score = std::numeric_limits<double>::infinity();
    bool any_valid = false;

    for (int vi = 0; vi < v_count; ++vi) {
        const double v = v_lo
            + (v_hi - v_lo) * static_cast<double>(vi) / static_cast<double>(v_count - 1);
        if (v < min_speed_mps_ - 1e-9) {
            continue;
        }
        for (int si = 0; si < steer_count; ++si) {
            const double steer = -limits.max_steer_rad
                + 2.0 * limits.max_steer_rad * static_cast<double>(si)
                    / static_cast<double>(steer_count - 1);

            // Rollout and critics.
            EntityState rolled = state;
            double max_cost = 0.0;
            double max_dynamic_proximity = 0.0;
            bool lethal = false;
            double t = 0.0;
            const double inv_two_sigma_sq =
                1.0 / std::max(1e-9, 2.0 * dynamic_sigma_m_ * dynamic_sigma_m_);
            while (t + 1e-9 < horizon_s_) {
                rolled = bicycle_propagate(rolled, v, steer, limits.wheelbase_m, rollout_dt_s_);
                t += rollout_dt_s_;
                std::uint8_t cost = costmap.cost_at_world(rolled.position.x, rolled.position.z);
                if (cost == OccupancyGrid::kUnknown) {
                    cost = 0;
                }
                if (cost >= lethal_threshold_) {
                    lethal = true;
                    break;
                }
                max_cost = std::max(max_cost, static_cast<double>(cost));

                // Predictive dynamic-obstacle critic: constant-velocity
                // extrapolation; inside the hard margin the rollout is
                // rejected like a lethal cell.
                if (!tracked_objects.empty()) {
                    const double tau = std::min(t, max_prediction_s_);
                    for (const TrackedObject& object : tracked_objects) {
                        const double px = object.position.x + object.velocity.x * tau;
                        const double pz = object.position.z + object.velocity.z * tau;
                        const double dx = rolled.position.x - px;
                        const double dz = rolled.position.z - pz;
                        const double d = std::sqrt(dx * dx + dz * dz);
                        if (d < robot_radius_m_ + object.radius_m + dynamic_margin_m_) {
                            lethal = true;
                            break;
                        }
                        max_dynamic_proximity = std::max(
                            max_dynamic_proximity, std::exp(-(d * d) * inv_two_sigma_sq));
                    }
                    if (lethal) {
                        break;
                    }
                }
            }
            if (lethal) {
                continue;
            }

            const double obstacle_score = max_cost / 254.0;
            const double path_score = distance_to_path(global_path, rolled.position);
            const double goal_score = (goal - rolled.position).horizontal_length();
            const double speed_score = 1.0 - v / std::max(1e-6, v_cap);
            const double score = w_obstacle_ * obstacle_score + w_path_ * path_score
                + w_goal_ * goal_score + w_speed_ * speed_score
                + w_dynamic_ * max_dynamic_proximity;
            // Strict < keeps the first-best sample under the deterministic
            // iteration order.
            if (score < best_score) {
                best_score = score;
                plan.v_cmd = v;
                plan.steer_cmd = steer;
                any_valid = true;
            }
        }
    }

    if (!any_valid) {
        plan.reason = "all_rollouts_lethal";
        plan.v_cmd = 0.0;
        plan.steer_cmd = 0.0;
        return plan;
    }

    plan.trajectory = rollout_trajectory(
        state, plan.v_cmd, plan.steer_cmd, limits.wheelbase_m, horizon_s_, rollout_dt_s_);
    plan.ok = true;
    return plan;
}

const LocalPlannerRegistry& default_local_planner_registry() {
    static const LocalPlannerRegistry registry = [] {
        LocalPlannerRegistry built;
        built.register_factory(
            "pure_pursuit",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<ILocalPlanner> {
                return std::make_unique<PurePursuitPlanner>(params);
            });
        built.register_factory(
            "dwa",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<ILocalPlanner> {
                return std::make_unique<DwaPlanner>(params);
            });
        built.register_factory(
            "mppi",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<ILocalPlanner> {
                return std::make_unique<MppiPlanner>(params);
            });
        return built;
    }();
    return registry;
}

} // namespace agbot::nav
