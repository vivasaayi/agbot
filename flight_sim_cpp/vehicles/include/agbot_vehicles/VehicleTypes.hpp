#pragma once

#include "agbot_flight_sim/Vec3.hpp"

namespace agbot::vehicles {

using agbot::flight_sim::Vec3;

// World frame: Y-up, X/Z horizontal local meters. Yaw is measured on the XZ
// plane so that heading = (cos(yaw), 0, sin(yaw)).
struct EntityState {
    Vec3 position;
    Vec3 velocity;
    double yaw_rad = 0.0;
    double pitch_rad = 0.0;
    double roll_rad = 0.0;
    double time_s = 0.0;
};

// throttle in [-1, 1]: positive maps to acceleration, negative to braking /
// reverse. steer_rad is the commanded front-wheel angle (rate limited by the
// model).
struct Actuation {
    double throttle = 0.0;
    double steer_rad = 0.0;
};

struct VehicleLimits {
    double max_speed_mps = 10.0;
    double max_accel_mps2 = 3.0;
    double max_brake_mps2 = 5.0;
    double max_steer_rad = 0.6;
    double max_steer_rate_radps = 1.5;
    double wheelbase_m = 2.0;
};

enum class VehicleKind {
    Car,
    Multirotor,
    FixedWing,
};

[[nodiscard]] const char* to_string(VehicleKind kind);

// Stateless kinematic bicycle propagation used by the car model and by local
// planner rollouts. Single Euler step, no substepping, no rate limits.
[[nodiscard]] EntityState bicycle_propagate(
    const EntityState& state,
    double speed_mps,
    double steer_rad,
    double wheelbase_m,
    double dt_s);

} // namespace agbot::vehicles
