#include "agbot_vehicles/MultirotorModel.hpp"

#include <algorithm>
#include <cmath>

namespace agbot::vehicles {

namespace {

int substep_count(double dt_s, double max_substep_s) {
    if (dt_s <= max_substep_s) {
        return 1;
    }
    return std::max(1, static_cast<int>(std::ceil(dt_s / max_substep_s - 1e-9)));
}

double clamp(double value, double lo, double hi) {
    return std::max(lo, std::min(hi, value));
}

// DroneSimulation::move_towards_velocity idiom: approach the desired velocity
// with an acceleration-magnitude clamp.
Vec3 move_towards(const Vec3& current, const Vec3& desired, double max_delta) {
    const Vec3 delta = desired - current;
    const double magnitude = delta.length();
    if (magnitude <= max_delta || magnitude <= 1e-12) {
        return desired;
    }
    return current + delta * (max_delta / magnitude);
}

} // namespace

MultirotorModel::MultirotorModel(const agbot::config::ParamTable& params) : MultirotorModel() {
    limits_.max_speed_mps = agbot::config::double_or(params, "max_speed_mps", limits_.max_speed_mps);
    limits_.max_accel_mps2 =
        agbot::config::double_or(params, "max_accel_mps2", limits_.max_accel_mps2);
    limits_.max_steer_rate_radps =
        agbot::config::double_or(params, "max_yaw_rate_radps", limits_.max_steer_rate_radps);
    hold_altitude_m_ = agbot::config::double_or(params, "hold_altitude_m", hold_altitude_m_);
    altitude_gain_per_s_ =
        agbot::config::double_or(params, "altitude_gain_per_s", altitude_gain_per_s_);
}

EntityState MultirotorModel::step(const EntityState& state, const Actuation& input, double dt_s) {
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

void MultirotorModel::substep(EntityState& state, const Actuation& input, double dt_s) {
    // Yaw-rate command.
    const double yaw_rate = clamp(input.steer_rad, -limits_.max_steer_rate_radps,
                                  limits_.max_steer_rate_radps);
    state.yaw_rad += yaw_rate * dt_s;

    Vec3 desired;
    if (velocity_setpoint_.has_value()) {
        desired = *velocity_setpoint_;
        const double horizontal = desired.horizontal_length();
        if (horizontal > limits_.max_speed_mps) {
            const double scale = limits_.max_speed_mps / horizontal;
            desired.x *= scale;
            desired.z *= scale;
        }
    } else {
        const double forward_speed =
            clamp(input.throttle, -1.0, 1.0) * limits_.max_speed_mps;
        desired = {
            forward_speed * std::cos(state.yaw_rad),
            0.0,
            forward_speed * std::sin(state.yaw_rad),
        };
    }
    // Altitude hold: proportional climb toward the hold altitude.
    desired.y = clamp((hold_altitude_m_ - state.position.y) * altitude_gain_per_s_,
                      -limits_.max_speed_mps, limits_.max_speed_mps);

    state.velocity = move_towards(state.velocity, desired, limits_.max_accel_mps2 * dt_s);
    state.position += state.velocity * dt_s;
    state.time_s += dt_s;
}

} // namespace agbot::vehicles
