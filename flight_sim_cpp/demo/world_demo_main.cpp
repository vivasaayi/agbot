// agbot_world_demo: builds the realistic-world demo scene for Lower Manhattan.
//
// Pipeline: terrain_engine (cached Terrarium DEM + synthetic detail fusion)
// + worldgen (NYC Open Data footprints -> extruded city mesh) -> render
// (.agbscn scene consumed by agbot_world_viewer).
//
// Usage:
//   agbot_world_demo            build scene, write out/world/manhattan.agbscn
//   agbot_world_demo --check    build scene, assert invariants, exit 0/1

#include "agbot_config/Params.hpp"
#include "agbot_config/Toml.hpp"
#include "agbot_flight_sim/MissionLoader.hpp"
#include "agbot_render/RenderScene.hpp"
#include "agbot_render/SceneFile.hpp"
#include "agbot_terrain/TerrainPipeline.hpp"
#include "agbot_worldgen/SceneMesh.hpp"
#include "agbot_worldgen/extractors/VectorImport.hpp"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <filesystem>
#include <iostream>
#include <string>

namespace {

namespace cfg = agbot::config;
namespace fs = agbot::flight_sim;

const char* kTerrainConfig = R"toml(
[pipeline]
target_gsd_m = 30.0
resolution = 128
aoi = { min_lat = 40.700, min_lon = -74.020, max_lat = 40.740, max_lon = -73.980 }

[[layer]]
algorithm = "dem_fusion"
weight = 1.0
  [layer.params]
  source = "terrarium"
  zoom = 13
  resample = "bilinear"
  void_fill = "idw"
  clamp_min_m = -2.0

[[layer]]
algorithm = "synthetic_detail"
weight = 1.0
  [layer.params]
  amplitude_m = 0.6
  octaves = 4
  frequency = 8.0
  seed = 1337
  confidence = 0.3

[fusion]
method = "detail_injection"
lambda = 0.3
cutoff_cells = 2

[validation]
enabled = true
reference_layer = 0
output_json = "out/world/terrain_validation.json"
)toml";

struct DemoStats {
    int terrain_width = 0;
    int terrain_height = 0;
    float terrain_min_m = 0.0f;
    float terrain_max_m = 0.0f;
    std::size_t building_count = 0;
    std::size_t city_vertices = 0;
    std::size_t city_triangles = 0;
    std::size_t city_batches = 0;
    double max_building_height_m = 0.0;
};

void terrain_color(float elevation_m, float& r, float& g, float& b) {
    // Low waterfront greens through tan to brown with elevation.
    const float t = std::clamp(elevation_m / 60.0f, 0.0f, 1.0f);
    r = 0.30f + 0.35f * t;
    g = 0.46f + 0.16f * t;
    b = 0.26f + 0.10f * t;
    if (elevation_m < 0.5f) {
        r = 0.22f;
        g = 0.34f;
        b = 0.42f;
    }
}

