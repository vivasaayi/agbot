#pragma once

#include "agbot_vehicles/FixedWingModel.hpp"
#include "agbot_vehicles/VehicleTypes.hpp"

#include <cstdint>
#include <string>
#include <vector>

// AIAA/ANSI S-119 "Flight Dynamics Model Exchange Standard" flavored logging.
// The record fields carry S-119 variable-naming discipline (trueAirspeed,
// angleOfAttack, angleOfSideslip, eulerAngle_phi/theta/psi, bodyAngularRate_
// p/q/r, ...) so recorded trajectories are portable and comparable across runs
// and, in principle, against reference check-case data.
namespace agbot::vehicles {

// One time step of named flight state. Angles in radians, distances in metres,
// speeds in m/s. Comment on each line gives the S-119 variable name.
struct FlightRecord {
    double time_s = 0.0;               // time
    double altitudeMsl_m = 0.0;        // geodeticAltitude (world Y up)
    double posEast_m = 0.0;            // world X
    double posNorth_m = 0.0;           // world Z
    double trueAirspeed_mps = 0.0;     // trueAirspeed
    double groundSpeed_mps = 0.0;      // groundSpeed (horizontal)
    double climbRate_mps = 0.0;        // altitudeRate (world Y rate)
    double angleOfAttack_rad = 0.0;    // angleOfAttack
    double angleOfSideslip_rad = 0.0;  // angleOfSideslip
    double eulerAngle_phi_rad = 0.0;   // eulerAngle_phi (roll)
    double eulerAngle_theta_rad = 0.0; // eulerAngle_theta (pitch)
    double eulerAngle_psi_rad = 0.0;   // eulerAngle_psi (repo yaw)
    double bodyAngularRate_p_radps = 0.0; // bodyAngularRate_p (roll rate)
    double bodyAngularRate_q_radps = 0.0; // bodyAngularRate_q (pitch rate)
    double bodyAngularRate_r_radps = 0.0; // bodyAngularRate_r (yaw rate)
    double coefficientOfLift = 0.0;    // CL
    double coefficientOfDrag = 0.0;    // CD
};

// Capture a record from the model's current aero observables + the entity state.
[[nodiscard]] FlightRecord capture_flight_record(const FixedWingModel& model,
                                                 const EntityState& state);

// S-119-named CSV header and one deterministically formatted row.
[[nodiscard]] std::string flight_record_csv_header();
[[nodiscard]] std::string flight_record_csv_row(const FlightRecord& record);

// FNV1a-64 over quantized channels (mm / mrad / milli-units). Two logs with the
// same quantized trajectory hash equal — the primitive behind Gate 6's
// deterministic-replay check.
[[nodiscard]] std::uint64_t flight_log_hash(const std::vector<FlightRecord>& log);

} // namespace agbot::vehicles
