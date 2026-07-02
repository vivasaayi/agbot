#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_vehicles/IVehicleModel.hpp"

namespace agbot::vehicles {

// Kinematic bicycle model on the XZ plane:
//   x' = v cos(yaw), z' = v sin(yaw), yaw' = v / L * tan(delta), v' = a
// Steering is rate limited toward the commanded angle; speed and acceleration
// are clamped by VehicleLimits. Reverse is allowed with a reduced max speed.
class KinematicBicycleModel final : public IVehicleModel {
public:
    KinematicBicycleModel() = default;
    explicit KinematicBicycleModel(const agbot::config::ParamTable& params);

    EntityState step(const EntityState& state, const Actuation& input, double dt_s) override;

    [[nodiscard]] const VehicleLimits& limits() const override { return limits_; }
    [[nodiscard]] VehicleKind kind() const override { return VehicleKind::Car; }
    [[nodiscard]] std::string name() const override { return "kinematic_bicycle"; }

    [[nodiscard]] double current_steer_rad() const { return steer_rad_; }
    void reset_steer(double steer_rad = 0.0) { steer_rad_ = steer_rad; }

private:
    void substep(EntityState& state, const Actuation& input, double dt_s);

    VehicleLimits limits_;
    double max_reverse_speed_mps_ = 3.0;
    double steer_rad_ = 0.0;
};

} // namespace agbot::vehicles
