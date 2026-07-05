#include "agbot_vehicles/FixedWingModel.hpp"
#include "agbot_vehicles/IVehicleModel.hpp"
#include "agbot_vehicles/KinematicBicycleModel.hpp"
#include "agbot_vehicles/MultirotorModel.hpp"

namespace agbot::vehicles {

const VehicleModelRegistry& default_vehicle_registry() {
    static const VehicleModelRegistry registry = [] {
        VehicleModelRegistry built;
        built.register_factory(
            "kinematic_bicycle",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<IVehicleModel> {
                return std::make_unique<KinematicBicycleModel>(params);
            });
        built.register_factory(
            "multirotor",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<IVehicleModel> {
                return std::make_unique<MultirotorModel>(params);
            });
        built.register_factory(
            "fixed_wing",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<IVehicleModel> {
                return std::make_unique<FixedWingModel>(params);
            });
        return built;
    }();
    return registry;
}

std::unique_ptr<IVehicleModel> create_vehicle_model(
    const std::string& name,
    const agbot::config::ParamTable& params) {
    return default_vehicle_registry().create(name, params);
}

} // namespace agbot::vehicles
