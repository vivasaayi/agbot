#pragma once

#include "agbot_flight_sim/Mission.hpp"

#include <filesystem>

namespace agbot::flight_sim {

class MissionLoader {
public:
    [[nodiscard]] static Mission load_from_file(const std::filesystem::path& path);
    [[nodiscard]] static Mission load_from_text(const std::string& text);
    static void save_to_file(const Mission& mission, const std::filesystem::path& path);
};

[[nodiscard]] std::filesystem::path default_sample_mission_path();

} // namespace agbot::flight_sim
