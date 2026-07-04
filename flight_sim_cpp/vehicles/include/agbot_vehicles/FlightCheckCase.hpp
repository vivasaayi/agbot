#pragma once

#include "agbot_vehicles/FixedWingModel.hpp"
#include "agbot_vehicles/FlightRecorder.hpp"

#include <cstdint>
#include <string>
#include <vector>

// Deterministic fixed-wing check cases in the spirit of NASA's 6-DOF
// flight-simulation verification cases: fixed initial condition + a scripted,
// open-loop control-surface time history, integrated at a fixed step, producing
// an S-119 trajectory log. Replaying the same case yields a byte-identical log
// hash (Gate 6), and each case carries analytic acceptance predicates.
namespace agbot::vehicles {

// Piecewise-constant control *delta from trim* applied from t_s until the next
// keyframe. The harness holds the trim controls before the first keyframe and
// adds each keyframe's fields to the trim controls (then clamps), so e.g.
// throttle = -1 forces idle and elevator = -0.2 is a nose-up pull from trim.
struct ControlKeyframe {
    double t_s = 0.0;
    FixedWingControls controls;
};

struct FlightCheckCase {
    std::string name;
    double altitude_m = 300.0;
    double airspeed_mps = 55.0;
    double heading_rad = 0.0;
    Vec3 wind_mps{0.0, 0.0, 0.0};
    double duration_s = 30.0;
    double dt_s = 0.02;
    // Control timeline (sorted by t_s). When empty, the trim controls hold. When
    // the first keyframe is at t>0, trim holds until it. Between keyframes the
    // most recent one's controls are held.
    std::vector<ControlKeyframe> inputs;
};

struct FlightCheckResult {
    std::string name;
    std::vector<FlightRecord> log;
    std::uint64_t log_hash = 0;
    FlightRecord initial;
    FlightRecord final_record;
    // Summary metrics over the run.
    double max_altitude_dev_m = 0.0;   // |altitude - initial altitude| peak
    double min_true_airspeed_mps = 0.0;
    double max_true_airspeed_mps = 0.0;
    double max_angle_of_attack_rad = 0.0;
    double max_abs_bank_rad = 0.0;
    double max_abs_sideslip_rad = 0.0;
    double heading_change_rad = 0.0;   // signed final - initial (wrapped)
    double min_altitude_m = 0.0;
    double max_altitude_m = 0.0;
};

// Run a check case to completion. Deterministic: identical case => identical
// result (log_hash included).
[[nodiscard]] FlightCheckResult run_flight_check_case(const FlightCheckCase& c);

// The standard acceptance pack: trimmed cruise, banked turn, climb, descent,
// crosswind response, stall entry/recovery.
[[nodiscard]] std::vector<FlightCheckCase> standard_check_cases();

} // namespace agbot::vehicles
