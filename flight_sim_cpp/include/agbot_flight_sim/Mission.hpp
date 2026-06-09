#pragma once

#include "agbot_flight_sim/Vec3.hpp"

#include <optional>
#include <string>
#include <vector>

namespace agbot::flight_sim {

enum class WaypointAction {
    Takeoff,
    FlyThrough,
    Loiter,
    Land,
    ReturnHome,
};

struct Waypoint {
    std::string name;
    Vec3 position;
    WaypointAction action = WaypointAction::FlyThrough;
    std::optional<double> speed_mps;
    double hold_seconds = 0.0;
};

struct Mission {
    std::string name = "Untitled Flight";
    Vec3 home;
    double cruise_speed_mps = 8.0;
    double acceptance_radius_m = 2.0;
    std::vector<Waypoint> waypoints;
};

[[nodiscard]] const char* to_string(WaypointAction action);
[[nodiscard]] WaypointAction waypoint_action_from_string(const std::string& value);

} // namespace agbot::flight_sim
