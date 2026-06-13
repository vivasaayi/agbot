#pragma once

#include "agbot_flight_sim/Mission.hpp"
#include "agbot_flight_sim/SafetyRules.hpp"

#include <cstddef>
#include <string>
#include <vector>

namespace agbot::flight_sim {

struct MissionValidationIssue {
    std::string code;
    std::string severity;
    std::size_t waypoint_index = 0;
    std::string message;
};

struct MissionValidationConfig {
    SafetyEnvelope safety;
    double flight_battery_drain_percent_per_s = 0.012;
    double idle_battery_drain_percent_per_s = 0.001;
    int terrain_resolution = 96;
};

struct MissionValidationReport {
    std::string mission_name;
    std::size_t waypoint_count = 0;
    double coverage_fraction = 0.0;
    double estimated_duration_s = 0.0;
    double estimated_battery_used_percent = 0.0;
    double battery_margin_percent = 0.0;
    std::size_t terrain_gap_count = 0;
    std::string terrain_policy = "not_georeferenced";
    bool blocked = false;
    std::vector<MissionValidationIssue> issues;

    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] MissionValidationReport validate_mission(
    const Mission& mission,
    const MissionValidationConfig& config = {});

} // namespace agbot::flight_sim
