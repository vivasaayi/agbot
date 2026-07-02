#include "agbot_vehicles/VehicleTypes.hpp"

#include <cmath>

namespace agbot::vehicles {

const char* to_string(VehicleKind kind) {
    switch (kind) {
        case VehicleKind::Car:
            return "car";
        case VehicleKind::Multirotor:
            return "multirotor";
        case VehicleKind::FixedWing:
            return "fixed_wing";
    }
    return "unknown";
}

EntityState bicycle_propagate(
    const EntityState& state,
    double speed_mps,
    double steer_rad,
    double wheelbase_m,
    double dt_s) {
    EntityState next = state;
    next.position.x += speed_mps * std::cos(state.yaw_rad) * dt_s;
    next.position.z += speed_mps * std::sin(state.yaw_rad) * dt_s;
    if (wheelbase_m > 1e-9) {
        next.yaw_rad += speed_mps / wheelbase_m * std::tan(steer_rad) * dt_s;
    }
    next.velocity = {
        speed_mps * std::cos(next.yaw_rad),
        0.0,
        speed_mps * std::sin(next.yaw_rad),
    };
    next.time_s += dt_s;
    return next;
}

} // namespace agbot::vehicles
