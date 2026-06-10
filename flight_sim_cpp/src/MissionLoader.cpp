#include "agbot_flight_sim/MissionLoader.hpp"

#include <algorithm>
#include <cctype>
#include <cmath>
#include <fstream>
#include <iomanip>
#include <initializer_list>
#include <sstream>
#include <stdexcept>

namespace agbot::flight_sim {
namespace {

constexpr double kPi = 3.14159265358979323846;
constexpr double kEarthMetersPerDegreeLat = 111'320.0;

std::string read_all(const std::filesystem::path& path) {
    std::ifstream file(path);
    if (!file) {
        throw std::runtime_error("Unable to open mission file: " + path.string());
    }

    std::ostringstream buffer;
    buffer << file.rdbuf();
    return buffer.str();
}

std::string escape_json(const std::string& value) {
    std::ostringstream output;
    for (const char c : value) {
        switch (c) {
            case '"':
                output << "\\\"";
                break;
            case '\\':
                output << "\\\\";
                break;
            case '\n':
                output << "\\n";
                break;
            case '\r':
                output << "\\r";
                break;
            case '\t':
                output << "\\t";
                break;
            default:
                output << c;
                break;
        }
    }
    return output.str();
}

std::size_t find_key(const std::string& text, const std::string& key, std::size_t start = 0) {
    const std::string quoted_key = "\"" + key + "\"";
    const std::size_t position = text.find(quoted_key, start);
    if (position == std::string::npos) {
        throw std::runtime_error("Mission JSON is missing key: " + key);
    }
    return position;
}

std::size_t find_value_start(const std::string& text, const std::string& key, std::size_t start = 0) {
    const std::size_t key_position = find_key(text, key, start);
    const std::size_t colon = text.find(':', key_position);
    if (colon == std::string::npos) {
        throw std::runtime_error("Mission JSON key has no value: " + key);
    }

    std::size_t value_start = colon + 1;
    while (value_start < text.size() && std::isspace(static_cast<unsigned char>(text[value_start])) != 0) {
        ++value_start;
    }
    return value_start;
}

std::size_t matching_delimiter(const std::string& text, std::size_t open_position, char open, char close) {
    int depth = 0;
    bool in_string = false;
    bool escaped = false;

    for (std::size_t index = open_position; index < text.size(); ++index) {
        const char c = text[index];
        if (in_string) {
            if (escaped) {
                escaped = false;
            } else if (c == '\\') {
                escaped = true;
            } else if (c == '"') {
                in_string = false;
            }
            continue;
        }

        if (c == '"') {
            in_string = true;
        } else if (c == open) {
            ++depth;
        } else if (c == close) {
            --depth;
            if (depth == 0) {
                return index;
            }
        }
    }

    throw std::runtime_error("Mission JSON has unmatched delimiter");
}

std::string object_for_key(const std::string& text, const std::string& key) {
    const std::size_t value_start = find_value_start(text, key);
    if (value_start >= text.size() || text[value_start] != '{') {
        throw std::runtime_error("Mission JSON key is not an object: " + key);
    }
    const std::size_t end = matching_delimiter(text, value_start, '{', '}');
    return text.substr(value_start, end - value_start + 1);
}

std::optional<std::string> optional_object_for_key(const std::string& text, const std::string& key) {
    try {
        return object_for_key(text, key);
    } catch (const std::exception&) {
        return std::nullopt;
    }
}

std::string array_for_key(const std::string& text, const std::string& key) {
    const std::size_t value_start = find_value_start(text, key);
    if (value_start >= text.size() || text[value_start] != '[') {
        throw std::runtime_error("Mission JSON key is not an array: " + key);
    }
    const std::size_t end = matching_delimiter(text, value_start, '[', ']');
    return text.substr(value_start, end - value_start + 1);
}

std::vector<std::string> top_level_objects(const std::string& array_text) {
    std::vector<std::string> objects;
    for (std::size_t index = 0; index < array_text.size(); ++index) {
        if (array_text[index] == '{') {
            const std::size_t end = matching_delimiter(array_text, index, '{', '}');
            objects.push_back(array_text.substr(index, end - index + 1));
            index = end;
        }
    }
    return objects;
}

double number_for_key(const std::string& text, const std::string& key) {
    const std::size_t value_start = find_value_start(text, key);
    std::size_t parsed = 0;
    try {
        const double value = std::stod(text.substr(value_start), &parsed);
        if (parsed == 0) {
            throw std::runtime_error("Mission JSON numeric key could not be parsed: " + key);
        }
        return value;
    } catch (const std::exception& error) {
        throw std::runtime_error("Mission JSON numeric key could not be parsed: " + key + " (" + error.what() + ")");
    }
}

double optional_number_for_key(const std::string& text, const std::string& key, double fallback) {
    try {
        return number_for_key(text, key);
    } catch (const std::exception&) {
        return fallback;
    }
}

std::optional<double> optional_number_for_key(const std::string& text, const std::string& key) {
    try {
        return number_for_key(text, key);
    } catch (const std::exception&) {
        return std::nullopt;
    }
}

std::optional<double> optional_number_for_keys(const std::string& text, std::initializer_list<const char*> keys) {
    for (const char* key : keys) {
        if (auto value = optional_number_for_key(text, key)) {
            return value;
        }
    }
    return std::nullopt;
}

std::string string_for_key(const std::string& text, const std::string& key, const std::string& fallback = {}) {
    try {
        const std::size_t value_start = find_value_start(text, key);
        if (value_start >= text.size() || text[value_start] != '"') {
            return fallback;
        }

        std::string result;
        bool escaped = false;
        for (std::size_t index = value_start + 1; index < text.size(); ++index) {
            const char c = text[index];
            if (escaped) {
                result.push_back(c);
                escaped = false;
            } else if (c == '\\') {
                escaped = true;
            } else if (c == '"') {
                return result;
            } else {
                result.push_back(c);
            }
        }
    } catch (const std::exception&) {
        return fallback;
    }

    return fallback;
}

std::optional<Vec3> optional_vec3_from_object(const std::string& object) {
    const auto x = optional_number_for_key(object, "x");
    const auto y = optional_number_for_key(object, "y");
    const auto z = optional_number_for_key(object, "z");
    if (!x || !y || !z) {
        return std::nullopt;
    }
    return Vec3(*x, *y, *z);
}

std::optional<GeoCoordinate> geo_from_object(const std::string& object) {
    const auto latitude = optional_number_for_keys(object, {"latitude", "lat"});
    const auto longitude = optional_number_for_keys(object, {"longitude", "lon", "lng"});
    if (!latitude || !longitude) {
        return std::nullopt;
    }

    return GeoCoordinate {
        *latitude,
        *longitude,
        optional_number_for_keys(object, {"altitude", "altitude_m", "alt"}).value_or(0.0),
    };
}

WaypointAction waypoint_action_from_command(double command) {
    const int mavlink_command = static_cast<int>(std::round(command));
    switch (mavlink_command) {
        case 21:
            return WaypointAction::Land;
        case 22:
            return WaypointAction::Takeoff;
        case 17:
            return WaypointAction::Loiter;
        case 20:
            return WaypointAction::ReturnHome;
        default:
            return WaypointAction::FlyThrough;
    }
}

Waypoint waypoint_from_object(const std::string& object, const Mission& mission) {
    Waypoint waypoint;
    const auto position_object = optional_object_for_key(object, "position");
    const std::string& coordinate_source = position_object.value_or(object);

    waypoint.name = string_for_key(object, "name", "waypoint");
    if (const auto sequence = optional_number_for_key(object, "sequence")) {
        waypoint.name = "waypoint_" + std::to_string(static_cast<int>(*sequence));
    }

    waypoint.geo = geo_from_object(coordinate_source);
    if (waypoint.geo && mission.home_geo) {
        waypoint.position = local_from_geo(*waypoint.geo, *mission.home_geo);
    } else if (const auto position = optional_vec3_from_object(coordinate_source)) {
        waypoint.position = *position;
    } else if (const auto position = optional_vec3_from_object(object)) {
        waypoint.position = *position;
    } else {
        throw std::runtime_error("Waypoint must contain either x/y/z or latitude/longitude coordinates");
    }

    if (const auto command = optional_number_for_key(object, "command")) {
        waypoint.action = waypoint_action_from_command(*command);
    } else {
        waypoint.action = waypoint_action_from_string(string_for_key(object, "action", "fly"));
    }
    waypoint.speed_mps = optional_number_for_key(object, "speed_mps");
    waypoint.hold_seconds = optional_number_for_key(object, "hold_seconds", 0.0);
    return waypoint;
}

} // namespace

Vec3 local_from_geo(const GeoCoordinate& coordinate, const GeoCoordinate& origin) {
    const double meters_per_degree_lon =
        kEarthMetersPerDegreeLat * std::cos(origin.latitude * kPi / 180.0);
    return {
        (coordinate.longitude - origin.longitude) * meters_per_degree_lon,
        coordinate.altitude_m,
        (coordinate.latitude - origin.latitude) * kEarthMetersPerDegreeLat,
    };
}

GeoCoordinate geo_from_local(const Vec3& local_position, const GeoCoordinate& origin) {
    const double meters_per_degree_lon =
        kEarthMetersPerDegreeLat * std::cos(origin.latitude * kPi / 180.0);
    return {
        origin.latitude + (local_position.z / kEarthMetersPerDegreeLat),
        origin.longitude + (local_position.x / meters_per_degree_lon),
        local_position.y,
    };
}

const char* to_string(WaypointAction action) {
    switch (action) {
        case WaypointAction::Takeoff:
            return "takeoff";
        case WaypointAction::FlyThrough:
            return "fly";
        case WaypointAction::Loiter:
            return "loiter";
        case WaypointAction::Land:
            return "land";
        case WaypointAction::ReturnHome:
            return "return_home";
    }
    return "unknown";
}

WaypointAction waypoint_action_from_string(const std::string& value) {
    std::string lower = value;
    std::transform(lower.begin(), lower.end(), lower.begin(), [](unsigned char c) {
        return static_cast<char>(std::tolower(c));
    });

    if (lower == "takeoff") {
        return WaypointAction::Takeoff;
    }
    if (lower == "loiter" || lower == "hover") {
        return WaypointAction::Loiter;
    }
    if (lower == "land" || lower == "landing") {
        return WaypointAction::Land;
    }
    if (lower == "return_home" || lower == "rtl" || lower == "return-to-home") {
        return WaypointAction::ReturnHome;
    }
    return WaypointAction::FlyThrough;
}

Mission MissionLoader::load_from_file(const std::filesystem::path& path) {
    return load_from_text(read_all(path));
}

Mission MissionLoader::load_from_text(const std::string& text) {
    Mission mission;
    mission.name = string_for_key(text, "name", mission.name);

    auto home_object = optional_object_for_key(text, "home");
    if (!home_object) {
        home_object = optional_object_for_key(text, "home_position");
    }
    if (!home_object) {
        throw std::runtime_error("Mission JSON is missing home or home_position");
    }

    mission.home_geo = geo_from_object(*home_object);
    if (const auto home = optional_vec3_from_object(*home_object)) {
        mission.home = *home;
    } else {
        mission.home = {};
    }

    mission.cruise_speed_mps = optional_number_for_key(text, "cruise_speed_mps", mission.cruise_speed_mps);
    mission.acceptance_radius_m = optional_number_for_key(text, "acceptance_radius_m", mission.acceptance_radius_m);

    const std::vector<std::string> waypoint_objects = top_level_objects(array_for_key(text, "waypoints"));
    if (waypoint_objects.empty()) {
        throw std::runtime_error("Mission must contain at least one waypoint");
    }

    mission.waypoints.reserve(waypoint_objects.size());
    for (const std::string& object : waypoint_objects) {
        mission.waypoints.push_back(waypoint_from_object(object, mission));
    }
    return mission;
}

void MissionLoader::save_to_file(const Mission& mission, const std::filesystem::path& path) {
    if (!path.parent_path().empty()) {
        std::filesystem::create_directories(path.parent_path());
    }

    std::ofstream file(path);
    if (!file) {
        throw std::runtime_error("Unable to write mission file: " + path.string());
    }
    file << mission_to_json(mission);
}

std::filesystem::path default_sample_mission_path() {
    return std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "samples" / "sample_field_loop.json";
}

std::string mission_to_json(const Mission& mission) {
    std::ostringstream output;
    output << std::fixed << std::setprecision(3);
    output << "{\n";
    output << "  \"name\": \"" << escape_json(mission.name) << "\",\n";
    output << "  \"home\": {\n";
    if (mission.home_geo) {
        output << "    \"latitude\": " << mission.home_geo->latitude << ",\n";
        output << "    \"longitude\": " << mission.home_geo->longitude << ",\n";
        output << "    \"altitude\": " << mission.home_geo->altitude_m << ",\n";
    }
    output << "    \"x\": " << mission.home.x << ",\n";
    output << "    \"y\": " << mission.home.y << ",\n";
    output << "    \"z\": " << mission.home.z << "\n";
    output << "  },\n";
    output << "  \"cruise_speed_mps\": " << mission.cruise_speed_mps << ",\n";
    output << "  \"acceptance_radius_m\": " << mission.acceptance_radius_m << ",\n";
    output << "  \"waypoints\": [\n";

    for (std::size_t index = 0; index < mission.waypoints.size(); ++index) {
        const Waypoint& waypoint = mission.waypoints[index];
        output << "    {\n";
        output << "      \"name\": \"" << escape_json(waypoint.name) << "\",\n";
        output << "      \"action\": \"" << to_string(waypoint.action) << "\",\n";
        if (mission.home_geo) {
            const GeoCoordinate coordinate = waypoint.geo.value_or(geo_from_local(waypoint.position, *mission.home_geo));
            output << "      \"latitude\": " << coordinate.latitude << ",\n";
            output << "      \"longitude\": " << coordinate.longitude << ",\n";
            output << "      \"altitude\": " << coordinate.altitude_m << ",\n";
        }
        output << "      \"x\": " << waypoint.position.x << ",\n";
        output << "      \"y\": " << waypoint.position.y << ",\n";
        output << "      \"z\": " << waypoint.position.z;
        if (waypoint.speed_mps.has_value()) {
            output << ",\n      \"speed_mps\": " << *waypoint.speed_mps;
        }
        if (waypoint.hold_seconds > 0.0) {
            output << ",\n      \"hold_seconds\": " << waypoint.hold_seconds;
        }
        output << "\n    }";
        if (index + 1 < mission.waypoints.size()) {
            output << ",";
        }
        output << "\n";
    }

    output << "  ]\n";
    output << "}\n";
    return output.str();
}

} // namespace agbot::flight_sim
