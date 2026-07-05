#include "agbot_nav/MppiPlanner.hpp"

#include "agbot_vehicles/VehicleTypes.hpp"

#include <algorithm>
#include <cmath>
#include <limits>

namespace agbot::nav {

namespace {

using agbot::vehicles::bicycle_propagate;
using agbot::vehicles::EntityState;

constexpr double kTwoPi = 6.28318530717958647692;
constexpr double kLethalPenalty = 1e6;

// SplitMix64: the counter-based RNG core. Deterministic and platform
// independent (no library distribution objects involved).
std::uint64_t splitmix64(std::uint64_t x) {
    x += 0x9E3779B97F4A7C15ULL;
    x = (x ^ (x >> 30)) * 0xBF58476D1CE4E5B9ULL;
    x = (x ^ (x >> 27)) * 0x94D049BB133111EBULL;
    return x ^ (x >> 31);
}

// Uniform double in (0, 1] from a 64-bit word (never 0, safe for log()).
double uniform_open(std::uint64_t bits) {
    return (static_cast<double>(bits >> 11) + 1.0) * 0x1.0p-53;
}

// Two standard normals from a (seed, call, sample, step) counter key via
// Box-Muller; identical keys always produce identical draws.
void counter_gaussians(
    std::uint64_t seed,
    std::uint64_t call,
    std::uint64_t sample,
    std::uint64_t step,
    double& z0,
    double& z1) {
    std::uint64_t key = splitmix64(seed ^ 0x8E51ECEEA24C6F31ULL);
    key = splitmix64(key ^ call);
    key = splitmix64(key ^ (sample * 0xD1B54A32D192ED03ULL));
    key = splitmix64(key ^ (step * 0xA0761D6478BD642FULL));
    const double u1 = uniform_open(key);
    const double u2 = uniform_open(splitmix64(key));
    const double radius = std::sqrt(-2.0 * std::log(u1));
    z0 = radius * std::cos(kTwoPi * u2);
    z1 = radius * std::sin(kTwoPi * u2);
}

double signed_speed(const EntityState& state) {
    return state.velocity.x * std::cos(state.yaw_rad)
        + state.velocity.z * std::sin(state.yaw_rad);
}

double distance_to_polyline(const std::vector<Vec3>& points, const Vec3& position) {
    if (points.empty()) {
        return 0.0;
    }
    if (points.size() == 1) {
        return (points.front() - position).horizontal_length();
    }
    double best = std::numeric_limits<double>::infinity();
    for (std::size_t i = 1; i < points.size(); ++i) {
        const Vec3& a = points[i - 1];
        const Vec3& b = points[i];
        const double abx = b.x - a.x;
        const double abz = b.z - a.z;
        const double denom = abx * abx + abz * abz;
        double t = 0.0;
        if (denom > 1e-12) {
            t = ((position.x - a.x) * abx + (position.z - a.z) * abz) / denom;
            t = std::clamp(t, 0.0, 1.0);
        }
        const double dx = position.x - (a.x + abx * t);
        const double dz = position.z - (a.z + abz * t);
        best = std::min(best, std::sqrt(dx * dx + dz * dz));
    }
    return best;
}

} // namespace

MppiPlanner::MppiPlanner(const agbot::config::ParamTable& params) {
    using agbot::config::double_or;
    using agbot::config::integer_or;
    time_steps_ = static_cast<int>(std::clamp<std::int64_t>(
        integer_or(params, "time_steps", time_steps_), 2, 1000));
    dt_ = double_or(params, "dt", dt_);
    num_samples_ = static_cast<int>(std::clamp<std::int64_t>(
        integer_or(params, "num_samples", num_samples_), 2, 65536));
    lambda_ = double_or(params, "lambda", lambda_);
    sigma_accel_ = double_or(params, "sigma_accel", sigma_accel_);
    sigma_steer_rate_ = double_or(params, "sigma_steer_rate", sigma_steer_rate_);
    cruise_speed_mps_ = double_or(params, "cruise_speed_mps", cruise_speed_mps_);
    lethal_threshold_ = static_cast<std::uint8_t>(std::clamp<std::int64_t>(
        integer_or(params, "lethal_threshold", lethal_threshold_), 1, 254));
    w_obstacle_ = double_or(params, "w_obstacle", w_obstacle_);
    w_path_ = double_or(params, "w_path", w_path_);
    w_goal_ = double_or(params, "w_goal", w_goal_);
    w_speed_ = double_or(params, "w_speed", w_speed_);
    w_smooth_ = double_or(params, "w_smooth", w_smooth_);
    min_speed_mps_ = double_or(params, "min_speed_mps", min_speed_mps_);
    goal_slow_gain_ = double_or(params, "goal_slow_gain", goal_slow_gain_);
    seed_ = static_cast<std::uint64_t>(
        std::max<std::int64_t>(0, integer_or(params, "seed", static_cast<std::int64_t>(seed_))));
    w_dynamic_ = double_or(params, "w_dynamic", w_dynamic_);
    dynamic_margin_m_ = double_or(params, "dynamic_margin_m", dynamic_margin_m_);
    dynamic_sigma_m_ = double_or(params, "dynamic_sigma_m", dynamic_sigma_m_);
    max_prediction_s_ = double_or(params, "max_prediction_s", max_prediction_s_);
    robot_radius_m_ = double_or(params, "robot_radius_m", robot_radius_m_);
}

LocalPlan MppiPlanner::compute(
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
    if (!(dt_ > 0.0) || !(lambda_ > 0.0)) {
        plan.reason = "invalid_planner_parameters";
        return plan;
    }
    const std::uint64_t call = call_index_++;
    const std::size_t horizon = static_cast<std::size_t>(time_steps_);

    // Shift the previous solution one step (receding horizon warm start).
    if (nominal_.size() == horizon) {
        for (std::size_t t = 0; t + 1 < horizon; ++t) {
            nominal_[t] = nominal_[t + 1];
        }
        nominal_.back() = Control{};
    } else {
        nominal_.assign(horizon, Control{});
    }

    // Local path window ahead of the robot for the path_align critic: keeps
    // the per-sample distance query cheap and pointed forward.
    std::vector<Vec3> path_window;
    {
        std::size_t nearest = 0;
        double best_distance = std::numeric_limits<double>::infinity();
        for (std::size_t i = 0; i < global_path.points.size(); ++i) {
            const double distance =
                (global_path.points[i] - state.position).horizontal_length();
            if (distance < best_distance) {
                best_distance = distance;
                nearest = i;
            }
        }
        const double window_length =
            std::max(cruise_speed_mps_, 1.0) * dt_ * static_cast<double>(time_steps_) + 5.0;
        double covered = 0.0;
        path_window.push_back(global_path.points[nearest]);
        for (std::size_t i = nearest + 1; i < global_path.points.size(); ++i) {
            covered += (global_path.points[i] - global_path.points[i - 1]).horizontal_length();
            path_window.push_back(global_path.points[i]);
            if (covered >= window_length) {
                break;
            }
        }
    }

    const double goal_distance = (goal - state.position).horizontal_length();
    const double v_target = std::min(
        {cruise_speed_mps_, limits.max_speed_mps,
         std::max(min_speed_mps_, goal_slow_gain_ * goal_distance)});
    const double v0 = signed_speed(state);
    const double steer0 = std::clamp(plan_steer_seed_, -limits.max_steer_rad,
                                     limits.max_steer_rad);

    std::vector<Control> perturbed(static_cast<std::size_t>(num_samples_) * horizon);
    std::vector<double> scores(static_cast<std::size_t>(num_samples_), 0.0);

    for (int k = 0; k < num_samples_; ++k) {
        Control* controls = &perturbed[static_cast<std::size_t>(k) * horizon];
        double obstacle_acc = 0.0;
        double path_acc = 0.0;
        double speed_acc = 0.0;
        double smooth_acc = 0.0;
        double dynamic_acc = 0.0;
        double lethal_cost = 0.0;
        EntityState rolled = state;
        double v = v0;
        double steer = steer0;
        for (std::size_t t = 0; t < horizon; ++t) {
            double noise_a = 0.0;
            double noise_s = 0.0;
            counter_gaussians(seed_, call, static_cast<std::uint64_t>(k),
                              static_cast<std::uint64_t>(t), noise_a, noise_s);
            noise_a *= sigma_accel_;
            noise_s *= sigma_steer_rate_;
            Control u;
            u.accel = std::clamp(nominal_[t].accel + noise_a,
                                 -limits.max_brake_mps2, limits.max_accel_mps2);
            u.steer_rate = std::clamp(nominal_[t].steer_rate + noise_s,
                                      -limits.max_steer_rate_radps,
                                      limits.max_steer_rate_radps);
            controls[t] = u;

            v = std::clamp(v + u.accel * dt_, 0.0, limits.max_speed_mps);
            steer = std::clamp(steer + u.steer_rate * dt_,
                               -limits.max_steer_rad, limits.max_steer_rad);
            rolled = bicycle_propagate(rolled, v, steer, limits.wheelbase_m, dt_);

            std::uint8_t cost = costmap.cost_at_world(rolled.position.x, rolled.position.z);
            if (cost == OccupancyGrid::kUnknown) {
                cost = 0;
            }
            if (cost >= lethal_threshold_) {
                lethal_cost += kLethalPenalty;
            }
            obstacle_acc += static_cast<double>(cost) / 254.0;

            // Predictive dynamic-obstacle critic: constant-velocity
            // extrapolation of every tracked object to this rollout time.
            if (!tracked_objects.empty()) {
                const double tau =
                    std::min(static_cast<double>(t + 1) * dt_, max_prediction_s_);
                const double inv_two_sigma_sq =
                    1.0 / std::max(1e-9, 2.0 * dynamic_sigma_m_ * dynamic_sigma_m_);
                for (const TrackedObject& object : tracked_objects) {
                    const double px = object.position.x + object.velocity.x * tau;
                    const double pz = object.position.z + object.velocity.z * tau;
                    const double dx = rolled.position.x - px;
                    const double dz = rolled.position.z - pz;
                    const double d = std::sqrt(dx * dx + dz * dz);
                    if (d < robot_radius_m_ + object.radius_m + dynamic_margin_m_) {
                        lethal_cost += kLethalPenalty;
                    }
                    dynamic_acc += std::exp(-(d * d) * inv_two_sigma_sq);
                }
            }

            path_acc += distance_to_polyline(path_window, rolled.position);
            speed_acc += std::abs(v - v_target);
            smooth_acc += (noise_a * noise_a)
                    / std::max(1e-9, sigma_accel_ * sigma_accel_)
                + (noise_s * noise_s)
                    / std::max(1e-9, sigma_steer_rate_ * sigma_steer_rate_);
        }
        const double steps = static_cast<double>(horizon);
        const double goal_score = (goal - rolled.position).horizontal_length();
        scores[static_cast<std::size_t>(k)] = lethal_cost
            + w_obstacle_ * obstacle_acc / steps
            + w_path_ * path_acc / steps
            + w_goal_ * goal_score
            + w_speed_ * speed_acc / std::max(1e-6, v_target) / steps
            + w_smooth_ * smooth_acc / steps
            + w_dynamic_ * dynamic_acc / steps;
    }

    const double best_score = *std::min_element(scores.begin(), scores.end());
    if (best_score >= kLethalPenalty) {
        plan.reason = "all_rollouts_lethal";
        nominal_.assign(horizon, Control{});
        return plan;
    }

    // Softmax weights with temperature lambda over the score gap.
    double weight_sum = 0.0;
    std::vector<double> weights(scores.size(), 0.0);
    for (std::size_t k = 0; k < scores.size(); ++k) {
        weights[k] = std::exp(-(scores[k] - best_score) / lambda_);
        weight_sum += weights[k];
    }
    for (std::size_t t = 0; t < horizon; ++t) {
        double accel = 0.0;
        double steer_rate = 0.0;
        for (std::size_t k = 0; k < scores.size(); ++k) {
            const Control& u = perturbed[k * horizon + t];
            accel += weights[k] * u.accel;
            steer_rate += weights[k] * u.steer_rate;
        }
        nominal_[t].accel = accel / weight_sum;
        nominal_[t].steer_rate = steer_rate / weight_sum;
    }

    // Roll out the weighted-mean sequence for the reference trajectory and
    // the immediate command.
    EntityState rolled = state;
    double v = v0;
    double steer = steer0;
    plan.trajectory.points.push_back({rolled.position, rolled.yaw_rad, v, 0.0});
    for (std::size_t t = 0; t < horizon; ++t) {
        v = std::clamp(v + nominal_[t].accel * dt_, 0.0, limits.max_speed_mps);
        steer = std::clamp(steer + nominal_[t].steer_rate * dt_,
                           -limits.max_steer_rad, limits.max_steer_rad);
        rolled = bicycle_propagate(rolled, v, steer, limits.wheelbase_m, dt_);
        plan.trajectory.points.push_back(
            {rolled.position, rolled.yaw_rad, v, static_cast<double>(t + 1) * dt_});
        if (t == 0) {
            plan.v_cmd = v;
            plan.steer_cmd = steer;
        }
    }
    plan_steer_seed_ = plan.steer_cmd;
    plan.ok = true;
    return plan;
}

} // namespace agbot::nav
