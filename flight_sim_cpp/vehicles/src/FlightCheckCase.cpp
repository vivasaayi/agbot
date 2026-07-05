#include "agbot_vehicles/FlightCheckCase.hpp"

#include <algorithm>
#include <cmath>

namespace agbot::vehicles {

namespace {

constexpr double kPi = 3.14159265358979323846;

double wrap_pi(double a) {
    while (a > kPi) {
        a -= 2.0 * kPi;
    }
    while (a < -kPi) {
        a += 2.0 * kPi;
    }
    return a;
}

// Active controls at time t: trim plus the most recent keyframe's delta.
FixedWingControls active_controls(const FixedWingControls& trim,
                                  const std::vector<ControlKeyframe>& inputs, double t_s) {
    FixedWingControls c = trim;
    for (const ControlKeyframe& kf : inputs) {
        if (kf.t_s <= t_s + 1e-9) {
            c.throttle = trim.throttle + kf.controls.throttle;
            c.elevator = trim.elevator + kf.controls.elevator;
            c.aileron = trim.aileron + kf.controls.aileron;
            c.rudder = trim.rudder + kf.controls.rudder;
        } else {
            break; // inputs are sorted by t_s
        }
    }
    return c;
}

} // namespace

FlightCheckResult run_flight_check_case(const FlightCheckCase& c) {
    FlightCheckResult r;
    r.name = c.name;

    FixedWingModel model;
    EntityState state = model.set_initial_trim(c.altitude_m, c.airspeed_mps, c.heading_rad);
    model.set_wind(c.wind_mps);
    const FixedWingControls trim = model.trim_controls();

    r.initial = capture_flight_record(model, state);
    r.log.push_back(r.initial);
    r.min_true_airspeed_mps = r.initial.trueAirspeed_mps;
    r.max_true_airspeed_mps = r.initial.trueAirspeed_mps;
    r.min_altitude_m = r.initial.altitudeMsl_m;
    r.max_altitude_m = r.initial.altitudeMsl_m;

    const int steps = std::max(1, static_cast<int>(std::llround(c.duration_s / c.dt_s)));
    for (int i = 0; i < steps; ++i) {
        const double t = static_cast<double>(i) * c.dt_s;
        model.set_controls(active_controls(trim, c.inputs, t));
        state = model.step(state, Actuation{}, c.dt_s);
        const FlightRecord rec = capture_flight_record(model, state);
        r.log.push_back(rec);
        r.max_altitude_dev_m =
            std::max(r.max_altitude_dev_m, std::abs(rec.altitudeMsl_m - r.initial.altitudeMsl_m));
        r.min_true_airspeed_mps = std::min(r.min_true_airspeed_mps, rec.trueAirspeed_mps);
        r.max_true_airspeed_mps = std::max(r.max_true_airspeed_mps, rec.trueAirspeed_mps);
        r.max_angle_of_attack_rad = std::max(r.max_angle_of_attack_rad, rec.angleOfAttack_rad);
        r.max_abs_bank_rad = std::max(r.max_abs_bank_rad, std::abs(rec.eulerAngle_phi_rad));
        r.max_abs_sideslip_rad =
            std::max(r.max_abs_sideslip_rad, std::abs(rec.angleOfSideslip_rad));
        r.min_altitude_m = std::min(r.min_altitude_m, rec.altitudeMsl_m);
        r.max_altitude_m = std::max(r.max_altitude_m, rec.altitudeMsl_m);
    }

    r.final_record = r.log.back();
    r.heading_change_rad =
        wrap_pi(r.final_record.eulerAngle_psi_rad - r.initial.eulerAngle_psi_rad);
    r.log_hash = flight_log_hash(r.log);
    return r;
}

std::vector<FlightCheckCase> standard_check_cases() {
    std::vector<FlightCheckCase> cases;

    // 1. Trimmed cruise: hold trim, expect it to stay put.
    {
        FlightCheckCase c;
        c.name = "trimmed_cruise";
        c.altitude_m = 300.0;
        c.airspeed_mps = 55.0;
        c.heading_rad = 0.0;
        c.duration_s = 30.0;
        cases.push_back(c);
    }
    // 2. Banked turn: roll in with aileron, then neutralize and hold the bank.
    {
        FlightCheckCase c;
        c.name = "banked_turn";
        c.altitude_m = 300.0;
        c.airspeed_mps = 55.0;
        c.heading_rad = 0.0;
        c.duration_s = 20.0;
        c.inputs = {
            {0.0, FixedWingControls{0.0, 0.0, 0.35, 0.0}}, // roll right
            {2.5, FixedWingControls{0.0, 0.0, 0.0, 0.0}},  // neutralize; hold bank
        };
        cases.push_back(c);
    }
    // 3. Climb: add power and pull the nose up.
    {
        FlightCheckCase c;
        c.name = "climb";
        c.altitude_m = 300.0;
        c.airspeed_mps = 50.0;
        c.heading_rad = 0.0;
        c.duration_s = 30.0;
        c.inputs = {{0.0, FixedWingControls{0.5, -0.15, 0.0, 0.0}}}; // +power, nose up
        cases.push_back(c);
    }
    // 4. Descent: idle power, slight nose down.
    {
        FlightCheckCase c;
        c.name = "descent";
        c.altitude_m = 500.0;
        c.airspeed_mps = 55.0;
        c.heading_rad = 0.0;
        c.duration_s = 30.0;
        c.inputs = {{0.0, FixedWingControls{-1.0, 0.05, 0.0, 0.0}}}; // idle, nose down
        cases.push_back(c);
    }
    // 5. Crosswind: trimmed flight into a steady crosswind (from the north,
    //    perpendicular to the eastbound heading) develops sideslip.
    {
        FlightCheckCase c;
        c.name = "crosswind";
        c.altitude_m = 300.0;
        c.airspeed_mps = 55.0;
        c.heading_rad = 0.0;             // eastbound (+X)
        c.wind_mps = {0.0, 0.0, 12.0};   // 12 m/s along +Z (crosswind)
        c.duration_s = 10.0;
        cases.push_back(c);
    }
    // 6. Stall entry + recovery: idle + held nose-up to stall, then recover with
    //    power and neutral elevator.
    {
        FlightCheckCase c;
        c.name = "stall_recovery";
        c.altitude_m = 1000.0;
        c.airspeed_mps = 55.0;
        c.heading_rad = 0.0;
        c.duration_s = 30.0;
        c.inputs = {
            {0.0, FixedWingControls{-1.0, -0.30, 0.0, 0.0}}, // idle, hold nose up -> stall
            {15.0, FixedWingControls{0.5, 0.0, 0.0, 0.0}},   // recover: power, release
        };
        cases.push_back(c);
    }
    return cases;
}

} // namespace agbot::vehicles
