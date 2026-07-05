#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_nav/NavTypes.hpp"
#include "agbot_vehicles/ParamRegistry.hpp"
#include "agbot_vehicles/VehicleTypes.hpp"

#include <memory>
#include <string>

namespace agbot::nav {

class IController {
public:
    virtual ~IController() = default;
    // Track the reference path at v_cmd; returns an Actuation respecting the
    // vehicle limits.
    virtual agbot::vehicles::Actuation control(
        const agbot::vehicles::EntityState& state,
        const Path& reference,
        double v_cmd,
        const agbot::vehicles::VehicleLimits& limits,
        double dt_s) = 0;
    virtual void reset() = 0;
    [[nodiscard]] virtual std::string name() const = 0;
};

// Longitudinal PID on speed error plus Stanley lateral control using the
// front-axle crosstrack error against the reference path:
//   steer = heading_error - atan(k_e * e / (k_soft + v))
// Params: kp, ki, kd, integral_limit, k_e, k_soft.
class PidStanleyController final : public IController {
public:
    PidStanleyController() = default;
    explicit PidStanleyController(const agbot::config::ParamTable& params);

    agbot::vehicles::Actuation control(
        const agbot::vehicles::EntityState& state,
        const Path& reference,
        double v_cmd,
        const agbot::vehicles::VehicleLimits& limits,
        double dt_s) override;

    void reset() override;
    [[nodiscard]] std::string name() const override { return "pid_stanley"; }

private:
    double kp_ = 1.2;
    double ki_ = 0.2;
    double kd_ = 0.0;
    double integral_limit_ = 2.0;
    double k_e_ = 1.0;
    double k_soft_ = 1.0;

    double integral_ = 0.0;
    double previous_error_ = 0.0;
    bool has_previous_error_ = false;
};

using ControllerRegistry = agbot::vehicles::ParamRegistry<IController>;
[[nodiscard]] const ControllerRegistry& default_controller_registry();

} // namespace agbot::nav
