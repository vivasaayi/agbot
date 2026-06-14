#include "agbot_flight_sim/TwinBackend.hpp"

#include "agbot_flight_sim/TwinContractV1.hpp"

#include <algorithm>
#include <sstream>
#include <stdexcept>
#include <string_view>
#include <utility>

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

void write_vec3_json(std::ostringstream& output, const Vec3& value) {
    output << "{\"x\":" << value.x
           << ",\"y\":" << value.y
           << ",\"z\":" << value.z
           << "}";
}

TwinCommandAckV1 accepted_ack(const FlightCommandV1& command, TelemetryV1 telemetry) {
    TwinCommandAckV1 ack;
    ack.command_id = command.command_id;
    ack.accepted = true;
    ack.telemetry = std::move(telemetry);
    return ack;
}

} // namespace

const char* to_string(TwinCommandType command_type) {
    switch (command_type) {
        case TwinCommandType::Arm:
            return "arm";
        case TwinCommandType::Disarm:
            return "disarm";
        case TwinCommandType::Step:
            return "step";
        case TwinCommandType::SetManualInput:
            return "set_manual_input";
        case TwinCommandType::SetWind:
            return "set_wind";
        case TwinCommandType::Abort:
            return "abort";
    }
    return "unknown";
}

std::string TwinErrorV1::to_json() const {
    std::ostringstream output;
    output << "{\"contract_version\":\"" << escape_json(contract_version) << "\""
           << ",\"code\":\"" << escape_json(code) << "\""
           << ",\"message\":\"" << escape_json(message) << "\""
           << ",\"retryable\":" << (retryable ? "true" : "false")
           << "}";
    return output.str();
}

std::string TelemetryV1::to_json() const {
    std::ostringstream output;
    output << "{\"contract_version\":\"" << escape_json(contract_version) << "\""
           << ",\"command_id\":\"" << escape_json(command_id) << "\""
           << ",\"time_s\":" << time_s
           << ",\"mode\":\"" << escape_json(mode) << "\""
           << ",\"position\":";
    write_vec3_json(output, position);
    output << ",\"velocity\":";
    write_vec3_json(output, velocity);
    output << ",\"attitude\":";
    write_vec3_json(output, attitude);
    output << ",\"battery_percent\":" << battery_percent
           << ",\"target_waypoint_index\":" << target_waypoint_index
           << ",\"armed\":" << (armed ? "true" : "false")
           << "}";
    return output.str();
}

std::string TwinCommandAckV1::to_json() const {
    std::ostringstream output;
    output << "{\"contract_version\":\"" << escape_json(contract_version) << "\""
           << ",\"command_id\":\"" << escape_json(command_id) << "\""
           << ",\"accepted\":" << (accepted ? "true" : "false")
           << ",\"error\":";
    if (error.has_value()) {
        output << error->to_json();
    } else {
        output << "null";
    }
    output << ",\"telemetry\":";
    if (telemetry.has_value()) {
        output << telemetry->to_json();
    } else {
        output << "null";
    }
    output << "}";
    return output.str();
}

TwinBackend::TwinBackend(Mission mission, TwinBackendConfig config, SimulationConfig simulation_config)
    : simulation_(std::move(mission), simulation_config), config_(config) {}

bool TwinBackend::available() const {
    return config_.available;
}

void TwinBackend::set_available(bool available) {
    config_.available = available;
}

const DroneSimulation& TwinBackend::simulation() const {
    return simulation_;
}

TwinCommandAckV1 TwinBackend::dispatch(const FlightCommandV1& command) {
    if (!config_.available) {
        return reject(
            command,
            "twin_unavailable",
            "simulation runtime mode requires the canonical flight_sim_cpp twin backend",
            true);
    }
    if (!is_compatible_contract_version(kTwinContractVersion, command.contract_version)) {
        return reject(
            command,
            "contract_version_unsupported",
            "flight command contract version is not compatible with the twin backend",
            false);
    }
    if (command.command_id.empty()) {
        return reject(command, "invalid_command", "command_id is required", false);
    }

    try {
        switch (command.command_type) {
            case TwinCommandType::Arm:
                simulation_.arm();
                break;
            case TwinCommandType::Disarm:
                simulation_.disarm();
                break;
            case TwinCommandType::Step:
                if (command.step_duration_s <= 0.0 || command.step_duration_s > config_.max_command_step_s) {
                    return reject(command, "invalid_step_duration", "step duration is outside the accepted range", false);
                }
                simulation_.step(command.step_duration_s);
                break;
            case TwinCommandType::SetManualInput:
                simulation_.set_control_mode(ControlMode::Manual);
                simulation_.set_manual_input(command.manual_input);
                if (command.step_duration_s > 0.0) {
                    simulation_.step(std::min(command.step_duration_s, config_.max_command_step_s));
                }
                break;
            case TwinCommandType::SetWind:
                simulation_.set_wind(command.wind_mps);
                break;
            case TwinCommandType::Abort:
                simulation_.request_emergency_abort();
                simulation_.step(0.05);
                break;
        }
    } catch (const std::exception& error) {
        return reject(command, "simulation_execution_failed", error.what(), false);
    }

    return accepted_ack(command, telemetry_for(command));
}

TwinCommandAckV1 TwinBackend::reject(
    const FlightCommandV1& command,
    std::string code,
    std::string message,
    bool retryable) const {
    TwinCommandAckV1 ack;
    ack.command_id = command.command_id;
    ack.accepted = false;
    ack.error = TwinErrorV1 {
        kTwinContractVersion,
        std::move(code),
        std::move(message),
        retryable,
    };
    return ack;
}

TelemetryV1 TwinBackend::telemetry_for(const FlightCommandV1& command) const {
    const auto& state = simulation_.state();
    TelemetryV1 telemetry;
    telemetry.command_id = command.command_id;
    telemetry.time_s = state.mission_time_s;
    telemetry.mode = to_string(state.mode);
    telemetry.position = state.position;
    telemetry.velocity = state.velocity;
    telemetry.attitude = {state.roll_rad, state.pitch_rad, state.yaw_rad};
    telemetry.battery_percent = state.battery_percent;
    telemetry.target_waypoint_index = state.target_waypoint_index;
    telemetry.armed = state.armed;
    return telemetry;
}

} // namespace agbot::flight_sim
