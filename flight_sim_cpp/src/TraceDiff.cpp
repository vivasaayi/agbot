#include "agbot_flight_sim/TraceDiff.hpp"

#include <algorithm>
#include <optional>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

namespace agbot::flight_sim {
namespace {

std::vector<std::string> split_lines(std::string_view text) {
    std::vector<std::string> lines;
    std::size_t start = 0;
    while (start < text.size()) {
        const std::size_t end = text.find('\n', start);
        if (end == std::string_view::npos) {
            lines.emplace_back(text.substr(start));
            break;
        }
        if (end > start) {
            lines.emplace_back(text.substr(start, end - start));
        }
        start = end + 1;
    }
    return lines;
}

std::size_t matching_brace(std::string_view text, std::size_t open_position) {
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
        } else if (c == '{') {
            ++depth;
        } else if (c == '}') {
            --depth;
            if (depth == 0) {
                return index;
            }
        }
    }
    return std::string_view::npos;
}

std::optional<std::string> scalar_for_key(std::string_view text, std::string_view key) {
    const std::string token = "\"" + std::string(key) + "\":";
    const std::size_t key_position = text.find(token);
    if (key_position == std::string_view::npos) {
        return std::nullopt;
    }

    std::size_t value_start = key_position + token.size();
    while (value_start < text.size() && text[value_start] == ' ') {
        ++value_start;
    }
    if (value_start >= text.size()) {
        return std::nullopt;
    }

    if (text[value_start] == '"') {
        const std::size_t value_end = text.find('"', value_start + 1);
        if (value_end == std::string_view::npos) {
            return std::nullopt;
        }
        return std::string(text.substr(value_start + 1, value_end - value_start - 1));
    }

    std::size_t value_end = value_start;
    while (value_end < text.size() && text[value_end] != ',' && text[value_end] != '}') {
        ++value_end;
    }
    return std::string(text.substr(value_start, value_end - value_start));
}

std::optional<std::string> object_for_key(std::string_view text, std::string_view key) {
    const std::string token = "\"" + std::string(key) + "\":{";
    const std::size_t object_position = text.find(token);
    if (object_position == std::string_view::npos) {
        return std::nullopt;
    }
    const std::size_t open_position = object_position + token.size() - 1;
    const std::size_t close_position = matching_brace(text, open_position);
    if (close_position == std::string_view::npos) {
        return std::nullopt;
    }
    return std::string(text.substr(open_position, close_position - open_position + 1));
}

std::optional<std::string> value_for_field(std::string_view line, std::string_view field_path) {
    const std::size_t dot = field_path.find('.');
    if (dot == std::string_view::npos) {
        return scalar_for_key(line, field_path);
    }

    const std::string_view object_key = field_path.substr(0, dot);
    const std::string_view scalar_key = field_path.substr(dot + 1);
    const auto object = object_for_key(line, object_key);
    if (!object) {
        return std::nullopt;
    }
    return scalar_for_key(*object, scalar_key);
}

const std::vector<std::string>& telemetry_fields() {
    static const std::vector<std::string> fields {
        "time_s",
        "mode",
        "position.x",
        "position.y",
        "position.z",
        "velocity.x",
        "velocity.y",
        "velocity.z",
        "yaw_rad",
        "pitch_rad",
        "roll_rad",
        "battery_percent",
        "target_waypoint_index",
    };
    return fields;
}

TraceDiffResult make_difference(
    std::size_t step_index,
    std::string field_path,
    std::string left_value,
    std::string right_value) {
    std::ostringstream message;
    message << "trace divergence at step " << step_index << " field " << field_path
            << ": left=" << left_value << " right=" << right_value;
    return {false, step_index, std::move(field_path), std::move(left_value), std::move(right_value), message.str()};
}

} // namespace

TraceDiffResult diff_trace_text(std::string_view left, std::string_view right) {
    const std::vector<std::string> left_lines = split_lines(left);
    const std::vector<std::string> right_lines = split_lines(right);
    const std::size_t common_count = std::min(left_lines.size(), right_lines.size());

    for (std::size_t index = 0; index < common_count; ++index) {
        if (left_lines[index] == right_lines[index]) {
            continue;
        }

        for (const std::string& field : telemetry_fields()) {
            const auto left_value = value_for_field(left_lines[index], field);
            const auto right_value = value_for_field(right_lines[index], field);
            if (left_value && right_value && *left_value != *right_value) {
                return make_difference(index, field, *left_value, *right_value);
            }
        }

        return make_difference(index, "<raw_line>", left_lines[index], right_lines[index]);
    }

    if (left_lines.size() != right_lines.size()) {
        return make_difference(
            common_count,
            "<line_count>",
            std::to_string(left_lines.size()),
            std::to_string(right_lines.size()));
    }

    return {};
}

} // namespace agbot::flight_sim
