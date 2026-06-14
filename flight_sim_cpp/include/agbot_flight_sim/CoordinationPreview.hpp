#pragma once

#include "agbot_flight_sim/Mission.hpp"
#include "agbot_flight_sim/Vec3.hpp"

#include <cstddef>
#include <string>
#include <vector>

namespace agbot::flight_sim {

enum class CoordinationPreviewStatus {
    WaitingForApproval,
    Passed,
    SeparationBreach,
    InvalidPlan,
    TwinExecutionFailed,
};

struct CoordinationPreviewConfig {
    bool approval_required = true;
    bool operator_approved = false;
    double min_separation_m = 25.0;
    double step_s = 0.25;
    double max_duration_s = 120.0;
    Vec3 wind_mps;
    std::string coordination_source = "multi_drone_control";
};

struct CoordinationPreviewDrone {
    std::string drone_id;
    Mission mission;
};

struct CoordinationTelemetrySample {
    std::string drone_id;
    double time_s = 0.0;
    Vec3 position;
    std::string mode;
    double battery_percent = 0.0;
    std::size_t target_waypoint_index = 0;
};

struct CoordinationSeparationSample {
    double time_s = 0.0;
    std::string left_drone_id;
    std::string right_drone_id;
    double separation_m = 0.0;
    bool violation = false;
};

struct CoordinationPreviewReport {
    std::string contract_version;
    std::string coordination_source;
    CoordinationPreviewStatus status = CoordinationPreviewStatus::WaitingForApproval;
    bool permitted = false;
    bool safe_to_execute = false;
    double min_required_separation_m = 0.0;
    double min_observed_separation_m = 0.0;
    std::size_t breach_count = 0;
    double duration_s = 0.0;
    std::string deterministic_run_id;
    std::vector<CoordinationTelemetrySample> telemetry_samples;
    std::vector<CoordinationSeparationSample> separation_samples;
    std::string message;

    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] const char* to_string(CoordinationPreviewStatus status);

[[nodiscard]] CoordinationPreviewReport run_coordination_preview(
    const std::vector<CoordinationPreviewDrone>& drones,
    CoordinationPreviewConfig config = {});

} // namespace agbot::flight_sim
