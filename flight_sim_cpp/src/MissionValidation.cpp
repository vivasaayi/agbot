#include "agbot_flight_sim/MissionValidation.hpp"

#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_flight_sim/MissionPreview.hpp"

#include <algorithm>
#include <cmath>
#include <iomanip>
#include <sstream>
#include <string_view>

namespace agbot::flight_sim {
namespace {

std::string escape_json(std::string_view value) {
    std::ostringstream output;
    for (const char c : value) {
        switch (c) {
            case '"':
                output << "\\\"";
                break;
            case '\\':
                output << "\\\\";
                break;
            case '\n':
                output << "\\n";
                break;
            case '\r':
                output << "\\r";
                break;
            case '\t':
                output << "\\t";
                break;
            default:
                output << c;
                break;
        }
    }
    return output.str();
}

double estimate_duration_s(const Mission& mission) {
    Vec3 current = mission.home;
    double duration_s = 0.0;
    for (const Waypoint& waypoint : mission.waypoints) {
        const double speed = std::max(0.1, waypoint.speed_mps.value_or(mission.cruise_speed_mps));
        duration_s += (waypoint.position - current).length() / speed;
        duration_s += std::max(0.0, waypoint.hold_seconds);
        current = waypoint.position;
    }
    return duration_s;
}

MissionValidationIssue issue_for_violation(const SafetyViolation& violation) {
    return MissionValidationIssue{
        to_string(violation.code),
        "blocker",
        violation.waypoint_index,
        violation.message,
    };
}

} // namespace

std::string MissionValidationReport::to_json() const {
    std::ostringstream output;
    output << std::fixed << std::setprecision(6)
           << "{\"mission_name\":\"" << escape_json(mission_name) << "\""
           << ",\"waypoint_count\":" << waypoint_count
           << ",\"coverage_fraction\":" << coverage_fraction
           << ",\"estimated_duration_s\":" << estimated_duration_s
           << ",\"estimated_battery_used_percent\":" << estimated_battery_used_percent
           << ",\"battery_margin_percent\":" << battery_margin_percent
           << ",\"terrain_gap_count\":" << terrain_gap_count
           << ",\"terrain_policy\":\"" << escape_json(terrain_policy) << "\""
           << ",\"blocked\":" << (blocked ? "true" : "false")
           << ",\"issues\":[";
    for (std::size_t index = 0; index < issues.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        const MissionValidationIssue& issue = issues[index];
        output << "{\"code\":\"" << escape_json(issue.code) << "\""
               << ",\"severity\":\"" << escape_json(issue.severity) << "\""
               << ",\"waypoint_index\":" << issue.waypoint_index
               << ",\"message\":\"" << escape_json(issue.message) << "\"}";
    }
    output << "]}";
    return output.str();
}

MissionValidationReport validate_mission(
    const Mission& mission,
    const MissionValidationConfig& config) {
    MissionValidationReport report;
    report.mission_name = mission.name;
    report.waypoint_count = mission.waypoints.size();
    report.coverage_fraction = build_mission_preview_overlay(mission).coverage_fraction;
    report.estimated_duration_s = estimate_duration_s(mission);
    report.estimated_battery_used_percent =
        (report.estimated_duration_s * config.flight_battery_drain_percent_per_s)
        + (static_cast<double>(mission.waypoints.size()) * config.idle_battery_drain_percent_per_s);
    report.battery_margin_percent =
        100.0 - report.estimated_battery_used_percent - config.safety.min_battery_percent;

    if (const auto bounds = terrain_bounds_for_mission(mission)) {
        const auto tiles = terrain_tiles_for_bounds_limited(*bounds, bounds->width_m());
        report.terrain_gap_count = tiles.size();
        report.terrain_policy = report.terrain_gap_count == 0 ? "available" : "runnable_with_gaps";
        if (report.terrain_gap_count > 0) {
            report.issues.push_back({
                "terrain_flat_fallback",
                "warning",
                0,
                "mission uses georeferenced terrain with flat_fallback tile gaps",
            });
        }
    } else {
        report.terrain_policy = "not_georeferenced";
    }

    for (std::size_t index = 0; index < mission.waypoints.size(); ++index) {
        const Waypoint& waypoint = mission.waypoints[index];
        const SafetySample sample{
            waypoint.position,
            100.0 - report.estimated_battery_used_percent,
            index,
            false,
        };
        if (const auto violation = evaluate_safety(sample, config.safety)) {
            report.issues.push_back(issue_for_violation(*violation));
            report.blocked = true;
        }
    }
    if (report.battery_margin_percent <= 0.0) {
        report.issues.push_back({
            "battery_margin",
            "blocker",
            0,
            "estimated battery margin is at or below zero",
        });
        report.blocked = true;
    }

    return report;
}

} // namespace agbot::flight_sim
