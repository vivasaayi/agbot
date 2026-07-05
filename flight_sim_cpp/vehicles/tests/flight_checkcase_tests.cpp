// M7 batch 1 — fixed-wing flight-dynamics validation (Gate 6).
//
// Runs the standard NASA-6DOF-style check cases, asserts each maneuver's
// analytic acceptance predicates, and requires byte-identical deterministic
// replay (S-119 log hash) across two runs.

#include "agbot_vehicles/FixedWingModel.hpp"
#include "agbot_vehicles/FlightCheckCase.hpp"
#include "agbot_vehicles/FlightRecorder.hpp"

#include <cmath>
#include <iostream>
#include <map>
#include <string>

namespace {

int failures = 0;

void expect(bool condition, const std::string& label) {
    std::cout << (condition ? "PASS " : "FAIL ") << label << "\n";
    if (!condition) {
        ++failures;
    }
}

constexpr double kPi = 3.14159265358979323846;
constexpr double kDeg = kPi / 180.0;

using namespace agbot::vehicles;

void dump(const FlightCheckResult& r) {
    std::cout << "  [" << r.name << "] alt " << r.initial.altitudeMsl_m << "->"
              << r.final_record.altitudeMsl_m << " m, dev " << r.max_altitude_dev_m
              << ", V [" << r.min_true_airspeed_mps << "," << r.max_true_airspeed_mps << "]"
              << ", maxAoA " << r.max_angle_of_attack_rad / kDeg << " deg, maxBank "
              << r.max_abs_bank_rad / kDeg << " deg, maxSideslip " << r.max_abs_sideslip_rad / kDeg
              << " deg, dHeading " << r.heading_change_rad / kDeg << " deg, minAlt "
              << r.min_altitude_m << "\n";
}

} // namespace

int main() {
    const auto cases = standard_check_cases();
    std::map<std::string, FlightCheckResult> results;
    for (const auto& c : cases) {
        results[c.name] = run_flight_check_case(c);
        dump(results[c.name]);
    }

    // Determinism (Gate 6 core): identical case => identical S-119 log hash.
    for (const auto& c : cases) {
        const FlightCheckResult again = run_flight_check_case(c);
        expect(again.log_hash == results[c.name].log_hash && again.log_hash != 0,
               "Gate 6: '" + c.name + "' replays byte-identically (log hash)");
    }

    // S-119 record surface.
    expect(flight_record_csv_header().find("angleOfAttack") != std::string::npos &&
               flight_record_csv_header().find("bodyAngularRate_p") != std::string::npos,
           "S-119 CSV header carries standard variable names");
    expect(!flight_record_csv_row(results.at("trimmed_cruise").final_record).empty(),
           "S-119 CSV row serializes");

    // Per-maneuver acceptance.
    {
        const auto& r = results.at("trimmed_cruise");
        expect(r.max_altitude_dev_m < 20.0, "trimmed cruise holds altitude within 20 m");
        expect(std::abs(r.max_true_airspeed_mps - r.initial.trueAirspeed_mps) < 3.0 &&
                   std::abs(r.min_true_airspeed_mps - r.initial.trueAirspeed_mps) < 3.0,
               "trimmed cruise holds airspeed within 3 m/s");
        expect(std::abs(r.heading_change_rad) < 5.0 * kDeg,
               "trimmed cruise holds heading within 5 deg");
    }
    {
        const auto& r = results.at("banked_turn");
        expect(r.max_abs_bank_rad > 15.0 * kDeg, "banked turn achieves > 15 deg of bank");
        expect(std::abs(r.heading_change_rad) > 20.0 * kDeg,
               "banked turn changes heading > 20 deg");
    }
    {
        const auto& r = results.at("climb");
        expect(r.final_record.altitudeMsl_m > r.initial.altitudeMsl_m + 30.0,
               "climb gains > 30 m altitude");
        expect(r.max_altitude_m > r.initial.altitudeMsl_m + 100.0,
               "climb sustains a strong climb (> 100 m gained)");
    }
    {
        const auto& r = results.at("descent");
        expect(r.final_record.altitudeMsl_m < r.initial.altitudeMsl_m - 30.0,
               "descent loses > 30 m altitude");
    }
    {
        const auto& r = results.at("crosswind");
        expect(r.max_abs_sideslip_rad > 2.0 * kDeg,
               "crosswind induces sideslip (> 2 deg) on the air-relative velocity");
    }
    {
        const auto& r = results.at("stall_recovery");
        FixedWingModel ref;
        const double alpha_stall = ref.params().alpha_stall_rad;
        expect(r.max_angle_of_attack_rad > alpha_stall,
               "stall: angle of attack exceeds the stall onset");
        expect(r.min_true_airspeed_mps < 32.0, "stall: airspeed decays into the stall regime");
        expect(r.min_altitude_m < r.initial.altitudeMsl_m - 50.0,
               "stall: altitude drops after the stall");
        expect(r.final_record.trueAirspeed_mps > r.min_true_airspeed_mps + 3.0,
               "stall recovery: airspeed recovers after power + release");
    }

    if (failures > 0) {
        std::cout << failures << " failure(s)\n";
        return 1;
    }
    std::cout << "all flight check-case tests passed (Gate 6)\n";
    return 0;
}
