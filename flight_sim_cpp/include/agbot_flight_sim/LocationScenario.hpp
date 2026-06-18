#pragma once

#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_flight_sim/Mission.hpp"
#include "agbot_flight_sim/SceneSynthesis.hpp"

#include <cstdint>
#include <string>
#include <vector>

namespace agbot::flight_sim {

struct LocationScenarioRequest {
    GeoCoordinate center;
    double area_km2 = 25.0;
    std::uint64_t scene_seed = 0;
    bool mark_tiles_unavailable = false;
    bool mark_features_unavailable = false;
};

struct LocationScenarioManifest {
    std::string contract_version;
    GeoCoordinate center;
    double area_km2 = 0.0;
    std::uint64_t scene_seed = 0;
    Mission mission;
    TerrainProfile terrain_profile;
    std::vector<TerrainTileStatus> terrain_tiles;
    MapTextureComposite map_textures;
    SceneSynthesisManifest scene;
    std::vector<std::string> gaps;
    bool flyable = false;
    std::string scenario_hash;

    [[nodiscard]] bool has_gap(const std::string& gap) const;
    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] Mission mission_for_location(GeoCoordinate center, double area_km2);
[[nodiscard]] LocationScenarioManifest load_location_scenario(
    const LocationScenarioRequest& request);

} // namespace agbot::flight_sim
