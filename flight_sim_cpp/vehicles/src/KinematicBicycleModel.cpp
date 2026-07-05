#include "agbot_vehicles/KinematicBicycleModel.hpp"

#include <algorithm>
#include <cmath>

namespace agbot::vehicles {

namespace {

int substep_count(double dt_s, double max_substep_s) {
    if (dt_s <= max_substep_s) {
        return 1;
    }
    // Guard against float noise so dt = n * max_substep splits into exactly n
    // substeps (keeps step(1.0) bit-identical to 50 x step(0.02)).
    return std::max(1, static_cast<int>(std::ceil(dt_s / max_substep_s - 1e-9)));
}

double clamp(double value, double lo, double hi) {
    return std::max(lo, std::min(hi, value));
}

} // namespace

KinematicBicycleModel::KinematicBicycleModel(const agbot::config::ParamTable& params) {
    limits_.max_speed_mps = agbot::config::double_or(params, "max_speed_mps", limits_.max_speed_mps);
    limits_.max_accel_mps2 =
        agbot::config::double_or(params, "max_accel_mps2", limits_.max_accel_mps2);
    limits_.max_brake_mps2 =
        agbot::config::double_or(params, "max_brake_mps2", limits_.max_brake_mps2);
    limits_.max_steer_rad = agbot::config::double_or(params, "max_steer_rad", limits_.max_steer_rad);
    limits_.max_steer_rate_radps =
        agbot::config::double_or(params, "max_steer_rate_radps", limits_.max_steer_rate_radps);
    limits_.wheelbase_m = agbot::config::double_or(params, "wheelbase_m", limits_.wheelbase_m);
    max_reverse_speed_mps_ =
        agbot::config::double_or(params, "max_reverse_speed_mps", max_reverse_speed_mps_);
}

EntityState KinematicBicycleModel::step(
    const EntityState& state,
    const Actuation& input,
    double dt_s) {
    EntityState next = state;
    if (!(dt_s > 0.0)) {
        return next;
    }
    const int steps = substep_count(dt_s, kMaxSubstepS);
    const double substep_s = dt_s / static_cast<double>(steps);
    for (int i = 0; i < steps; ++i) {
        substep(next, input, substep_s);
    }
    return next;
}

void KinematicBicycleModel::substep(EntityState& state, const Actuation& input, double dt_s) {
    // Steering: rate limit toward the clamped command.
    const double steer_target = clamp(input.steer_rad, -limits_.max_steer_rad, limits_.max_steer_rad);
    const double max_steer_delta = limits_.max_steer_rate_radps * dt_s;
    steer_rad_ += clamp(steer_target - steer_rad_, -max_steer_delta, max_steer_delta);

    // Longitudinal: signed speed along the heading.
    const double heading_x = std::cos(state.yaw_rad);
    const double heading_z = std::sin(state.yaw_rad);
    double speed = state.velocity.x * heading_x + state.velocity.z * heading_z;
    const double throttle = clamp(input.throttle, -1.0, 1.0);
    const double accel = throttle >= 0.0
        ? throttle * limits_.max_accel_mps2
        : throttle * limits_.max_brake_mps2;
    speed = clamp(speed + accel * dt_s, -max_reverse_speed_mps_, limits_.max_speed_mps);

    state = bicycle_propagate(state, speed, steer_rad_, limits_.wheelbase_m, dt_s);
}

} // namespace agbot::vehicles
