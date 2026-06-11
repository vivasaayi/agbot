#include "agbot_flight_sim/TelemetryRecorder.hpp"

#include <iomanip>
#include <sstream>
#include <stdexcept>

namespace agbot::flight_sim {

std::string format_telemetry_sample(const DroneState& state) {
    std::ostringstream stream;
    stream << std::fixed << std::setprecision(3)
           << "{\"time_s\":" << state.mission_time_s
           << ",\"mode\":\"" << to_string(state.mode) << "\""
           << ",\"position\":{\"x\":" << state.position.x
           << ",\"y\":" << state.position.y
           << ",\"z\":" << state.position.z << "}"
           << ",\"velocity\":{\"x\":" << state.velocity.x
           << ",\"y\":" << state.velocity.y
           << ",\"z\":" << state.velocity.z << "}"
           << ",\"yaw_rad\":" << state.yaw_rad
           << ",\"pitch_rad\":" << state.pitch_rad
           << ",\"roll_rad\":" << state.roll_rad
           << ",\"battery_percent\":" << state.battery_percent
           << ",\"target_waypoint_index\":" << state.target_waypoint_index
           << "}";
    return stream.str();
}

TelemetryRecorder::TelemetryRecorder(const std::filesystem::path& output_path) {
    if (!output_path.parent_path().empty()) {
        std::filesystem::create_directories(output_path.parent_path());
    }

    stream_.open(output_path);
    if (!stream_) {
        throw std::runtime_error("Unable to open telemetry output: " + output_path.string());
    }
}

TelemetryRecorder::~TelemetryRecorder() {
    close();
}

void TelemetryRecorder::write_sample(const DroneState& state) {
    if (!stream_) {
        throw std::runtime_error("Telemetry recorder is not open");
    }

    stream_ << format_telemetry_sample(state) << "\n";
}

void TelemetryRecorder::close() {
    if (stream_.is_open()) {
        stream_.flush();
        stream_.close();
    }
}

bool TelemetryRecorder::is_open() const {
    return stream_.is_open();
}

} // namespace agbot::flight_sim
