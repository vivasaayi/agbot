#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_vehicles/FixedWingModel.hpp"
#include "agbot_vehicles/VehicleTypes.hpp"

namespace agbot::vehicles {

// Commanded flight condition. heading_rad uses the repo yaw convention
// (angle on the XZ plane from +X east toward +Z north).
struct AutopilotCommand {
    double heading_rad = 0.0;
    double altitude_m = 0.0;
    double airspeed_mps = 0.0;
};

// Classic cascaded PID autopilot for the fixed wing model:
//   airspeed -> throttle           (PI with anti-windup)
//   altitude -> climb-rate target -> pitch target (PI) -> elevator (P + q damping)
//   heading  -> bank target (P, bank-limited) -> aileron (P + p damping)
//   rudder: washed-out yaw-rate damper (leaves steady turn rate alone)
// All gains and limits are overridable via ParamTable.
class FixedWingAutopilot {
public:
    FixedWingAutopilot() = default;
    explicit FixedWingAutopilot(const agbot::config::ParamTable& params);

    // Preset integrators from trim controls so engagement is bumpless.
    void reset(const FixedWingControls& trim);

    // body_rates_pqr are the aircraft body rates (p, q, r) from
    // FixedWingModel::body_rates().
    FixedWingControls update(
        const EntityState& state,
        const Vec3& body_rates_pqr,
        const AutopilotCommand& command,
        double dt_s);

private:
    // Airspeed loop.
    double kp_speed_ = 0.12;
    double ki_speed_ = 0.04;
    // Altitude -> climb rate.
    double kp_alt_ = 0.15;
    double climb_rate_limit_mps_ = 3.5;
    // Climb rate -> pitch target.
    double kp_vs_ = 0.03;
    double ki_vs_ = 0.012;
    double pitch_min_rad_ = -0.30;
    double pitch_max_rad_ = 0.30;
    // Pitch -> elevator.
    double kp_pitch_ = 3.0;
    double kd_q_ = 5.0;
    // Heading -> bank.
    double kp_heading_ = 1.6;
    double bank_limit_rad_ = 0.5236; // ~30 deg
    // Bank -> aileron.
    double kp_roll_ = 2.0;
    double kd_p_ = 0.6;
    // Rudder yaw damper (washout high-pass).
    double k_yaw_damper_ = 1.0;
    double washout_tau_s_ = 1.0;

    double speed_integrator_ = 0.0;
    double pitch_integrator_ = 0.0;
    double yaw_rate_filtered_ = 0.0;
    double elevator_trim_ = 0.0;
};

} // namespace agbot::vehicles
