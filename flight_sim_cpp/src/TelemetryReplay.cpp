#include "agbot_flight_sim/TelemetryReplay.hpp"

#include <fstream>
#include <sstream>
#include <stdexcept>

namespace agbot::flight_sim {
namespace {

std::size_t find_key(const std::string& text, const std::string& key, std::size_t start = 0) {
    const std::string quoted_key = "\"" + key + "\"";
    const std::size_t position = text.find(quoted_key, start);
    if (position == std::string::npos) {
        throw std::runtime_error("Telemetry JSON is missing key: " + key);
    }
    return position;
}

double number_for_key(const std::string& text, const std::string& key, std::size_t start = 0) {
    const std::size_t key_position = find_key(text, key, start);
    const std::size_t colon = text.find(':', key_position);
    if (colon == std::string::npos) {
        throw std::runtime_error("Telemetry JSON key has no value: " + key);
    }
    std::size_t parsed = 0;
    return std::stod(text.substr(colon + 1), &parsed);
}

DroneMode mode_from_line(const std::string& line) {
    if (line.find("\"mode\":\"takeoff\"") != std::string::npos) {
        return DroneMode::Takeoff;
    }
    if (line.find("\"mode\":\"flying\"") != std::string::npos) {
        return DroneMode::Flying;
    }
    if (line.find("\"mode\":\"loiter\"") != std::string::npos) {
        return DroneMode::Loiter;
    }
    if (line.find("\"mode\":\"landing\"") != std::string::npos) {
        return DroneMode::Landing;
    }
    if (line.find("\"mode\":\"completed\"") != std::string::npos) {
        return DroneMode::Completed;
    }
    if (line.find("\"mode\":\"failsafe\"") != std::string::npos) {
        return DroneMode::Failsafe;
    }
    return DroneMode::Idle;
}

TelemetryFrame parse_frame(const std::string& line) {
    TelemetryFrame frame;
    frame.state.mission_time_s = number_for_key(line, "time_s");
    frame.state.mode = mode_from_line(line);
    frame.state.position.x = number_for_key(line, "x", find_key(line, "position"));
    frame.state.position.y = number_for_key(line, "y", find_key(line, "position"));
    frame.state.position.z = number_for_key(line, "z", find_key(line, "position"));
    frame.state.velocity.x = number_for_key(line, "x", find_key(line, "velocity"));
    frame.state.velocity.y = number_for_key(line, "y", find_key(line, "velocity"));
    frame.state.velocity.z = number_for_key(line, "z", find_key(line, "velocity"));
    frame.state.yaw_rad = number_for_key(line, "yaw_rad");
    frame.state.pitch_rad = number_for_key(line, "pitch_rad");
    frame.state.roll_rad = number_for_key(line, "roll_rad");
    frame.state.battery_percent = number_for_key(line, "battery_percent");
    frame.state.control_mode = ControlMode::Replay;
    return frame;
}

} // namespace

TelemetryReplay TelemetryReplay::load_jsonl(const std::filesystem::path& path) {
    std::ifstream file(path);
    if (!file) {
        throw std::runtime_error("Unable to open telemetry replay: " + path.string());
    }

    TelemetryReplay replay;
    std::string line;
    while (std::getline(file, line)) {
        if (!line.empty()) {
            replay.frames_.push_back(parse_frame(line));
        }
    }
    return replay;
}

bool TelemetryReplay::empty() const {
    return frames_.empty();
}

const std::vector<TelemetryFrame>& TelemetryReplay::frames() const {
    return frames_;
}

const DroneState& TelemetryReplay::sample(double time_s) const {
    if (frames_.empty()) {
        throw std::runtime_error("Telemetry replay has no frames");
    }

    for (const TelemetryFrame& frame : frames_) {
        if (frame.state.mission_time_s >= time_s) {
            return frame.state;
        }
    }
    return frames_.back().state;
}

double TelemetryReplay::duration_s() const {
    if (frames_.empty()) {
        return 0.0;
    }
    return frames_.back().state.mission_time_s;
}

} // namespace agbot::flight_sim
