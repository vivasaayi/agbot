#pragma once

#include "agbot_vehicles/ParamRegistry.hpp"
#include "agbot_vehicles/VehicleTypes.hpp"

#include <memory>
#include <string>

namespace agbot::vehicles {

class IVehicleModel {
public:
    virtual ~IVehicleModel() = default;

    // Advance the vehicle by dt seconds. Implementations substep internally at
    // no more than kMaxSubstepS so large steps stay deterministic and stable.
    virtual EntityState step(const EntityState& state, const Actuation& input, double dt_s) = 0;

    [[nodiscard]] virtual const VehicleLimits& limits() const = 0;
    [[nodiscard]] virtual VehicleKind kind() const = 0;
    [[nodiscard]] virtual std::string name() const = 0;

    static constexpr double kMaxSubstepS = 0.02;
};

using VehicleModelRegistry = ParamRegistry<IVehicleModel>;

// Registry pre-populated with the built-in models:
//   "kinematic_bicycle" -> KinematicBicycleModel (Car)
//   "multirotor"        -> MultirotorModel (Multirotor)
[[nodiscard]] const VehicleModelRegistry& default_vehicle_registry();

[[nodiscard]] std::unique_ptr<IVehicleModel> create_vehicle_model(
    const std::string& name,
    const agbot::config::ParamTable& params);

} // namespace agbot::vehicles
