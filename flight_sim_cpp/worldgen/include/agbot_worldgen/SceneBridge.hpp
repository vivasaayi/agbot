#pragma once

#include "agbot_flight_sim/SceneSynthesis.hpp"
#include "agbot_worldgen/Feature.hpp"

#include <cstdint>
#include <vector>

namespace agbot::worldgen {

// Converts extracted features into the flight-sim scene synthesis contract.
// Building-class features become BuildingFootprintFeature entries (exterior
// ring only; SceneSynthesis does not model holes) and Vegetation-class
// features become VegetationClassFeature entries. The terrain profile is an
// asserted EPSG:4326 profile over `aoi` so the manifest can reach Ready.
[[nodiscard]] agbot::flight_sim::SceneSynthesisInput to_scene_input(
    const std::vector<ExtractedFeature>& features,
    const agbot::flight_sim::GeoBounds& aoi,
    std::uint64_t seed);

// Convenience wrapper: bridge then synthesize the scene manifest.
[[nodiscard]] agbot::flight_sim::SceneSynthesisManifest scene_manifest_for(
    const std::vector<ExtractedFeature>& features,
    const agbot::flight_sim::GeoBounds& aoi,
    std::uint64_t seed);

} // namespace agbot::worldgen
