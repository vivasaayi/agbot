#pragma once

#include "agbot_worldgen/RoadNetwork.hpp"
#include "agbot_worldgen/SceneMesh.hpp"

namespace agbot::worldgen {

struct RoadMeshParams {
    // Ribbon width = lanes * lane_width_m when the edge carries a lanes
    // count; otherwise the class default (class_width_m / default_width_m).
    double lane_width_m = 3.2;
    std::map<std::string, double> class_width_m = {
        {"motorway", 14.0},    {"trunk", 12.0},         {"primary", 10.0},
        {"secondary", 9.0},    {"tertiary", 8.0},       {"residential", 7.0},
        {"unclassified", 7.0}, {"living_street", 5.0},  {"service", 4.0},
    };
    double default_width_m = 7.0;
    // Ribbons sit slightly above the ground plane to avoid z-fighting.
    double ground_offset_m = 0.15;
};

// Builds flat road ribbons (two triangles per centerline segment) from the
// network's directed edges, skipping reverse companions so each two-way
// street is meshed once. Output reuses the CityMesh layout (single batch,
// class_id = class_id_for(FeatureClass::Road), dark-gray class in the
// renderer palette) so existing city-mesh consumers can draw roads
// unchanged. Deterministic: edges are meshed in ascending edge-id order.
[[nodiscard]] CityMesh build_road_mesh(const RoadNetwork& network, const RoadMeshParams& params);

} // namespace agbot::worldgen
