#include "agbot_flight_sim/SafetyRules.hpp"

#include <algorithm>
#include <cmath>
#include <sstream>
#include <utility>

namespace agbot::flight_sim {
namespace {

SafetyViolation make_violation(
    SafetyViolationCode code,
    std::string rule_id,
    std::size_t waypoint_index,
    std::string message) {
    return {code, std::move(rule_id), waypoint_index, std::move(message)};
}

double horizontal_distance_m(Vec3 a, Vec3 b) {
    const double dx = a.x - b.x;
    const double dz = a.z - b.z;
    return std::sqrt(dx * dx + dz * dz);
}

} // namespace

const char* to_string(SafetyViolationCode code) {
    switch (code) {
        case SafetyViolationCode::GeofenceViolation:
            return "geofence_violation";
        case SafetyViolationCode::AltitudeCeilingViolation:
            return "altitude_ceiling_violation";
        case SafetyViolationCode::NoFlyZoneViolation:
            return "no_fly_zone_violation";
        case SafetyViolationCode::LowBatteryAbort:
            return "low_battery_abort";
        case SafetyViolationCode::EmergencyAbort:
            return "emergency_abort";
    }
    return "unknown_safety_violation";
}

std::optional<SafetyViolation> evaluate_safety(
    const SafetySample& sample,
    const SafetyEnvelope& envelope) {
    if (sample.emergency_abort) {
        return make_violation(
            SafetyViolationCode::EmergencyAbort,
            "emergency_abort",
            sample.target_waypoint_index,
            "Emergency abort requested");
    }

    if (sample.battery_percent <= envelope.min_battery_percent) {
        std::ostringstream message;
        message << "Battery " << sample.battery_percent
                << "% is at or below abort threshold " << envelope.min_battery_percent << "%";
        return make_violation(
            SafetyViolationCode::LowBatteryAbort,
            "battery_minimum",
            sample.target_waypoint_index,
            message.str());
    }

    if (sample.position.y > envelope.max_altitude_m) {
        std::ostringstream message;
        message << "Altitude " << sample.position.y
                << "m exceeds ceiling " << envelope.max_altitude_m << "m";
        return make_violation(
            SafetyViolationCode::AltitudeCeilingViolation,
            "altitude_ceiling",
            sample.target_waypoint_index,
            message.str());
    }

    if (sample.position.x < envelope.min_x_m || sample.position.x > envelope.max_x_m ||
        sample.position.z < envelope.min_z_m || sample.position.z > envelope.max_z_m) {
        std::ostringstream message;
        message << "Position (" << sample.position.x << ", " << sample.position.z
                << ") is outside geofence";
        return make_violation(
            SafetyViolationCode::GeofenceViolation,
            "geofence_bounds",
            sample.target_waypoint_index,
            message.str());
    }

    for (const CircularNoFlyZone& zone : envelope.no_fly_zones) {
        if (horizontal_distance_m(sample.position, zone.center) <= zone.radius_m) {
            std::ostringstream message;
            message << "Position entered no-fly zone " << zone.id;
            return make_violation(
                SafetyViolationCode::NoFlyZoneViolation,
                zone.id,
                sample.target_waypoint_index,
                message.str());
        }
    }

    return std::nullopt;
}

std::vector<SafetyParityCase> default_safety_parity_cases() {
    SafetyEnvelope geofence;
    geofence.min_x_m = -5.0;
    geofence.max_x_m = 5.0;
    geofence.min_z_m = -5.0;
    geofence.max_z_m = 5.0;

    SafetyEnvelope altitude;
    altitude.max_altitude_m = 20.0;

    SafetyEnvelope no_fly;
    no_fly.no_fly_zones.push_back({"nfz-test", Vec3(0.0, 0.0, 0.0), 10.0});

    SafetyEnvelope battery;
    battery.min_battery_percent = 25.0;

    SafetyEnvelope emergency;

    return {
        {"geofence", geofence, {Vec3(6.0, 2.0, 0.0), 100.0, 1, false}, SafetyViolationCode::GeofenceViolation},
        {"altitude", altitude, {Vec3(0.0, 21.0, 0.0), 100.0, 1, false}, SafetyViolationCode::AltitudeCeilingViolation},
        {"no_fly_zone", no_fly, {Vec3(3.0, 2.0, 4.0), 100.0, 1, false}, SafetyViolationCode::NoFlyZoneViolation},
        {"battery", battery, {Vec3(0.0, 2.0, 0.0), 24.0, 1, false}, SafetyViolationCode::LowBatteryAbort},
        {"emergency_abort", emergency, {Vec3(0.0, 2.0, 0.0), 100.0, 1, true}, SafetyViolationCode::EmergencyAbort},
    };
}

std::vector<SafetyViolationCode> missing_required_safety_coverage(
    const std::vector<SafetyParityCase>& cases) {
    const std::vector<SafetyViolationCode> required {
        SafetyViolationCode::GeofenceViolation,
        SafetyViolationCode::AltitudeCeilingViolation,
        SafetyViolationCode::NoFlyZoneViolation,
        SafetyViolationCode::LowBatteryAbort,
        SafetyViolationCode::EmergencyAbort,
    };

    std::vector<SafetyViolationCode> missing;
    for (const SafetyViolationCode required_code : required) {
        const auto found = std::find_if(cases.begin(), cases.end(), [required_code](const SafetyParityCase& test_case) {
            return test_case.expected_code == required_code;
        });
        if (found == cases.end()) {
            missing.push_back(required_code);
        }
    }
    return missing;
}

} // namespace agbot::flight_sim
