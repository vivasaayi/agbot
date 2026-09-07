#pragma once

#include <cstddef>
#include <string>
#include <string_view>
#include <vector>

namespace agbot::flight_sim {

struct TraceDiffOptions {
    double absolute_tolerance = 0.0;
    double relative_tolerance = 0.0;
    std::size_t max_differences = 1;
};

struct TraceDifference {
    std::size_t step_index = 0;
    std::string field_path;
    std::string left_value;
    std::string right_value;
};

struct TraceDiffResult {
    bool identical = true;
    bool compatible = true;
    std::string code = "identical";
    std::vector<TraceDifference> differences;
    std::size_t difference_count = 0;
    bool truncated = false;

    // First-difference compatibility fields used by golden regression callers.
    std::size_t step_index = 0;
    std::string field_path;
    std::string left_value;
    std::string right_value;
    std::string message = "traces identical";

    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] TraceDiffResult diff_trace_text(
    std::string_view left,
    std::string_view right,
    const TraceDiffOptions& options = {});

} // namespace agbot::flight_sim
