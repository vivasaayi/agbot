#include "agbot_flight_sim/TraceDiff.hpp"
#include "agbot_flight_sim/TwinContractV1.hpp"

#include <algorithm>
#include <cmath>
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

std::string escape_json(std::string_view value) {
    std::ostringstream output;
    for (const char c : value) {
        switch (c) {
            case '"': output << "\\\""; break;
            case '\\': output << "\\\\"; break;
            case '\n': output << "\\n"; break;
            case '\r': output << "\\r"; break;
            case '\t': output << "\\t"; break;
            default: output << c; break;
        }
    }
    return output.str();
}

std::optional<double> parse_number(std::string_view value) {
    try {
        const std::string owned(value);
        std::size_t parsed = 0;
        const double number = std::stod(owned, &parsed);
        if (parsed != owned.size() || !std::isfinite(number)) {
            return std::nullopt;
        }
        return number;
    } catch (const std::exception&) {
        return std::nullopt;
    }
}

bool values_equal(
    std::string_view left,
    std::string_view right,
    const TraceDiffOptions& options) {
    if (left == right) {
        return true;
    }
    const auto left_number = parse_number(left);
    const auto right_number = parse_number(right);
    if (!left_number || !right_number) {
        return false;
    }
    const double difference = std::abs(*left_number - *right_number);
    const double scale = std::max(std::abs(*left_number), std::abs(*right_number));
    return difference <= options.absolute_tolerance + options.relative_tolerance * scale;
}

void append_difference(
    TraceDiffResult& result,
    TraceDifference difference,
    std::size_t max_differences) {
    ++result.difference_count;
    if (result.differences.size() < max_differences) {
        result.differences.push_back(std::move(difference));
    } else {
        result.truncated = true;
    }
}

void finalize_result(TraceDiffResult& result) {
    if (result.difference_count == 0) {
        return;
    }
    result.identical = false;
    if (result.code == "identical") {
        result.code = "different";
    }
    const TraceDifference& first = result.differences.front();
    result.step_index = first.step_index;
    result.field_path = first.field_path;
    result.left_value = first.left_value;
    result.right_value = first.right_value;

    std::ostringstream message;
    if (!result.compatible) {
        message << "incompatible contract versions: left=" << first.left_value
                << " right=" << first.right_value;
    } else if (result.difference_count == 1) {
        message << "trace divergence at step " << first.step_index << " field " << first.field_path
                << ": left=" << first.left_value << " right=" << first.right_value;
    } else {
        message << result.difference_count << " trace differences; first at step "
                << first.step_index << " field " << first.field_path;
    }
    result.message = message.str();
}

} // namespace

std::string TraceDiffResult::to_json() const {
    std::ostringstream output;
    const char* status = !compatible ? "incompatible_contract" : (identical ? "identical" : "different");
    output << "{\"status\":\"" << status << "\""
           << ",\"code\":\"" << escape_json(code) << "\""
           << ",\"compatible\":" << (compatible ? "true" : "false")
           << ",\"difference_count\":" << difference_count
           << ",\"truncated\":" << (truncated ? "true" : "false")
           << ",\"differences\":[";
    for (std::size_t index = 0; index < differences.size(); ++index) {
        if (index > 0) {
            output << ',';
        }
        const auto& difference = differences[index];
        output << "{\"step_index\":" << difference.step_index
               << ",\"field_path\":\"" << escape_json(difference.field_path) << "\""
               << ",\"left_value\":\"" << escape_json(difference.left_value) << "\""
               << ",\"right_value\":\"" << escape_json(difference.right_value) << "\"}";
    }
    output << "]}";
    return output.str();
}

TraceDiffResult diff_trace_text(
    std::string_view left,
    std::string_view right,
    const TraceDiffOptions& options) {
    const std::vector<std::string> left_lines = split_lines(left);
    const std::vector<std::string> right_lines = split_lines(right);
    const std::size_t common_count = std::min(left_lines.size(), right_lines.size());
    const std::size_t max_differences = std::max<std::size_t>(1, options.max_differences);
    TraceDiffResult result;

    for (std::size_t index = 0; index < common_count; ++index) {
        if (left_lines[index] == right_lines[index]) {
            continue;
        }

        const auto left_contract = scalar_for_key(left_lines[index], "contract_version");
        const auto right_contract = scalar_for_key(right_lines[index], "contract_version");
        const bool contract_missing = left_contract.has_value() != right_contract.has_value();
        const bool contract_incompatible = left_contract && right_contract
            && *left_contract != *right_contract
            && !is_compatible_contract_version(*left_contract, *right_contract);
        if (contract_missing || contract_incompatible) {
            result.compatible = false;
            result.code = "incompatible_contract_version";
            append_difference(
                result,
                {
                    index,
                    "contract_version",
                    left_contract.value_or("<missing>"),
                    right_contract.value_or("<missing>"),
                },
                max_differences);
            finalize_result(result);
            return result;
        }

        bool found_recognized_change =
            left_contract && right_contract && *left_contract != *right_contract;
        for (const std::string& field : telemetry_fields()) {
            const auto left_value = value_for_field(left_lines[index], field);
            const auto right_value = value_for_field(right_lines[index], field);
            if (left_value && right_value && *left_value != *right_value) {
                found_recognized_change = true;
                if (!values_equal(*left_value, *right_value, options)) {
                    append_difference(result, {index, field, *left_value, *right_value}, max_differences);
                }
            } else if (left_value.has_value() != right_value.has_value()) {
                append_difference(
                    result,
                    {index, field, left_value.value_or("<missing>"), right_value.value_or("<missing>")},
                    max_differences);
                found_recognized_change = true;
            }
        }

        if (!found_recognized_change) {
            append_difference(
                result,
                {index, "<raw_line>", left_lines[index], right_lines[index]},
                max_differences);
        }
    }

    if (left_lines.size() != right_lines.size()) {
        append_difference(
            result,
            {common_count, "<line_count>", std::to_string(left_lines.size()), std::to_string(right_lines.size())},
            max_differences);
    }

    finalize_result(result);
    return result;
}

} // namespace agbot::flight_sim
