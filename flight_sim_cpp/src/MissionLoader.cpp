#include "agbot_flight_sim/MissionLoader.hpp"

#include <algorithm>
#include <cctype>
#include <fstream>
#include <sstream>
#include <stdexcept>

namespace agbot::flight_sim {
namespace {

std::string read_all(const std::filesystem::path& path) {
    std::ifstream file(path);
    if (!file) {
        throw std::runtime_error("Unable to open mission file: " + path.string());
    }

    std::ostringstream buffer;
    buffer << file.rdbuf();
    return buffer.str();
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

Vec3 vec3_from_object(const std::string& object) {
    return {
        number_for_key(object, "x"),
        number_for_key(object, "y"),
        number_for_key(object, "z"),
    };
}

Waypoint waypoint_from_object(const std::string& object) {
    Waypoint waypoint;
    waypoint.name = string_for_key(object, "name", "waypoint");
    waypoint.position = vec3_from_object(object);
    waypoint.action = waypoint_action_from_string(string_for_key(object, "action", "fly"));
    waypoint.speed_mps = optional_number_for_key(object, "speed_mps");
    waypoint.hold_seconds = optional_number_for_key(object, "hold_seconds", 0.0);
    return waypoint;
}

} // namespace

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
    mission.home = vec3_from_object(object_for_key(text, "home"));
    mission.cruise_speed_mps = optional_number_for_key(text, "cruise_speed_mps", mission.cruise_speed_mps);
    mission.acceptance_radius_m = optional_number_for_key(text, "acceptance_radius_m", mission.acceptance_radius_m);

    const std::vector<std::string> waypoint_objects = top_level_objects(array_for_key(text, "waypoints"));
    if (waypoint_objects.empty()) {
        throw std::runtime_error("Mission must contain at least one waypoint");
    }

    mission.waypoints.reserve(waypoint_objects.size());
    for (const std::string& object : waypoint_objects) {
        mission.waypoints.push_back(waypoint_from_object(object));
    }
    return mission;
}

std::filesystem::path default_sample_mission_path() {
    return std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "samples" / "sample_field_loop.json";
}

} // namespace agbot::flight_sim
