#include "agbot_worldgen/SceneBridge.hpp"

#include <algorithm>

namespace agbot::worldgen {
namespace {

agbot::flight_sim::TerrainProfile profile_for(const agbot::flight_sim::GeoBounds& aoi) {
    agbot::flight_sim::TerrainProfile profile;
    profile.crs = "EPSG:4326";
    profile.bounds = aoi;
    profile.resolution = 1;
    profile.resolution_x_m = aoi.width_m();
    profile.resolution_y_m = aoi.height_m();
    profile.asserted = true;
    return profile;
}

} // namespace

agbot::flight_sim::SceneSynthesisInput to_scene_input(
    const std::vector<ExtractedFeature>& features,
    const agbot::flight_sim::GeoBounds& aoi,
    std::uint64_t seed) {
    agbot::flight_sim::SceneSynthesisInput input;
    input.seed = seed;
    input.profile = profile_for(aoi);
    for (const ExtractedFeature& feature : features) {
        if (feature.exterior.size() < 3) {
            continue;
        }
        if (feature.cls == FeatureClass::Building) {
            agbot::flight_sim::BuildingFootprintFeature building;
            building.source_id = feature.source_id;
            building.class_name = feature.class_name;
            building.footprint = feature.exterior;
            building.height_m = feature.height_m.value_or(0.0);
            input.buildings.push_back(std::move(building));
        } else if (feature.cls == FeatureClass::Vegetation) {
            agbot::flight_sim::VegetationClassFeature vegetation;
            vegetation.source_id = feature.source_id;
            vegetation.class_name = feature.class_name;
            const auto crop_it = feature.attributes.find("crop");
            vegetation.crop = crop_it != feature.attributes.end() ? crop_it->second : "";
            vegetation.footprint = feature.exterior;
            vegetation.canopy_height_m = feature.height_m.value_or(0.0);
            vegetation.coverage_fraction = 1.0;
            input.vegetation.push_back(std::move(vegetation));
        }
    }
    return input;
}

agbot::flight_sim::SceneSynthesisManifest scene_manifest_for(
    const std::vector<ExtractedFeature>& features,
    const agbot::flight_sim::GeoBounds& aoi,
    std::uint64_t seed) {
    return agbot::flight_sim::synthesize_scene_manifest(to_scene_input(features, aoi, seed));
}

} // namespace agbot::worldgen
