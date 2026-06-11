#pragma once

#include "agbot_flight_sim/Vec3.hpp"

#include <cstddef>
#include <limits>
#include <optional>
#include <string>
#include <vector>

namespace agbot::flight_sim {

enum class SafetyViolationCode {
    GeofenceViolation,
    AltitudeCeilingViolation,
    NoFlyZoneViolation,
    LowBatteryAbort,
    EmergencyAbort,
};

struct CircularNoFlyZone {
    std::string id = "no_fly_zone";
    Vec3 center;
    double radius_m = 0.0;
};

struct SafetyEnvelope {
    double min_x_m = -std::numeric_limits<double>::infinity();
    double max_x_m = std::numeric_limits<double>::infinity();
    double min_z_m = -std::numeric_limits<double>::infinity();
    double max_z_m = std::numeric_limits<double>::infinity();
    double max_altitude_m = std::numeric_limits<double>::infinity();
    double min_battery_percent = 12.0;
    std::vector<CircularNoFlyZone> no_fly_zones;
};

struct SafetySample {
    Vec3 position;
    double battery_percent = 100.0;
    std::size_t target_waypoint_index = 0;
    bool emergency_abort = false;
};

struct SafetyViolation {
    SafetyViolationCode code = SafetyViolationCode::EmergencyAbort;
    std::string rule_id;
    std::size_t waypoint_index = 0;
    std::string message;
};

struct SafetyParityCase {
    std::string name;
    SafetyEnvelope envelope;
    SafetySample sample;
    SafetyViolationCode expected_code = SafetyViolationCode::EmergencyAbort;
};

[[nodiscard]] const char* to_string(SafetyViolationCode code);
[[nodiscard]] std::optional<SafetyViolation> evaluate_safety(
    const SafetySample& sample,
    const SafetyEnvelope& envelope);
[[nodiscard]] std::vector<SafetyParityCase> default_safety_parity_cases();
[[nodiscard]] std::vector<SafetyViolationCode> missing_required_safety_coverage(
    const std::vector<SafetyParityCase>& cases);

} // namespace agbot::flight_sim
