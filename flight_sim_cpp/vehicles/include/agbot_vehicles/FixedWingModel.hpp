#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_vehicles/IVehicleModel.hpp"

#include <optional>

namespace agbot::vehicles {

// Control surface commands, normalized. Sign conventions follow standard
// aerodynamic surface deflections (not pilot-stick deflections):
//   throttle  in [0, 1]
//   elevator  in [-1, 1]; positive = trailing edge down = nose-DOWN moment
//             (Cm_de is negative), so "stick pull" is negative elevator.
//   aileron   in [-1, 1]; positive = roll RIGHT moment (right wing down).
//   rudder    in [-1, 1]; positive = nose-LEFT yaw moment (Cn_dr negative).
struct FixedWingControls {
    double throttle = 0.0;
    double elevator = 0.0;
    double aileron = 0.0;
    double rudder = 0.0;
};

// All aerodynamic, mass, propulsion and geometric parameters of the fixed
// wing model. Defaults are Cessna-172-class values taken from the standard
// public-domain datasets (Roskam "Airplane Flight Dynamics", McRuer et al.,
// and the UIUC/Beard-McLain C172 coefficient sets).
struct FixedWingParams {
    // Mass / geometry.
    double mass_kg = 1043.0;
    double wing_area_m2 = 16.17;
    double wing_span_m = 11.0;
    double wing_chord_m = 1.49;
    double ixx = 1285.0;
    double iyy = 1825.0;
    double izz = 2667.0;

    // Lift: CL = CL0 + CLa * alpha (linear region), blended into a
    // flat-plate falloff beyond alpha_stall (see FixedWingModel.cpp).
    double cl0 = 0.31;
    double cl_alpha = 5.1;       // per rad
    double alpha_stall_rad = 0.2618; // ~15 deg
    double stall_blend_rate = 50.0;  // sigmoid sharpness of the stall blend

    // Drag polar: CD = CD0 + K * CL^2.
    double cd0 = 0.031;
    double k_induced = 0.054;

    // Side force.
    double cy_beta = -0.31; // per rad

    // Pitch moment.
    double cm0 = -0.015;
    double cm_alpha = -0.89; // per rad
    double cm_de = -1.28;    // per rad of elevator deflection
    double cm_q = -12.4;     // per (q c / 2V)

    // Roll moment.
    double cl_da = 0.178;  // per rad of aileron deflection
    double cl_p = -0.47;   // per (p b / 2V)
    double cl_beta = -0.089; // dihedral effect, per rad
    double cl_r = 0.096;   // per (r b / 2V)

    // Yaw moment.
    double cn_beta = 0.065; // per rad
    double cn_dr = -0.069;  // per rad of rudder deflection
    double cn_r = -0.099;   // per (r b / 2V)
    double cn_p = -0.03;    // per (p b / 2V)

    // Surface deflection limits (rad) mapping the normalized controls.
    double elevator_max_rad = 0.4363; // 25 deg
    double aileron_max_rad = 0.3491;  // 20 deg
    double rudder_max_rad = 0.2793;   // 16 deg

    // Propulsion. Static thrust with a quadratic airspeed falloff:
    //   T = throttle * t_max_n * max(0, 1 - (V / v_max_mps)^2)
    // A quadratic (roughly constant-power) falloff is used instead of a
    // linear one because a linear falloff to zero at 85 m/s leaves less
    // thrust at 55 m/s (~776 N) than the trimmed cruise drag (~1.1 kN), so
    // the aircraft could not hold its book cruise speed.
    double t_max_n = 2200.0;
    double v_max_mps = 85.0;

