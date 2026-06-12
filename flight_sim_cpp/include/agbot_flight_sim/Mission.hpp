#pragma once

#include "agbot_flight_sim/Vec3.hpp"

#include <optional>
#include <string>
#include <vector>

namespace agbot::flight_sim {

struct GeoCoordinate {
    double latitude = 0.0;
    double longitude = 0.0;
    double altitude_m = 0.0;
};

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
    std::optional<GeoCoordinate> geo;
    WaypointAction action = WaypointAction::FlyThrough;
    std::optional<double> speed_mps;
    double hold_seconds = 0.0;
};

struct FieldBoundary {
    std::string field_id;
    std::vector<GeoCoordinate> coordinates;
};

struct Mission {
    std::string name = "Untitled Flight";
    Vec3 home;
    std::optional<GeoCoordinate> home_geo;
    double cruise_speed_mps = 8.0;
    double acceptance_radius_m = 2.0;
    std::optional<FieldBoundary> field_boundary;
    std::vector<Waypoint> waypoints;
};

[[nodiscard]] Vec3 local_from_geo(const GeoCoordinate& coordinate, const GeoCoordinate& origin);
[[nodiscard]] GeoCoordinate geo_from_local(const Vec3& local_position, const GeoCoordinate& origin);

[[nodiscard]] const char* to_string(WaypointAction action);
[[nodiscard]] WaypointAction waypoint_action_from_string(const std::string& value);
[[nodiscard]] std::string mission_to_json(const Mission& mission);

} // namespace agbot::flight_sim
