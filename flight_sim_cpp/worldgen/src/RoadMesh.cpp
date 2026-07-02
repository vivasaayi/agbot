#include "agbot_worldgen/RoadMesh.hpp"

#include <algorithm>
#include <cmath>
#include <limits>

namespace agbot::worldgen {
namespace {

double width_for(const RoadEdge& edge, const RoadMeshParams& params) {
    if (edge.lanes > 0) {
        return static_cast<double>(edge.lanes) * params.lane_width_m;
    }
    const auto it = params.class_width_m.find(edge.highway);
    return it == params.class_width_m.end() ? params.default_width_m : it->second;
}

} // namespace

CityMesh build_road_mesh(const RoadNetwork& network, const RoadMeshParams& params) {
    CityMesh mesh;
    const std::uint32_t class_id = class_id_for(FeatureClass::Road);
    const float y = static_cast<float>(params.ground_offset_m);

    std::uint32_t object_ordinal = 0;
    for (const RoadEdge& edge : network.edges()) {
        // Mesh each two-way street once: skip the reverse companion.
        const RoadEdge* reverse = network.reverse_edge(edge.id);
        if (reverse != nullptr && reverse->id < edge.id) {
            continue;
        }
        const double half_width = 0.5 * width_for(edge, params);
        for (std::size_t i = 1; i < edge.polyline.size(); ++i) {
            const Vec3& a = edge.polyline[i - 1];
            const Vec3& b = edge.polyline[i];
            const double dx = b.x - a.x;
            const double dz = b.z - a.z;
            const double length = std::sqrt(dx * dx + dz * dz);
            if (length < 1e-9) {
                continue;
            }
            // Left-hand perpendicular in the XZ plane.
            const double nx = -dz / length * half_width;
            const double nz = dx / length * half_width;

            const std::uint32_t base = static_cast<std::uint32_t>(mesh.vertices.size());
            const double corners[4][2] = {
                {a.x + nx, a.z + nz},
                {a.x - nx, a.z - nz},
                {b.x + nx, b.z + nz},
                {b.x - nx, b.z - nz},
            };
            for (const auto& corner : corners) {
                CityVertex vertex;
                vertex.position[0] = static_cast<float>(corner[0]);
                vertex.position[1] = y;
                vertex.position[2] = static_cast<float>(corner[1]);
                vertex.normal[1] = 1.0f;
                vertex.class_id = class_id;
                vertex.object_ordinal = object_ordinal;
                mesh.vertices.push_back(vertex);
            }
            // Two up-facing triangles per segment.
            mesh.indices.push_back(base + 0);
            mesh.indices.push_back(base + 2);
            mesh.indices.push_back(base + 1);
            mesh.indices.push_back(base + 1);
            mesh.indices.push_back(base + 2);
            mesh.indices.push_back(base + 3);
        }
        ++object_ordinal;
    }

    if (!mesh.indices.empty()) {
        CityMeshBatch batch;
        batch.tile_x = 0;
        batch.tile_z = 0;
        batch.index_offset = 0;
        batch.index_count = static_cast<std::uint32_t>(mesh.indices.size());
        for (int axis = 0; axis < 3; ++axis) {
            batch.aabb.min[axis] = std::numeric_limits<float>::max();
            batch.aabb.max[axis] = std::numeric_limits<float>::lowest();
        }
        for (const CityVertex& vertex : mesh.vertices) {
            for (int axis = 0; axis < 3; ++axis) {
                batch.aabb.min[axis] = std::min(batch.aabb.min[axis], vertex.position[axis]);
                batch.aabb.max[axis] = std::max(batch.aabb.max[axis], vertex.position[axis]);
            }
        }
        mesh.batches.push_back(batch);
    }
    return mesh;
}

} // namespace agbot::worldgen
