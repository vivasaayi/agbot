#include "agbot_flight_sim/CoordinationPreview.hpp"

#include "agbot_flight_sim/DeterministicRunner.hpp"
#include "agbot_flight_sim/TwinBackend.hpp"
#include "agbot_flight_sim/TwinContractV1.hpp"

#include <algorithm>
#include <cmath>
#include <iomanip>
#include <limits>
#include <memory>
#include <sstream>
#include <string_view>
#include <utility>

namespace agbot::flight_sim {
namespace {

struct PreviewTwin {
    std::string drone_id;
    TwinBackend backend;
};

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

void write_double(std::ostringstream& output, double value) {
    if (std::isfinite(value)) {
        output << std::fixed << std::setprecision(6) << value;
    } else {
        output << "null";
    }
}

void write_vec3_json(std::ostringstream& output, const Vec3& value) {
    output << "{\"x\":";
    write_double(output, value.x);
    output << ",\"y\":";
    write_double(output, value.y);
    output << ",\"z\":";
    write_double(output, value.z);
    output << "}";
}

bool valid_config(const CoordinationPreviewConfig& config) {
    return std::isfinite(config.min_separation_m)
        && config.min_separation_m > 0.0
        && std::isfinite(config.step_s)
        && config.step_s > 0.0
        && std::isfinite(config.max_duration_s)
        && config.max_duration_s > 0.0;
}

std::string run_id_material(
    const std::vector<CoordinationPreviewDrone>& drones,
    const CoordinationPreviewConfig& config) {
    std::ostringstream stream;
    stream << std::fixed << std::setprecision(9)
           << kTwinContractVersion << "|"
           << config.approval_required << "|"
           << config.operator_approved << "|"
           << config.min_separation_m << "|"
           << config.step_s << "|"
           << config.max_duration_s << "|"
           << config.wind_mps.x << ","
           << config.wind_mps.y << ","
           << config.wind_mps.z << "|"
           << config.coordination_source;
    for (const auto& drone : drones) {
        stream << "|" << drone.drone_id << "|" << mission_to_json(drone.mission);
    }
    return stream.str();
}

CoordinationPreviewReport base_report(
    const std::vector<CoordinationPreviewDrone>& drones,
    const CoordinationPreviewConfig& config) {
    CoordinationPreviewReport report;
    report.contract_version = kTwinContractVersion;
    report.coordination_source = config.coordination_source;
    report.min_required_separation_m = config.min_separation_m;
    report.deterministic_run_id = sha256_hex(run_id_material(drones, config));
    return report;
}

TwinCommandAckV1 dispatch_or_fail(
    PreviewTwin& twin,
    TwinCommandType type,
    std::string command_suffix,
    double step_s,
    Vec3 wind_mps = {}) {
    FlightCommandV1 command;
    command.command_id = twin.drone_id + ":" + std::move(command_suffix);
    command.command_type = type;
    command.step_duration_s = step_s;
    command.wind_mps = wind_mps;
    return twin.backend.dispatch(command);
}

CoordinationTelemetrySample telemetry_sample(const PreviewTwin& twin) {
    const auto& state = twin.backend.simulation().state();
    return {
        twin.drone_id,
        state.mission_time_s,
        state.position,
        to_string(state.mode),
        state.battery_percent,
        state.target_waypoint_index,
    };
}

void append_telemetry_samples(
    const std::vector<std::unique_ptr<PreviewTwin>>& twins,
    CoordinationPreviewReport& report) {
    for (const auto& twin : twins) {
        report.telemetry_samples.push_back(telemetry_sample(*twin));
    }
}

void append_separation_samples(
    const std::vector<std::unique_ptr<PreviewTwin>>& twins,
    CoordinationPreviewConfig config,
    CoordinationPreviewReport& report) {
    for (std::size_t left = 0; left < twins.size(); ++left) {
        for (std::size_t right = left + 1; right < twins.size(); ++right) {
            const auto& left_twin = *twins[left];
            const auto& right_twin = *twins[right];
            const Vec3 delta = left_twin.backend.simulation().state().position
                - right_twin.backend.simulation().state().position;
            const double separation = delta.length();
            const bool violation = separation < config.min_separation_m;
            report.min_observed_separation_m = std::min(report.min_observed_separation_m, separation);
            if (violation) {
                ++report.breach_count;
            }
            report.separation_samples.push_back({
                std::max(
                    left_twin.backend.simulation().state().mission_time_s,
                    right_twin.backend.simulation().state().mission_time_s),
                left_twin.drone_id,
                right_twin.drone_id,
                separation,
                violation,
            });
        }
    }
}

bool all_twins_complete(const std::vector<std::unique_ptr<PreviewTwin>>& twins) {
    return std::all_of(twins.begin(), twins.end(), [](const auto& twin) {
        return twin->backend.simulation().is_complete();
    });
}

double max_mission_time(const std::vector<std::unique_ptr<PreviewTwin>>& twins) {
    double value = 0.0;
    for (const auto& twin : twins) {
        value = std::max(value, twin->backend.simulation().state().mission_time_s);
    }
    return value;
}

} // namespace

const char* to_string(CoordinationPreviewStatus status) {
    switch (status) {
        case CoordinationPreviewStatus::WaitingForApproval:
            return "waiting_for_approval";
        case CoordinationPreviewStatus::Passed:
            return "passed";
        case CoordinationPreviewStatus::SeparationBreach:
            return "separation_breach";
        case CoordinationPreviewStatus::InvalidPlan:
            return "invalid_plan";
        case CoordinationPreviewStatus::TwinExecutionFailed:
            return "twin_execution_failed";
    }
    return "unknown";
}

std::string CoordinationPreviewReport::to_json() const {
    std::ostringstream output;
    output << "{"
           << "\"contract_version\":\"" << escape_json(contract_version) << "\""
           << ",\"coordination_source\":\"" << escape_json(coordination_source) << "\""
           << ",\"status\":\"" << to_string(status) << "\""
           << ",\"permitted\":" << (permitted ? "true" : "false")
           << ",\"safe_to_execute\":" << (safe_to_execute ? "true" : "false")
           << ",\"min_required_separation_m\":";
    write_double(output, min_required_separation_m);
    output << ",\"min_observed_separation_m\":";
    write_double(output, min_observed_separation_m);
    output << ",\"breach_count\":" << breach_count
           << ",\"duration_s\":";
    write_double(output, duration_s);
    output << ",\"deterministic_run_id\":\"" << escape_json(deterministic_run_id) << "\""
           << ",\"message\":\"" << escape_json(message) << "\""
           << ",\"telemetry_samples\":[";
    for (std::size_t index = 0; index < telemetry_samples.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        const auto& sample = telemetry_samples[index];
        output << "{\"drone_id\":\"" << escape_json(sample.drone_id) << "\""
               << ",\"time_s\":";
        write_double(output, sample.time_s);
        output << ",\"position\":";
        write_vec3_json(output, sample.position);
        output << ",\"mode\":\"" << escape_json(sample.mode) << "\""
               << ",\"battery_percent\":";
        write_double(output, sample.battery_percent);
        output << ",\"target_waypoint_index\":" << sample.target_waypoint_index
               << "}";
    }
    output << "],\"separation_samples\":[";
    for (std::size_t index = 0; index < separation_samples.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        const auto& sample = separation_samples[index];
        output << "{\"time_s\":";
        write_double(output, sample.time_s);
        output << ",\"left_drone_id\":\"" << escape_json(sample.left_drone_id) << "\""
               << ",\"right_drone_id\":\"" << escape_json(sample.right_drone_id) << "\""
               << ",\"separation_m\":";
        write_double(output, sample.separation_m);
        output << ",\"violation\":" << (sample.violation ? "true" : "false")
               << "}";
    }
    output << "]}";
    return output.str();
}

CoordinationPreviewReport run_coordination_preview(
    const std::vector<CoordinationPreviewDrone>& drones,
    CoordinationPreviewConfig config) {
    CoordinationPreviewReport report = base_report(drones, config);
    report.min_observed_separation_m = std::numeric_limits<double>::infinity();

    if (!valid_config(config) || drones.size() < 2) {
        report.status = CoordinationPreviewStatus::InvalidPlan;
        report.message = "coordination preview requires at least two drones and finite positive timing/separation config";
        report.min_observed_separation_m = 0.0;
        return report;
    }

    if (config.approval_required && !config.operator_approved) {
        report.status = CoordinationPreviewStatus::WaitingForApproval;
        report.message = "coordination preview is approval-gated and disabled until operator approval";
        report.min_observed_separation_m = 0.0;
        return report;
    }

    report.permitted = true;
    std::vector<std::unique_ptr<PreviewTwin>> twins;
    twins.reserve(drones.size());
    for (const auto& drone : drones) {
        twins.push_back(std::make_unique<PreviewTwin>(PreviewTwin {
            drone.drone_id,
            TwinBackend(drone.mission),
        }));
        auto& twin = *twins.back();
        const auto wind_ack = dispatch_or_fail(twin, TwinCommandType::SetWind, "set_wind", 0.0, config.wind_mps);
        const auto arm_ack = dispatch_or_fail(twin, TwinCommandType::Arm, "arm", 0.0);
        if (!wind_ack.accepted || !arm_ack.accepted) {
            report.status = CoordinationPreviewStatus::TwinExecutionFailed;
            report.message = "twin command dispatch failed before coordination preview";
            report.min_observed_separation_m = 0.0;
            return report;
        }
    }

    append_telemetry_samples(twins, report);
    append_separation_samples(twins, config, report);

    std::size_t step_index = 0;
    while (report.breach_count == 0
        && !all_twins_complete(twins)
        && max_mission_time(twins) < config.max_duration_s) {
        for (auto& twin : twins) {
            if (twin->backend.simulation().is_complete()) {
                continue;
            }
            const auto ack = dispatch_or_fail(
                *twin,
                TwinCommandType::Step,
                "step_" + std::to_string(step_index),
                config.step_s);
            if (!ack.accepted) {
                report.status = CoordinationPreviewStatus::TwinExecutionFailed;
                report.message = "twin step command failed during coordination preview";
                report.duration_s = max_mission_time(twins);
                if (!std::isfinite(report.min_observed_separation_m)) {
                    report.min_observed_separation_m = 0.0;
                }
                return report;
            }
        }
        ++step_index;
        report.duration_s = max_mission_time(twins);
        append_telemetry_samples(twins, report);
        append_separation_samples(twins, config, report);
    }

    report.duration_s = max_mission_time(twins);
    if (report.breach_count > 0) {
        report.status = CoordinationPreviewStatus::SeparationBreach;
        report.safe_to_execute = false;
        report.message = "coordination preview blocked by minimum separation breach";
    } else if (all_twins_complete(twins)) {
        report.status = CoordinationPreviewStatus::Passed;
        report.safe_to_execute = true;
        report.message = "coordination preview passed minimum separation across all twin instances";
    } else {
        report.status = CoordinationPreviewStatus::TwinExecutionFailed;
        report.safe_to_execute = false;
        report.message = "coordination preview exceeded max duration before all twin instances completed";
    }
    if (!std::isfinite(report.min_observed_separation_m)) {
        report.min_observed_separation_m = 0.0;
    }
    return report;
}

} // namespace agbot::flight_sim
