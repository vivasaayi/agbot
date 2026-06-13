#pragma once

#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_flight_sim/Mission.hpp"
#include "agbot_flight_sim/Vec3.hpp"

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

namespace agbot::flight_sim {

enum class SceneSynthesisStatus {
    Ready,
    Unpopulated,
    Invalid,
};

struct BuildingFootprintFeature {
    std::string source_id;
    std::string class_name;
    std::vector<GeoCoordinate> footprint;
    double height_m = 0.0;
};

struct VegetationClassFeature {
    std::string source_id;
    std::string class_name;
    std::string crop;
    std::vector<GeoCoordinate> footprint;
    double canopy_height_m = 0.0;
    double coverage_fraction = 0.0;
};

struct SceneSynthesisInput {
    std::uint64_t seed = 0;
    TerrainProfile profile;
    std::vector<BuildingFootprintFeature> buildings;
    std::vector<VegetationClassFeature> vegetation;
};

struct SceneObject {
    std::string object_id;
    std::string source_id;
    std::string source_kind;
    std::string class_name;
    std::string crop;
    std::vector<GeoCoordinate> footprint_geo;
    std::vector<Vec3> footprint_local_m;
    double height_m = 0.0;
    std::uint64_t placement_seed = 0;
};

struct UnpopulatedArea {
    GeoBounds bounds;
    std::string reason;
};

struct SceneSynthesisManifest {
    std::string contract_version;
    SceneSynthesisStatus status = SceneSynthesisStatus::Invalid;
    std::uint64_t seed = 0;
    TerrainProfile profile;
    std::vector<SceneObject> objects;
    std::vector<UnpopulatedArea> unpopulated_areas;
    std::size_t object_count = 0;
    std::size_t unpopulated_area_count = 0;
    std::string scene_hash;

    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] const char* to_string(SceneSynthesisStatus status);

[[nodiscard]] SceneSynthesisManifest synthesize_scene_manifest(const SceneSynthesisInput& input);

} // namespace agbot::flight_sim