agbot::render::RenderMesh terrain_render_mesh(const agbot::terrain::HeightField& field,
                                              const fs::GeoCoordinate& origin) {
    const agbot::terrain::Raster& elevation = field.elevation;
    agbot::render::RenderMesh mesh;
    const int width = elevation.width;
    const int height = elevation.height;
    mesh.vertices.reserve(static_cast<std::size_t>(width) * static_cast<std::size_t>(height));

    const double lat_span = elevation.bounds.max_latitude - elevation.bounds.min_latitude;
    const double lon_span = elevation.bounds.max_longitude - elevation.bounds.min_longitude;

    auto elevation_at = [&](int row, int col) -> float {
        const float value = elevation.at(std::clamp(row, 0, height - 1),
                                         std::clamp(col, 0, width - 1));
        return agbot::terrain::Raster::is_nodata(value) ? 0.0f : value;
    };

    for (int row = 0; row < height; ++row) {
        // Row 0 is the northernmost row.
        const double latitude = elevation.bounds.max_latitude -
            lat_span * static_cast<double>(row) / static_cast<double>(height - 1);
        for (int col = 0; col < width; ++col) {
            const double longitude = elevation.bounds.min_longitude +
                lon_span * static_cast<double>(col) / static_cast<double>(width - 1);
            const float elev = elevation_at(row, col);
            const fs::Vec3 local =
                fs::local_from_geo({latitude, longitude, static_cast<double>(elev)}, origin);

            // Finite-difference normal (grid spacing in meters).
            const fs::Vec3 east_step = fs::local_from_geo(
                {latitude, longitude + lon_span / (width - 1), 0.0}, origin);
            const fs::Vec3 north_step = fs::local_from_geo(
                {latitude + lat_span / (height - 1), longitude, 0.0}, origin);
            const float dx = static_cast<float>(east_step.x - local.x);
            const float dz = static_cast<float>(north_step.z - local.z);
            const float dedx = (elevation_at(row, col + 1) - elevation_at(row, col - 1)) /
                (2.0f * std::max(dx, 1.0f));
            const float dedz = (elevation_at(row - 1, col) - elevation_at(row + 1, col)) /
                (2.0f * std::max(dz, 1.0f));
            float nx = -dedx;
            float ny = 1.0f;
            float nz = -dedz;
            const float norm = std::sqrt(nx * nx + ny * ny + nz * nz);
            nx /= norm;
            ny /= norm;
            nz /= norm;

            agbot::render::RenderVertex vertex;
            vertex.px = static_cast<float>(local.x);
            vertex.py = elev;
            vertex.pz = static_cast<float>(local.z);
            vertex.nx = nx;
            vertex.ny = ny;
            vertex.nz = nz;
            terrain_color(elev, vertex.r, vertex.g, vertex.b);
            vertex.a = 1.0f;
            mesh.vertices.push_back(vertex);
        }
    }

    for (int row = 0; row + 1 < height; ++row) {
        for (int col = 0; col + 1 < width; ++col) {
            const std::uint32_t i00 = static_cast<std::uint32_t>(row * width + col);
            const std::uint32_t i01 = i00 + 1;
            const std::uint32_t i10 = i00 + static_cast<std::uint32_t>(width);
            const std::uint32_t i11 = i10 + 1;
            mesh.indices.insert(mesh.indices.end(), {i00, i11, i10, i00, i01, i11});
        }
    }
    return mesh;
}

agbot::render::RenderMesh city_render_mesh(const agbot::worldgen::CityMesh& city) {
    agbot::render::RenderMesh mesh;
    mesh.vertices.reserve(city.vertices.size());
    for (const agbot::worldgen::CityVertex& vertex : city.vertices) {
        agbot::render::RenderVertex out;
        out.px = vertex.position[0];
        out.py = vertex.position[1];
        out.pz = vertex.position[2];
        out.nx = vertex.normal[0];
        out.ny = vertex.normal[1];
        out.nz = vertex.normal[2];
        // Per-building brightness variation keyed off the object ordinal so
        // facades of adjacent towers read as distinct volumes.
        const float variation =
            static_cast<float>((vertex.object_ordinal * 2654435761u) % 1000u) / 1000.0f;
        const float base = 0.62f + 0.24f * variation;
        out.r = base;
        out.g = base;
        out.b = std::min(1.0f, base + 0.05f);
        out.a = 1.0f;
        mesh.vertices.push_back(out);
    }
    mesh.indices = city.indices;
    return mesh;
}

} // namespace

