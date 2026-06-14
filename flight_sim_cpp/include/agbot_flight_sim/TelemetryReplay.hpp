#pragma once

#include "agbot_flight_sim/DroneSimulation.hpp"

#include <filesystem>
#include <vector>

namespace agbot::flight_sim {

struct TelemetryFrame {
    DroneState state;
};

class TelemetryReplay {
public:
    [[nodiscard]] static TelemetryReplay load_jsonl(const std::filesystem::path& path);

    [[nodiscard]] bool empty() const;
    [[nodiscard]] const std::vector<TelemetryFrame>& frames() const;
    [[nodiscard]] const DroneState& sample(double time_s) const;
    [[nodiscard]] double duration_s() const;

private:
    std::vector<TelemetryFrame> frames_;
};

} // namespace agbot::flight_sim
