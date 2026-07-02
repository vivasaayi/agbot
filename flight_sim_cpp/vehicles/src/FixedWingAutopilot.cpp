#include "agbot_vehicles/FixedWingAutopilot.hpp"

#include <algorithm>
#include <cmath>

namespace agbot::vehicles {

namespace {

constexpr double kPi = 3.14159265358979323846;

double clamp(double value, double lo, double hi) {
    return std::max(lo, std::min(hi, value));
}

double wrap_pi(double angle) {
    while (angle > kPi) {
        angle -= 2.0 * kPi;
    }
    while (angle < -kPi) {
        angle += 2.0 * kPi;
    }
    return angle;
}

} // namespace

FixedWingAutopilot::FixedWingAutopilot(const agbot::config::ParamTable& params) {
    using agbot::config::double_or;
    kp_speed_ = double_or(params, "kp_speed", kp_speed_);
    ki_speed_ = double_or(params, "ki_speed", ki_speed_);
    kp_alt_ = double_or(params, "kp_alt", kp_alt_);
    climb_rate_limit_mps_ = double_or(params, "climb_rate_limit_mps", climb_rate_limit_mps_);
    kp_vs_ = double_or(params, "kp_vs", kp_vs_);
    ki_vs_ = double_or(params, "ki_vs", ki_vs_);
    pitch_min_rad_ = double_or(params, "pitch_min_rad", pitch_min_rad_);
    pitch_max_rad_ = double_or(params, "pitch_max_rad", pitch_max_rad_);
    kp_pitch_ = double_or(params, "kp_pitch", kp_pitch_);
    kd_q_ = double_or(params, "kd_q", kd_q_);
    kp_heading_ = double_or(params, "kp_heading", kp_heading_);
    bank_limit_rad_ = double_or(params, "bank_limit_rad", bank_limit_rad_);
    kp_roll_ = double_or(params, "kp_roll", kp_roll_);
    kd_p_ = double_or(params, "kd_p", kd_p_);
    k_yaw_damper_ = double_or(params, "k_yaw_damper", k_yaw_damper_);
    washout_tau_s_ = double_or(params, "washout_tau_s", washout_tau_s_);
}

void FixedWingAutopilot::reset(const FixedWingControls& trim) {
    // Bumpless engagement: the speed integrator carries the trim throttle
    // directly and the pitch loop gets the trim elevator as a feedforward via
    // the integrator of the elevator command path (stored normalized).
    speed_integrator_ = trim.throttle;
    elevator_trim_ = trim.elevator;
    pitch_integrator_ = 0.0;
    yaw_rate_filtered_ = 0.0;
}

FixedWingControls FixedWingAutopilot::update(
    const EntityState& state,
    const Vec3& body_rates_pqr,
    const AutopilotCommand& command,
    double dt_s) {
    FixedWingControls out;
    if (!(dt_s > 0.0)) {
        return out;
    }

    const double airspeed = state.velocity.length();

    // --- Airspeed -> throttle (PI, clamped integrator anti-windup). ---
    const double speed_err = command.airspeed_mps - airspeed;
    double throttle = kp_speed_ * speed_err + speed_integrator_;
    const bool throttle_saturated =
        (throttle >= 1.0 && speed_err > 0.0) || (throttle <= 0.0 && speed_err < 0.0);
    if (!throttle_saturated) {
        speed_integrator_ = clamp(speed_integrator_ + ki_speed_ * speed_err * dt_s, 0.0, 1.0);
    }
    out.throttle = clamp(throttle, 0.0, 1.0);

    // --- Altitude -> climb rate -> pitch target (PI). ---
    const double climb_cmd = clamp(kp_alt_ * (command.altitude_m - state.position.y),
                                   -climb_rate_limit_mps_, climb_rate_limit_mps_);
    const double climb_err = climb_cmd - state.velocity.y;
    double pitch_cmd = kp_vs_ * climb_err + pitch_integrator_;
    const bool pitch_saturated = (pitch_cmd >= pitch_max_rad_ && climb_err > 0.0)
        || (pitch_cmd <= pitch_min_rad_ && climb_err < 0.0);
    if (!pitch_saturated) {
        pitch_integrator_ = clamp(pitch_integrator_ + ki_vs_ * climb_err * dt_s,
                                  pitch_min_rad_, pitch_max_rad_);
    }
    pitch_cmd = clamp(pitch_cmd, pitch_min_rad_, pitch_max_rad_);

    // --- Pitch -> elevator. Positive elevator = nose-down (Cm_de < 0), so a
    // nose-up pitch error commands negative elevator. Trim feedforward keeps
    // the loop centered on the trimmed surface position.
    out.elevator = clamp(
        elevator_trim_ - kp_pitch_ * (pitch_cmd - state.pitch_rad) + kd_q_ * body_rates_pqr.y,
        -1.0, 1.0);

    // --- Heading -> bank target. Repo yaw increases from +X (east) toward
    // +Z (north): a RIGHT bank (positive roll) turns the aircraft clockwise
    // seen from above, which DECREASES repo yaw, hence the sign flip.
    const double heading_err = wrap_pi(command.heading_rad - state.yaw_rad);
    const double bank_cmd = clamp(-kp_heading_ * heading_err, -bank_limit_rad_, bank_limit_rad_);

    // --- Bank -> aileron (P + roll damping). ---
    out.aileron = clamp(
        kp_roll_ * (bank_cmd - state.roll_rad) - kd_p_ * body_rates_pqr.x, -1.0, 1.0);

    // --- Rudder: washed-out yaw damper. The washout removes the steady
    // turn-rate component so the damper only fights dutch-roll transients.
    yaw_rate_filtered_ += (body_rates_pqr.z - yaw_rate_filtered_) * dt_s / washout_tau_s_;
    out.rudder = clamp(k_yaw_damper_ * (body_rates_pqr.z - yaw_rate_filtered_), -1.0, 1.0);

    return out;
}

} // namespace agbot::vehicles
