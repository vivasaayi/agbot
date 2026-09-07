#include "agbot_render/WorldPackage.hpp"

#include "agbot_render/SceneFile.hpp"

#include <nlohmann/json.hpp>

#include <algorithm>
#include <cmath>
#include <fstream>
#include <limits>

namespace agbot::render {
namespace {

bool is_within(const std::filesystem::path& root, const std::filesystem::path& candidate) {
    const auto mismatch = std::mismatch(
        root.begin(), root.end(), candidate.begin(), candidate.end());
    return mismatch.first == root.end();
}

WorldPackageResult failure(std::string message) {
    WorldPackageResult result;
    result.error = std::move(message);
    return result;
}

WorldTerrainResult terrain_failure(std::string message) {
    WorldTerrainResult result;
    result.error = std::move(message);
    return result;
}

template <typename Vertex>
agbot::flight_sim::TerrainMesh convert_terrain_mesh(
    const std::vector<Vertex>& vertices,
    const std::vector<std::uint32_t>& indices,
    int resolution,
    agbot::flight_sim::Vec3 frame_offset) {
    agbot::flight_sim::TerrainMesh mesh;
    mesh.vertices.reserve(vertices.size());
    mesh.indices = indices;
    mesh.min_elevation_m = std::numeric_limits<float>::max();
    mesh.max_elevation_m = std::numeric_limits<float>::lowest();

    for (std::size_t index = 0; index < vertices.size(); ++index) {
        const Vertex& vertex = vertices[index];
        const int grid_x = static_cast<int>(index % static_cast<std::size_t>(resolution));
        const int grid_z = static_cast<int>(index / static_cast<std::size_t>(resolution));
        mesh.vertices.push_back({
            {static_cast<double>(vertex.px) + frame_offset.x,
             static_cast<double>(vertex.py),
             static_cast<double>(vertex.pz) + frame_offset.z},
            {vertex.nx, vertex.ny, vertex.nz},
            static_cast<double>(grid_x) / static_cast<double>(resolution - 1),
            1.0 - static_cast<double>(grid_z) / static_cast<double>(resolution - 1),
        });
        mesh.min_elevation_m = std::min(mesh.min_elevation_m, vertex.py);
        mesh.max_elevation_m = std::max(mesh.max_elevation_m, vertex.py);
    }
    mesh.has_elevation = mesh.max_elevation_m > mesh.min_elevation_m;
    return mesh;
}

template <typename Mesh>
const Mesh* find_terrain_grid(
    const std::vector<Mesh>& meshes,
    std::size_t expected_vertices,
    std::size_t expected_indices) {
    const auto found = std::find_if(
        meshes.begin(),
        meshes.end(),
        [expected_vertices, expected_indices](const Mesh& mesh) {
            if (mesh.vertices.size() != expected_vertices ||
                mesh.indices.size() != expected_indices) {
                return false;
            }
            return std::all_of(
                mesh.indices.begin(),
                mesh.indices.end(),
                [expected_vertices](std::uint32_t index) {
                    return index < expected_vertices;
                });
        });
    return found == meshes.end() ? nullptr : &*found;
}

} // namespace

WorldPackageResult read_world_package(const std::filesystem::path& manifest_path) {
    std::ifstream input(manifest_path, std::ios::binary);
    if (!input) {
        return failure("world_manifest_not_found:" + manifest_path.string());
    }

    nlohmann::json manifest;
    try {
        input >> manifest;
    } catch (const std::exception& error) {
        return failure(std::string("world_manifest_invalid_json:") + error.what());
    }
    if (!manifest.is_object() || !manifest.contains("world_hash") ||
        !manifest.contains("tiles") || !manifest["tiles"].is_array() ||
        manifest["tiles"].empty()) {
        return failure("world_manifest_missing_required_fields");
    }
    const auto& tile = manifest["tiles"].front();
    if (!tile.is_object() || !tile.contains("scene_path") ||
        !tile["scene_path"].is_string()) {
        return failure("world_manifest_missing_scene_path");
    }
    const std::filesystem::path relative_scene =
        tile["scene_path"].get<std::string>();
    if (relative_scene.empty() || relative_scene.is_absolute()) {
        return failure("world_manifest_scene_path_not_relative");
    }

    std::error_code path_error;
    const std::filesystem::path absolute_manifest =
        std::filesystem::absolute(manifest_path, path_error);
    if (path_error) {
        return failure("world_manifest_path_invalid:" + path_error.message());
    }
    const std::filesystem::path package_root =
        std::filesystem::weakly_canonical(absolute_manifest.parent_path(), path_error);
    if (path_error) {
        return failure("world_manifest_package_path_invalid:" + path_error.message());
    }
    const std::filesystem::path scene_path =
        std::filesystem::weakly_canonical(package_root / relative_scene, path_error);
    if (path_error) {
        return failure("world_manifest_scene_path_invalid:" + path_error.message());
    }
    if (!is_within(package_root, scene_path)) {
        return failure("world_manifest_scene_path_escapes_package");
    }
    if (scene_path.extension() != ".agbscn" || !std::filesystem::is_regular_file(scene_path)) {
        return failure("world_manifest_scene_payload_missing");
    }

    WorldPackageResult result;
    result.package.manifest_path =
        std::filesystem::weakly_canonical(absolute_manifest, path_error);
    if (path_error) {
        return failure("world_manifest_path_invalid:" + path_error.message());
    }
    result.package.scene_path = scene_path;
    try {
        result.package.world_hash = manifest["world_hash"].get<std::uint64_t>();
        if (manifest.contains("crs_policy") && manifest["crs_policy"].is_object()) {
            result.package.horizontal_crs =
                manifest["crs_policy"].value("horizontal", "");
            result.package.vertical_datum =
                manifest["crs_policy"].value("vertical_datum", "");
            result.package.runtime_frame =
                manifest["crs_policy"].value("runtime_frame", "");
        }
        result.package.elevation_state = tile.value("elevation_state", "");
        if (manifest.contains("aoi") && manifest["aoi"].is_object()) {
            const auto& aoi = manifest["aoi"];
            result.package.aoi = {
                aoi.at("min_lat").get<double>(),
                aoi.at("min_lon").get<double>(),
                aoi.at("max_lat").get<double>(),
                aoi.at("max_lon").get<double>(),
            };
            result.package.has_aoi =
                result.package.aoi.max_latitude > result.package.aoi.min_latitude &&
                result.package.aoi.max_longitude > result.package.aoi.min_longitude;
        }
        if (manifest.contains("quality") && manifest["quality"].is_object()) {
            const auto& quality = manifest["quality"];
            result.package.terrain_cell_count =
                quality.value("terrain_cell_count", std::size_t {0});
            result.package.terrain_nodata_cells =
                quality.value("terrain_nodata_cells", std::size_t {0});
        }
        if (tile.contains("provenance") && tile["provenance"].is_array()) {
            for (const auto& provenance : tile["provenance"]) {
                if (provenance.is_object() &&
                    provenance.value("kind", "") == "terrain") {
                    result.package.terrain_source_id =
                        provenance.value("source_id", "");
                    break;
                }
            }
        }
        if (result.package.terrain_source_id.empty() &&
            manifest.contains("sources") && manifest["sources"].is_array() &&
            !manifest["sources"].empty() && manifest["sources"].front().is_object()) {
            result.package.terrain_source_id =
                manifest["sources"].front().value("source_id", "");
        }
    } catch (const std::exception& error) {
        return failure(std::string("world_manifest_invalid_metadata:") + error.what());
    }
    return result;
}

WorldTerrainResult load_world_terrain(
    const std::filesystem::path& manifest_path,
    const agbot::flight_sim::GeoCoordinate& runtime_origin) {
    const WorldPackageResult package_result = read_world_package(manifest_path);
    if (!package_result.ok()) {
        return terrain_failure(*package_result.error);
    }
    const WorldPackage& package = package_result.package;
    if (!package.has_aoi) {
        return terrain_failure("world_manifest_missing_valid_aoi");
    }
    if (package.horizontal_crs != "EPSG:4326") {
        return terrain_failure("world_manifest_unsupported_horizontal_crs");
    }
    if (package.vertical_datum.empty()) {
        return terrain_failure("world_manifest_missing_vertical_datum");
    }
    if (package.runtime_frame != "local_enu_m") {
        return terrain_failure("world_manifest_unsupported_runtime_frame");
    }
    if (package.elevation_state.empty() || package.elevation_state == "missing") {
        return terrain_failure("world_manifest_terrain_elevation_missing");
    }
    if (package.terrain_cell_count < 4) {
        return terrain_failure("world_manifest_missing_terrain_grid");
    }
    if (package.terrain_nodata_cells > 0) {
        return terrain_failure("world_manifest_terrain_contains_nodata");
    }

    const double root = std::sqrt(static_cast<double>(package.terrain_cell_count));
    const int resolution = static_cast<int>(std::llround(root));
    if (resolution < 2 ||
        static_cast<std::size_t>(resolution * resolution) != package.terrain_cell_count) {
        return terrain_failure("world_manifest_terrain_grid_not_square");
    }
    const std::size_t expected_indices =
        static_cast<std::size_t>((resolution - 1) * (resolution - 1) * 6);

    const SceneFileResult scene_result = read_scene_file(package.scene_path);
    if (!scene_result.ok()) {
        return terrain_failure(
            "world_scene_invalid:" + scene_result.error->message);
    }

    const agbot::flight_sim::Vec3 frame_offset =
        agbot::flight_sim::local_from_geo(package.aoi.center(), runtime_origin);
    agbot::flight_sim::TerrainMesh terrain_mesh;
    if (const RenderMesh* mesh = find_terrain_grid(
            scene_result.scene.static_meshes,
            package.terrain_cell_count,
            expected_indices)) {
        terrain_mesh = convert_terrain_mesh(
            mesh->vertices, mesh->indices, resolution, frame_offset);
    } else if (const TexturedMesh* mesh = find_terrain_grid(
                   scene_result.scene.textured_meshes,
                   package.terrain_cell_count,
                   expected_indices)) {
        terrain_mesh = convert_terrain_mesh(
            mesh->vertices, mesh->indices, resolution, frame_offset);
    } else {
        return terrain_failure("world_scene_terrain_grid_missing");
    }

    WorldTerrainResult result;
    result.terrain.mesh = std::move(terrain_mesh);
    result.terrain.bounds = package.aoi;
    result.terrain.world_hash = package.world_hash;
    result.terrain.source_id = package.terrain_source_id;
    result.terrain.vertical_datum = package.vertical_datum;
    result.terrain.elevation_state = package.elevation_state;
    result.terrain.resolution = resolution;
    result.terrain.nodata_cells = package.terrain_nodata_cells;
    return result;
}

} // namespace agbot::render