    // Constant sea-level density; the model does not apply an ISA lapse.
    double rho = 1.225;
    double gravity = 9.80665;
};

// Instantaneous aerodynamic observables of the last substep, exposed so
// tests can verify stall/CL-cap behavior without reaching into internals.
struct FixedWingAeroDebug {
    double airspeed_mps = 0.0;
    double alpha_rad = 0.0;
    double beta_rad = 0.0;
    double cl = 0.0;
    double cd = 0.0;
};

// Full 6-DOF rigid-body fixed wing (Cessna-172 class).
//
// World frame (repo convention): X east, Y up, Z north. yaw_rad is measured
// on the XZ plane from +X toward +Z, so heading = (cos yaw, 0, sin yaw);
// yaw = pi/2 points north (+Z). pitch_rad is positive nose-up and roll_rad
// is positive right-wing-down.
//
// Internally the dynamics are integrated in a standard NED nav frame
// (n = +Z_world, e = +X_world, d = -Y_world) with an aircraft body frame
// (x fwd, y right, z down): quaternion attitude, body velocity (u, v, w)
// and body rates (p, q, r), RK4 at the 0.02 s substep, quaternion
// renormalized every substep. The internal state is authoritative between
// consecutive step() calls; if the caller passes an EntityState that does
// not match the previous output (external reset), the rigid-body state is
// re-derived from it with zero body rates.
//
// Generic Actuation mapping (interface compatibility only):
//   input.throttle  -> throttle (clamped to [0, 1])
//   input.steer_rad -> aileron  (clamped to [-1, 1])
// Elevator/rudder stay at their last set_controls() values. Calling
// set_controls() switches the model to the full FixedWingControls until
// clear_controls() is called.
class FixedWingModel final : public IVehicleModel {
public:
    FixedWingModel();
    explicit FixedWingModel(const agbot::config::ParamTable& params);

    EntityState step(const EntityState& state, const Actuation& input, double dt_s) override;

    [[nodiscard]] const VehicleLimits& limits() const override { return limits_; }
    [[nodiscard]] VehicleKind kind() const override { return VehicleKind::FixedWing; }
    [[nodiscard]] std::string name() const override { return "fixed_wing"; }

    void set_controls(const FixedWingControls& controls);
    void clear_controls();
    [[nodiscard]] const FixedWingControls& controls() const { return controls_; }

    // Initialize the internal rigid-body state in steady level cruise at the
    // given altitude/airspeed/heading (repo yaw convention). Returns the
    // matching EntityState; trim_controls() then holds the throttle/elevator
    // that balance drag and pitch moment at that condition.
    EntityState set_initial_trim(
        double altitude_m,
        double airspeed_mps,
        double heading_rad,
        double x_m = 0.0,
        double z_m = 0.0);
    [[nodiscard]] const FixedWingControls& trim_controls() const { return trim_controls_; }

    // Body angular rates (p, q, r) in rad/s (aircraft body axes).
    [[nodiscard]] Vec3 body_rates() const;
    [[nodiscard]] const FixedWingAeroDebug& aero_debug() const { return aero_debug_; }
    [[nodiscard]] const FixedWingParams& params() const { return params_; }

private:
    struct RigidBodyState {
        double pos[3] = {0.0, 0.0, 0.0}; // NED position (n, e, d)
        double quat[4] = {1.0, 0.0, 0.0, 0.0}; // nav->body attitude quaternion
        double vel[3] = {0.0, 0.0, 0.0}; // body velocity (u, v, w)
        double rate[3] = {0.0, 0.0, 0.0}; // body rates (p, q, r)
        double time_s = 0.0;
    };

    void derivatives(
        const RigidBodyState& state,
        const FixedWingControls& controls,
        double* out) const;
    void substep(RigidBodyState& state, const FixedWingControls& controls, double dt_s);
    void sync_internal_from_entity(const EntityState& state);
    [[nodiscard]] EntityState entity_from_internal() const;
    [[nodiscard]] bool matches_last_output(const EntityState& state) const;

    FixedWingParams params_;
    VehicleLimits limits_ = {85.0, 6.0, 6.0, 0.5236, 1.5, 0.0};
    FixedWingControls controls_;
    FixedWingControls trim_controls_;
    bool use_direct_controls_ = false;
    std::optional<RigidBodyState> internal_;
    EntityState last_output_;
    FixedWingAeroDebug aero_debug_;
};

} // namespace agbot::vehicles