int main(int argc, char** argv) {
    const bool check_mode = argc > 1 && std::string(argv[1]) == "--check";
    const std::filesystem::path source_dir = AGBOT_FLIGHT_SIM_SOURCE_DIR;

    // --- Terrain ---
    const cfg::TomlParseResult terrain_config = cfg::parse_toml(kTerrainConfig);
    if (!terrain_config.ok) {
        std::cerr << "terrain config parse failed: " << terrain_config.error << "\n";
        return 1;
    }
    const agbot::terrain::PipelineResult terrain =
        agbot::terrain::run_terrain_pipeline(terrain_config.root);
    if (!terrain.ok) {
        std::cerr << "terrain pipeline failed: " << terrain.error << "\n";
        return 1;
    }

    const fs::GeoBounds aoi = terrain.fused.elevation.bounds;
    const fs::GeoCoordinate origin = aoi.center();

    // --- City ---
    const std::filesystem::path buildings_path =
        source_dir / "data/worldgen/manhattan_buildings.geojson";
    if (!std::filesystem::exists(buildings_path)) {
        std::cerr << "SKIP: building data missing; run "
                     "worldgen/tools/fetch_nyc_buildings.sh first ("
                  << buildings_path.string() << ")\n";
        return 77; // ctest SKIP_RETURN_CODE
    }
    cfg::ParamTable import_params;
    import_params["path"] =
        cfg::ParamValue(buildings_path.string());
    import_params["height_attr"] = cfg::ParamValue(std::string("height_roof"));
    import_params["height_units"] = cfg::ParamValue(std::string("feet"));
    import_params["base_elev_attr"] = cfg::ParamValue(std::string("ground_elevation"));
    import_params["base_units"] = cfg::ParamValue(std::string("feet"));
    import_params["id_attr"] = cfg::ParamValue(std::string("bin"));
    import_params["min_area_m2"] = cfg::ParamValue(10.0);

    const agbot::worldgen::VectorImportExtractor extractor;
    const agbot::worldgen::ExtractionResult extraction =
        extractor.extract({aoi, import_params});
    if (!extraction.ok) {
        std::cerr << "city extraction failed: " << extraction.error_code << " — "
                  << extraction.error_detail << "\n";
        return 1;
    }

    agbot::worldgen::SceneMeshParams mesh_params;
    const agbot::worldgen::CityMesh city =
        agbot::worldgen::build_city_mesh(extraction.features, origin, mesh_params);

    // --- Scene assembly ---
    agbot::render::RenderScene scene;
    scene.static_meshes.push_back(terrain_render_mesh(terrain.fused, origin));
    scene.static_meshes.push_back(city_render_mesh(city));
    scene.markers.push_back({0.0f, 320.0f, 0.0f, 1.0f, 0.25f, 0.2f, 12.0f});
    scene.sun_dir[0] = 0.4f;
    scene.sun_dir[1] = -0.75f;
    scene.sun_dir[2] = 0.53f;

    const std::filesystem::path out_path = source_dir / "out/world/manhattan.agbscn";
    std::filesystem::create_directories(out_path.parent_path());
    if (const auto error = agbot::render::write_scene_file(out_path, scene)) {
        std::cerr << "scene write failed: " << error->message << "\n";
        return 1;
    }

    // --- Stats + evidence ---
    DemoStats stats;
    stats.terrain_width = terrain.fused.elevation.width;
    stats.terrain_height = terrain.fused.elevation.height;
    stats.terrain_min_m = std::numeric_limits<float>::max();
    stats.terrain_max_m = std::numeric_limits<float>::lowest();
    for (const float value : terrain.fused.elevation.values) {
        if (!agbot::terrain::Raster::is_nodata(value)) {
            stats.terrain_min_m = std::min(stats.terrain_min_m, value);
            stats.terrain_max_m = std::max(stats.terrain_max_m, value);
        }
    }
    stats.building_count = extraction.features.size();
    stats.city_vertices = city.vertices.size();
    stats.city_triangles = city.indices.size() / 3;
    stats.city_batches = city.batches.size();
    for (const agbot::worldgen::ExtractedFeature& feature : extraction.features) {
        if (feature.height_m.has_value()) {
            stats.max_building_height_m = std::max(stats.max_building_height_m, *feature.height_m);
        }
    }

    std::cout << "manhattan world scene: " << out_path.string() << "\n"
              << "  terrain grid " << stats.terrain_width << "x" << stats.terrain_height
              << ", elevation " << stats.terrain_min_m << ".." << stats.terrain_max_m << " m"
              << " (source: " << terrain.fused.source_algorithm << ")\n"
              << "  terrain validation RMSE vs DEM: " << terrain.validation.metrics.rmse << " m\n"
              << "  buildings " << stats.building_count << ", max height "
              << stats.max_building_height_m << " m\n"
              << "  city mesh " << stats.city_vertices << " verts, " << stats.city_triangles
              << " tris, " << stats.city_batches << " batches\n"
              << "  param_hash " << std::hex << terrain.param_hash << std::dec << "\n";

    if (check_mode) {
        int failures = 0;
        auto expect = [&failures](bool condition, const char* label) {
            std::cout << (condition ? "PASS " : "FAIL ") << label << "\n";
            failures += condition ? 0 : 1;
        };
        expect(stats.terrain_width >= 64 && stats.terrain_height >= 64, "terrain grid resolved");
        expect(stats.terrain_min_m > -15.0f && stats.terrain_max_m < 150.0f &&
               stats.terrain_max_m > stats.terrain_min_m,
               "manhattan elevation range plausible");
        expect(terrain.validation.metrics.rmse < 5.0, "detail layer stays anchored to DEM (<5 m RMSE)");
        expect(stats.building_count > 1000, "more than 1000 buildings imported");
        expect(stats.max_building_height_m > 150.0 && stats.max_building_height_m < 400.0,
               "tallest building 150-400 m");
        expect(stats.city_triangles > 50000, "city mesh has >50k triangles");
        expect(stats.city_batches > 10, "spatial batching active");
        const auto readback = agbot::render::read_scene_file(out_path);
        expect(readback.ok() && readback.scene.static_meshes.size() == 2,
               "scene file round-trips with 2 meshes");
        if (failures != 0) {
            std::cout << failures << " failing checks\n";
            return 1;
        }
        std::cout << "world demo check passed\n";
    }
    return 0;
}
