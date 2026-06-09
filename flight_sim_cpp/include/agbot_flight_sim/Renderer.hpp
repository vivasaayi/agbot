#pragma once

#include "agbot_flight_sim/DroneSimulation.hpp"

#include <string>
#include <vector>

namespace agbot::flight_sim {

struct RenderFrame {
    Mission mission;
    DroneState drone;
    std::vector<Vec3> trail;
    Vec3 wind_mps;
    double progress = 0.0;
    std::string status_text;
};

class Renderer {
public:
    virtual ~Renderer() = default;
    virtual void render(const RenderFrame& frame) = 0;
};

} // namespace agbot::flight_sim
