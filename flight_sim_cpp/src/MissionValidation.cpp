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

std::optional<Vec3> safety_position_for_waypoint(
    const Mission& mission,
    const Waypoint& waypoint,
    const std::optional<TerrainMesh>& terrain,
    const std::optional<double>& home_ground_elevation_m) {
    if (!terrain.has_value()) {
        if (mission.altitude_reference ==
            AltitudeReference::MeanSeaLevel) {
            return std::nullopt;
        }
        return waypoint.position;
    }

    const auto ground_elevation = terrain_height_at(
        *terrain, waypoint.position.x, waypoint.position.z);
    if (!ground_elevation.has_value() ||
        !home_ground_elevation_m.has_value()) {
        return std::nullopt;
    }

    Vec3 safety_position = waypoint.position;
    if (waypoint.action == WaypointAction::Land) {
        safety_position.y = 0.0;
    } else {
        switch (mission.altitude_reference) {
            case AltitudeReference::AboveGroundLevel:
                safety_position.y = waypoint.position.y;
                break;
            case AltitudeReference::RelativeHome:
                safety_position.y =
                    waypoint.position.y -
                    (*ground_elevation -
                     *home_ground_elevation_m);
                break;
            case AltitudeReference::MeanSeaLevel:
                safety_position.y =
                    waypoint.position.y - *ground_elevation;
                break;
        }
    }
    return safety_position;
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

    if (config.terrain.has_value()) {
        report.terrain_policy = "runtime_terrain";
        report.terrain_gap_count = 0;
    } else if (const auto bounds = terrain_bounds_for_mission(mission)) {
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

    std::optional<double> home_ground_elevation_m;
    if (config.terrain.has_value()) {
        home_ground_elevation_m = terrain_height_at(
            *config.terrain, mission.home.x, mission.home.z);
        if (!home_ground_elevation_m.has_value()) {
            report.issues.push_back({
                "terrain_unavailable",
                "blocker",
                0,
                "runtime terrain does not cover the mission home",
            });
            report.blocked = true;
        }
    }

    for (std::size_t index = 0; index < mission.waypoints.size(); ++index) {
        const Waypoint& waypoint = mission.waypoints[index];
        const auto safety_position = safety_position_for_waypoint(
            mission,
            waypoint,
            config.terrain,
            home_ground_elevation_m);
        if (!safety_position.has_value()) {
            report.issues.push_back({
                "terrain_unavailable",
                "blocker",
                index,
                config.terrain.has_value()
                    ? "runtime terrain does not cover the waypoint"
                    : "MSL altitude validation requires runtime terrain",
            });
            report.blocked = true;
            continue;
        }
        const SafetySample sample{
            *safety_position,
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
