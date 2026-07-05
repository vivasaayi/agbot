#pragma once

#include "agbot_flight_sim/Mission.hpp"
#include "agbot_worldgen/Feature.hpp"

#include <cstdint>
#include <vector>

namespace agbot::worldgen {

// Vertex of the extruded city mesh. Positions are local meters around the
// mesh origin, matching the sim frame: x = east, y = up, z = north.
struct CityVertex {
    float position[3] = {0.0f, 0.0f, 0.0f};
    float normal[3] = {0.0f, 0.0f, 0.0f};
    std::uint32_t class_id = 0;
    std::uint32_t object_ordinal = 0;
};

// Axis-aligned bounding box in local meters.
struct CityAabb {
    float min[3] = {0.0f, 0.0f, 0.0f};
    float max[3] = {0.0f, 0.0f, 0.0f};
};

// One draw batch: a contiguous index range covering all geometry whose
// footprint centroid falls inside a spatial grid tile.
struct CityMeshBatch {
    std::int32_t tile_x = 0;
    std::int32_t tile_z = 0;
    std::uint32_t index_offset = 0;
    std::uint32_t index_count = 0;
    CityAabb aabb;
};

struct CityMesh {
    std::vector<CityVertex> vertices;
    std::vector<std::uint32_t> indices;
    std::vector<CityMeshBatch> batches;
};

struct SceneMeshParams {
    double tile_size_m = 500.0;
    // Height used when a feature carries no resolved height.
    double fallback_height_m = 3.0;
};

[[nodiscard]] std::uint32_t class_id_for(FeatureClass cls);

// Extrudes every Building-class feature into a cap (earcut triangulated,
// holes supported) at y = base + height plus wall quads (two triangles per
// edge, exterior and holes). Features are processed in deterministic order
// (sorted by source_id) and batched into a spatial grid of `tile_size_m`.
[[nodiscard]] CityMesh build_city_mesh(
    const std::vector<ExtractedFeature>& features,
    const agbot::flight_sim::GeoCoordinate& origin,
    const SceneMeshParams& params);

// FNV1a-64 over the raw vertex bytes; used for determinism checks.
[[nodiscard]] std::uint64_t city_mesh_vertex_hash(const CityMesh& mesh);

} // namespace agbot::worldgen
