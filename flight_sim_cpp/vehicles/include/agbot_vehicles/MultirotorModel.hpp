#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_vehicles/IVehicleModel.hpp"

#include <optional>

namespace agbot::vehicles {

// Velocity-control multirotor kinematics matching DroneSimulation's
// move_towards_velocity idiom: the commanded velocity is approached with an
// acceleration clamp, altitude is held at hold_altitude_m.
//
// Actuation mapping: throttle in [-1,1] -> forward speed setpoint
// (throttle * max_speed along the current heading), steer_rad -> yaw rate
// command clamped to max_steer_rate_radps. A direct world-frame velocity
// setpoint can be injected with set_velocity_setpoint(), overriding the
// Actuation mapping until cleared.
class MultirotorModel final : public IVehicleModel {
public:
    MultirotorModel() = default;
    explicit MultirotorModel(const agbot::config::ParamTable& params);

    EntityState step(const EntityState& state, const Actuation& input, double dt_s) override;

    [[nodiscard]] const VehicleLimits& limits() const override { return limits_; }
    [[nodiscard]] VehicleKind kind() const override { return VehicleKind::Multirotor; }
    [[nodiscard]] std::string name() const override { return "multirotor"; }

    void set_velocity_setpoint(const Vec3& velocity_mps) { velocity_setpoint_ = velocity_mps; }
    void clear_velocity_setpoint() { velocity_setpoint_.reset(); }
    [[nodiscard]] double hold_altitude_m() const { return hold_altitude_m_; }

private:
    void substep(EntityState& state, const Actuation& input, double dt_s);

    VehicleLimits limits_ = {12.0, 6.0, 6.0, 1.4, 1.4, 0.0};
    double hold_altitude_m_ = 5.0;
    double altitude_gain_per_s_ = 1.0;
    std::optional<Vec3> velocity_setpoint_;
};

} // namespace agbot::vehicles
