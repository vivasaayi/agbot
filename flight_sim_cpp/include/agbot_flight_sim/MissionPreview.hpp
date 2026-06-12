#pragma once

#include "agbot_flight_sim/Mission.hpp"

#include <string>
#include <vector>

namespace agbot::flight_sim {

struct MissionPreviewOverlay {
    bool has_boundary = false;
    double coverage_fraction = 0.0;
    std::string status = "no boundary";
    std::vector<GeoCoordinate> boundary_geo;
    std::vector<Vec3> boundary_local;
    std::vector<Vec3> mission_path_local;
};

[[nodiscard]] MissionPreviewOverlay build_mission_preview_overlay(const Mission& mission);

} // namespace agbot::flight_sim
