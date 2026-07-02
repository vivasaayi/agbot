#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_nav/LocalPlanner.hpp"

#include <cstdint>
#include <string>
#include <vector>

namespace agbot::nav {

// Model Predictive Path Integral local planner: num_samples control sequences
// of (accel, steer_rate) over time_steps x dt are drawn as the shifted
// previous solution plus seeded Gaussian noise, rolled out through the shared
// kinematic bicycle, scored by critics, and softmax-combined with temperature
// lambda into a weighted-mean control sequence. Critics: obstacle (costmap
// lookup, lethal samples get a huge penalty and never dominate the softmax),
// path_align (mean distance to the global path), goal_dist (final distance to
// goal), speed (deviation from the goal-regulated cruise target) and
// smoothness (control effort of the injected noise). Determinism: a
// counter-based RNG (SplitMix64 keyed on seed, compute-call index, sample and
// step, Box-Muller transform) makes every compute() bit-identical for a fixed
// seed and call sequence, independent of platform library distributions.
//
// Params: time_steps, dt, num_samples, lambda, sigma_accel, sigma_steer_rate,
// cruise_speed_mps, lethal_threshold, w_obstacle, w_path, w_goal, w_speed,
// w_smooth, min_speed_mps, goal_slow_gain, seed.
class MppiPlanner final : public ILocalPlanner {
public:
    MppiPlanner() = default;
    explicit MppiPlanner(const agbot::config::ParamTable& params);

    LocalPlan compute(
        const Costmap& costmap,
        const Path& global_path,
        const agbot::vehicles::EntityState& state,
        const agbot::vehicles::VehicleLimits& limits,
        const Vec3& goal) override;

    [[nodiscard]] std::string name() const override { return "mppi"; }

private:
    struct Control {
        double accel = 0.0;
        double steer_rate = 0.0;
    };

    int time_steps_ = 30;
    double dt_ = 0.05;
    int num_samples_ = 1024;
    double lambda_ = 0.4;
    double sigma_accel_ = 1.0;
    double sigma_steer_rate_ = 1.2;
    double cruise_speed_mps_ = 3.0;
    std::uint8_t lethal_threshold_ = 200;
    double w_obstacle_ = 3.0;
    double w_path_ = 1.2;
    double w_goal_ = 0.6;
    double w_speed_ = 0.4;
    double w_smooth_ = 0.05;
    double min_speed_mps_ = 0.0;
    double goal_slow_gain_ = 0.8;
    std::uint64_t seed_ = 1;

    std::vector<Control> nominal_; // previous solution, shifted each call
    double plan_steer_seed_ = 0.0; // steer state carried between calls
    std::uint64_t call_index_ = 0;
};

} // namespace agbot::nav
