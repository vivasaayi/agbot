#pragma once

#include "agbot_flight_sim/DroneSimulation.hpp"

#include <filesystem>
#include <fstream>
#include <string>

namespace agbot::flight_sim {

/// Serialize a single telemetry sample to its canonical JSON-object string
/// (no trailing newline). This is the single source of truth for telemetry
/// line formatting, shared by TelemetryRecorder and the deterministic runner,
/// so that recorded traces and golden fixtures can never diverge in format.
[[nodiscard]] std::string format_telemetry_sample(const DroneState& state);

class TelemetryRecorder {
public:
    explicit TelemetryRecorder(const std::filesystem::path& output_path);
    ~TelemetryRecorder();

    void write_sample(const DroneState& state);
    void close();
    [[nodiscard]] bool is_open() const;

private:
    std::ofstream stream_;
};

} // namespace agbot::flight_sim
