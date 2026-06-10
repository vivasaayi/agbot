#pragma once

#include "agbot_flight_sim/DroneSimulation.hpp"

#include <filesystem>
#include <fstream>

namespace agbot::flight_sim {

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
