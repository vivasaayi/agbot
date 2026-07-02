#include "agbot_worldgen/SceneMesh.hpp"

#include <mapbox/earcut.hpp>

#include <algorithm>
#include <array>
#include <cmath>
#include <cstring>
#include <limits>
#include <map>
#include <utility>

namespace agbot::worldgen {
namespace {

using agbot::flight_sim::GeoCoordinate;
using agbot::flight_sim::Vec3;

using Point2 = std::array<double, 2>;

// Signed shoelace area in the (x, z) ground plane.
double signed_area_xz(const std::vector<Vec3>& ring) {
    double doubled = 0.0;
    for (std::size_t index = 0; index < ring.size(); ++index) {
        const Vec3& current = ring[index];
        const Vec3& next = ring[(index + 1) % ring.size()];
        doubled += current.x * next.z - next.x * current.z;
    }
    return doubled * 0.5;
}

std::vector<Vec3> local_ring(
    const std::vector<GeoCoordinate>& ring,
    const GeoCoordinate& origin) {
    std::vector<Vec3> local;
    local.reserve(ring.size());
    for (const GeoCoordinate& point : ring) {
        Vec3 position = agbot::flight_sim::local_from_geo(point, origin);
        position.y = 0.0;
        local.push_back(position);
    }
    return local;
}

// Normalizes winding: exteriors get positive shoelace area in (x, z), holes
// negative, so wall outward normals follow one fixed formula.
void normalize_winding(std::vector<Vec3>& ring, bool is_hole) {
    const double area = signed_area_xz(ring);
    if ((is_hole && area > 0.0) || (!is_hole && area < 0.0)) {
        std::reverse(ring.begin(), ring.end());
    }
}

struct Bucket {
    std::vector<CityVertex> vertices;
    std::vector<std::uint32_t> indices;
};

CityVertex make_vertex(
    const Vec3& position,
    const Vec3& normal,
    std::uint32_t class_id,
    std::uint32_t object_ordinal) {
    CityVertex vertex;
    vertex.position[0] = static_cast<float>(position.x);
    vertex.position[1] = static_cast<float>(position.y);
    vertex.position[2] = static_cast<float>(position.z);
    vertex.normal[0] = static_cast<float>(normal.x);
    vertex.normal[1] = static_cast<float>(normal.y);
    vertex.normal[2] = static_cast<float>(normal.z);
    vertex.class_id = class_id;
    vertex.object_ordinal = object_ordinal;
    return vertex;
}

void emit_walls(
    Bucket& bucket,
    const std::vector<Vec3>& ring,
    double base_m,
    double top_m,
    std::uint32_t class_id,
    std::uint32_t object_ordinal) {
    for (std::size_t index = 0; index < ring.size(); ++index) {
        const Vec3& start = ring[index];
        const Vec3& end = ring[(index + 1) % ring.size()];
        const double dx = end.x - start.x;
        const double dz = end.z - start.z;
        const double length = std::sqrt(dx * dx + dz * dz);
        if (length <= 0.0) {
            continue;
        }
        // Outward normal for a shoelace-positive exterior (holes are wound
        // negative so the same formula points into the hole cavity).
        const Vec3 normal{dz / length, 0.0, -dx / length};

        const std::uint32_t base_index = static_cast<std::uint32_t>(bucket.vertices.size());
        bucket.vertices.push_back(
            make_vertex({start.x, base_m, start.z}, normal, class_id, object_ordinal));
        bucket.vertices.push_back(
            make_vertex({end.x, base_m, end.z}, normal, class_id, object_ordinal));
        bucket.vertices.push_back(
            make_vertex({end.x, top_m, end.z}, normal, class_id, object_ordinal));
        bucket.vertices.push_back(
            make_vertex({start.x, top_m, start.z}, normal, class_id, object_ordinal));
        // (b_i, t_j, b_j) and (b_i, t_i, t_j): CCW from outside.
        bucket.indices.insert(
            bucket.indices.end(),
            {base_index, base_index + 2, base_index + 1, base_index, base_index + 3,
             base_index + 2});
    }
}

void emit_cap(
    Bucket& bucket,
    const std::vector<std::vector<Vec3>>& rings,
    double top_m,
    std::uint32_t class_id,
    std::uint32_t object_ordinal) {
    std::vector<std::vector<Point2>> polygon;
    polygon.reserve(rings.size());
    std::vector<Vec3> flattened;
    for (const std::vector<Vec3>& ring : rings) {
        std::vector<Point2> ring_points;
        ring_points.reserve(ring.size());
        for (const Vec3& point : ring) {
            ring_points.push_back({point.x, point.z});
            flattened.push_back(point);
        }
        polygon.push_back(std::move(ring_points));
    }

    const std::vector<std::uint32_t> triangles = mapbox::earcut<std::uint32_t>(polygon);
    const std::uint32_t base_index = static_cast<std::uint32_t>(bucket.vertices.size());
    const Vec3 up{0.0, 1.0, 0.0};
    for (const Vec3& point : flattened) {
        bucket.vertices.push_back(
            make_vertex({point.x, top_m, point.z}, up, class_id, object_ordinal));
    }
    for (std::size_t triangle = 0; triangle + 2 < triangles.size(); triangle += 3) {
        std::uint32_t i0 = triangles[triangle];
        std::uint32_t i1 = triangles[triangle + 1];
        std::uint32_t i2 = triangles[triangle + 2];
        // Flip when the triangle's cross product points down (-y): a 2D
        // triangle with positive signed area in (x, z) has a -y 3D normal.
        const Vec3& a = flattened[i0];
        const Vec3& b = flattened[i1];
        const Vec3& c = flattened[i2];
        const double area2 =
            (b.x - a.x) * (c.z - a.z) - (c.x - a.x) * (b.z - a.z);
        if (area2 > 0.0) {
            std::swap(i1, i2);
        }
        bucket.indices.insert(
            bucket.indices.end(), {base_index + i0, base_index + i1, base_index + i2});
    }
}

} // namespace

std::uint32_t class_id_for(FeatureClass cls) {
    switch (cls) {
        case FeatureClass::Building:
            return 1;
        case FeatureClass::Road:
            return 2;
        case FeatureClass::Water:
            return 3;
        case FeatureClass::Vegetation:
            return 4;
        case FeatureClass::Bare:
            return 5;
        case FeatureClass::Unknown:
            return 0;
    }
    return 0;
}

CityMesh build_city_mesh(
    const std::vector<ExtractedFeature>& features,
    const GeoCoordinate& origin,
    const SceneMeshParams& params) {
    CityMesh mesh;
    const double tile_size = params.tile_size_m > 0.0 ? params.tile_size_m : 500.0;

    // Deterministic order: buildings sorted by source_id.
    std::vector<const ExtractedFeature*> buildings;
    for (const ExtractedFeature& feature : features) {
        if (feature.cls == FeatureClass::Building && feature.exterior.size() >= 3) {
            buildings.push_back(&feature);
        }
    }
    std::sort(
        buildings.begin(), buildings.end(),
        [](const ExtractedFeature* left, const ExtractedFeature* right) {
            return left->source_id < right->source_id;
        });

    std::map<std::pair<std::int32_t, std::int32_t>, Bucket> buckets;
    std::uint32_t object_ordinal = 0;
    for (const ExtractedFeature* feature : buildings) {
        std::vector<std::vector<Vec3>> rings;
        rings.push_back(local_ring(feature->exterior, origin));
        normalize_winding(rings.front(), /*is_hole=*/false);
        for (const std::vector<GeoCoordinate>& hole : feature->holes) {
            if (hole.size() < 3) {
                continue;
            }
            rings.push_back(local_ring(hole, origin));
            normalize_winding(rings.back(), /*is_hole=*/true);
        }

        const double base_m = feature->base_elev_m.value_or(0.0);
        const double height_m = std::max(0.1, feature->height_m.value_or(params.fallback_height_m));
        const double top_m = base_m + height_m;

        double centroid_x = 0.0;
        double centroid_z = 0.0;
        for (const Vec3& point : rings.front()) {
            centroid_x += point.x;
            centroid_z += point.z;
        }
        centroid_x /= static_cast<double>(rings.front().size());
        centroid_z /= static_cast<double>(rings.front().size());
        const std::pair<std::int32_t, std::int32_t> tile{
            static_cast<std::int32_t>(std::floor(centroid_x / tile_size)),
            static_cast<std::int32_t>(std::floor(centroid_z / tile_size)),
        };

        Bucket& bucket = buckets[tile];
        const std::uint32_t class_id = class_id_for(feature->cls);
        emit_cap(bucket, rings, top_m, class_id, object_ordinal);
        for (const std::vector<Vec3>& ring : rings) {
            emit_walls(bucket, ring, base_m, top_m, class_id, object_ordinal);
        }
        ++object_ordinal;
    }

    for (const auto& [tile, bucket] : buckets) {
        if (bucket.indices.empty()) {
            continue;
        }
        CityMeshBatch batch;
        batch.tile_x = tile.first;
        batch.tile_z = tile.second;
        batch.index_offset = static_cast<std::uint32_t>(mesh.indices.size());
        batch.index_count = static_cast<std::uint32_t>(bucket.indices.size());

        CityAabb aabb;
        for (int axis = 0; axis < 3; ++axis) {
            aabb.min[axis] = std::numeric_limits<float>::max();
            aabb.max[axis] = std::numeric_limits<float>::lowest();
        }
        for (const CityVertex& vertex : bucket.vertices) {
            for (int axis = 0; axis < 3; ++axis) {
                aabb.min[axis] = std::min(aabb.min[axis], vertex.position[axis]);
                aabb.max[axis] = std::max(aabb.max[axis], vertex.position[axis]);
            }
        }
        batch.aabb = aabb;

        const std::uint32_t vertex_base = static_cast<std::uint32_t>(mesh.vertices.size());
        mesh.vertices.insert(mesh.vertices.end(), bucket.vertices.begin(), bucket.vertices.end());
        for (const std::uint32_t index : bucket.indices) {
            mesh.indices.push_back(vertex_base + index);
        }
        mesh.batches.push_back(batch);
    }
    return mesh;
}

std::uint64_t city_mesh_vertex_hash(const CityMesh& mesh) {
    static_assert(sizeof(CityVertex) == 8 * sizeof(float), "CityVertex must be tightly packed");
    std::uint64_t hash = 1469598103934665603ULL;
    const unsigned char* bytes = reinterpret_cast<const unsigned char*>(mesh.vertices.data());
    const std::size_t byte_count = mesh.vertices.size() * sizeof(CityVertex);
    for (std::size_t index = 0; index < byte_count; ++index) {
        hash ^= static_cast<std::uint64_t>(bytes[index]);
        hash *= 1099511628211ULL;
    }
    return hash;
}

} // namespace agbot::worldgen
