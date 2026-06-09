#include "agbot_flight_sim/MissionLoader.hpp"

#include <cmath>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <regex>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

using agbot::flight_sim::Mission;
using agbot::flight_sim::MissionLoader;
using agbot::flight_sim::Vec3;
using agbot::flight_sim::Waypoint;
using agbot::flight_sim::WaypointAction;

namespace {

constexpr double kEarthMetersPerDegreeLat = 111'320.0;
constexpr double kPi = 3.14159265358979323846;

struct GeoPoint {
    double latitude = 0.0;
    double longitude = 0.0;
    double altitude = 0.0;
};

std::string read_all(const std::filesystem::path& path) {
    std::ifstream file(path);
    if (!file) {
        throw std::runtime_error("Unable to open input mission: " + path.string());
    }
    std::ostringstream buffer;
    buffer << file.rdbuf();
    return buffer.str();
}

std::string string_for_key(const std::string& text, const std::string& key, const std::string& fallback) {
    const std::regex pattern("\"" + key + "\"\\s*:\\s*\"([^\"]*)\"");
    std::smatch match;
    if (std::regex_search(text, match, pattern) && match.size() >= 2) {
        return match[1].str();
    }
    return fallback;
}

std::vector<GeoPoint> extract_geo_points(const std::string& text) {
    const std::regex point_pattern(
        "\"latitude\"\\s*:\\s*(-?[0-9.]+)\\s*,\\s*"
        "\"longitude\"\\s*:\\s*(-?[0-9.]+)\\s*,\\s*"
        "\"altitude\"\\s*:\\s*(-?[0-9.]+)"
    );

    std::vector<GeoPoint> points;
    for (std::sregex_iterator it(text.begin(), text.end(), point_pattern), end; it != end; ++it) {
        points.push_back({
            std::stod((*it)[1].str()),
            std::stod((*it)[2].str()),
            std::stod((*it)[3].str()),
        });
    }
    return points;
}

Vec3 local_from_geo(const GeoPoint& point, const GeoPoint& origin) {
    const double meters_per_degree_lon =
        kEarthMetersPerDegreeLat * std::cos(origin.latitude * kPi / 180.0);
    return {
        (point.longitude - origin.longitude) * meters_per_degree_lon,
        point.altitude - origin.altitude,
        (point.latitude - origin.latitude) * kEarthMetersPerDegreeLat,
    };
}

struct Args {
    std::filesystem::path input = std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR).parent_path()
        / "mission_planner" / "samples" / "sample_mission.json";
    std::filesystem::path output = std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR)
        / "out" / "bridged_mission.json";
};

Args parse_args(int argc, char** argv) {
    Args args;
    for (int index = 1; index < argc; ++index) {
        const std::string current = argv[index];
        if (current == "--input" && index + 1 < argc) {
            args.input = argv[++index];
        } else if (current == "--output" && index + 1 < argc) {
            args.output = argv[++index];
        } else if (current == "--help" || current == "-h") {
            std::cout << "Usage: agbot_mission_bridge [--input mission.json] [--output flight_sim.json]\n";
            std::exit(0);
        } else {
            throw std::runtime_error("Unknown argument: " + current);
        }
    }
    return args;
}

} // namespace

int main(int argc, char** argv) {
    try {
        const Args args = parse_args(argc, argv);
        const std::string text = read_all(args.input);
        const std::vector<GeoPoint> points = extract_geo_points(text);
        if (points.size() < 2) {
            throw std::runtime_error("Input mission must contain home_position plus at least one waypoint position");
        }

        const GeoPoint home = points.front();
        Mission mission;
        mission.name = string_for_key(text, "name", "Bridged Rust Mission");
        mission.home = {0.0, 0.0, 0.0};
        mission.cruise_speed_mps = 8.0;
        mission.acceptance_radius_m = 2.0;

        Waypoint takeoff;
        takeoff.name = "takeoff";
        takeoff.action = WaypointAction::Takeoff;
        takeoff.position = {0.0, 25.0, 0.0};
        takeoff.speed_mps = 5.0;
        mission.waypoints.push_back(takeoff);

        for (std::size_t index = 1; index < points.size(); ++index) {
            Waypoint waypoint;
            waypoint.name = "rust_waypoint_" + std::to_string(index);
            waypoint.action = WaypointAction::FlyThrough;
            waypoint.position = local_from_geo(points[index], home);
            waypoint.position.y = std::max(25.0, waypoint.position.y + 25.0);
            mission.waypoints.push_back(waypoint);
        }

        Waypoint land;
        land.name = "land_home";
        land.action = WaypointAction::Land;
        land.position = {0.0, 0.0, 0.0};
        land.speed_mps = 4.0;
        mission.waypoints.push_back(land);

        MissionLoader::save_to_file(mission, args.output);
        std::cout << "Wrote FlightSim mission: " << args.output << "\n";
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "agbot_mission_bridge: " << error.what() << "\n";
        return 1;
    }
}
