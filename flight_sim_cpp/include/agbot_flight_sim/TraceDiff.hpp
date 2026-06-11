#pragma once

#include <cstddef>
#include <string>
#include <string_view>

namespace agbot::flight_sim {

struct TraceDiffResult {
    bool identical = true;
    std::size_t step_index = 0;
    std::string field_path;
    std::string left_value;
    std::string right_value;
    std::string message = "traces identical";
};

[[nodiscard]] TraceDiffResult diff_trace_text(std::string_view left, std::string_view right);

} // namespace agbot::flight_sim
