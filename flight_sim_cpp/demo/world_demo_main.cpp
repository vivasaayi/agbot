// agbot_world_demo: builds the realistic-world demo scene for Lower Manhattan.
//
// Pipeline: worldgen::compile_world (terrain_engine DEM + detail fusion + NYC
// footprint extrusion + OSM basemap draping) emits an .agbworld manifest with
// per-tile provenance and deterministic hashes plus the base .agbscn scene.
// This driver then overlays a Cessna flythrough and a delivery-robot street
// route as demonstration markers and writes the final artifacts.
//
// Usage:
//   agbot_world_demo            build scene, write out/world/manhattan.{agbscn,agbworld}
//   agbot_world_demo --check    build scene, assert invariants, exit 0/1

#include "agbot_config/Params.hpp"
#include "agbot_nav/AerialPlanner.hpp"
#include "agbot_nav/RoadGraphPlanner.hpp"
#include "agbot_render/SceneFile.hpp"
#include "agbot_vehicles/FixedWingAutopilot.hpp"
#include "agbot_vehicles/FixedWingModel.hpp"
#include "agbot_worldgen/RoadNetwork.hpp"
#include "agbot_worldgen/WorldCompiler.hpp"

#include <array>
#include <cmath>
#include <filesystem>
#include <iostream>
#include <memory>
#include <string>

namespace {

namespace cfg = agbot::config;
namespace fs = agbot::flight_sim;

// Terrain config with the authoritative bare-earth DEM (layer 0, the Gate 2
// reference) plus subordinate synthetic detail. When the 3DEP GeoTIFF is
// present it is the ground-truth base (NAVD88 metres); otherwise the compiler
// falls back to Terrarium tiles so a data-less checkout still builds.
std::string terrain_config_toml(const std::string& dem_geotiff_path) {
    std::string base_layer;
    if (!dem_geotiff_path.empty()) {
        base_layer =
            "[[layer]]\n"
            "algorithm = \"dem_fusion\"\n"
            "weight = 1.0\n"
            "  [layer.params]\n"
            "  source = \"geotiff\"\n"
            "  path = \"" + dem_geotiff_path + "\"\n"
            "  resample = \"bilinear\"\n"
            "  clamp_min_m = -30.0\n";
    } else {
        base_layer =
            "[[layer]]\n"
            "algorithm = \"dem_fusion\"\n"
            "weight = 1.0\n"
            "  [layer.params]\n"
            "  source = \"terrarium\"\n"
            "  zoom = 13\n"
            "  resample = \"bilinear\"\n"
            "  void_fill = \"idw\"\n"
            "  clamp_min_m = -2.0\n";
    }
    return
        "[pipeline]\n"
        "target_gsd_m = 30.0\n"
        "resolution = 128\n"
        "aoi = { min_lat = 40.700, min_lon = -74.020, max_lat = 40.740, max_lon = -73.980 }\n\n" +
        base_layer +
        "\n[[layer]]\n"
        "algorithm = \"synthetic_detail\"\n"
        "weight = 1.0\n"
        "  [layer.params]\n"
        "  amplitude_m = 0.6\n"
        "  octaves = 4\n"
        "  frequency = 8.0\n"
        "  seed = 1337\n"
        "  confidence = 0.3\n\n"
        "[fusion]\n"
        "method = \"detail_injection\"\n"
        "lambda = 0.3\n"
        "cutoff_cells = 2\n\n"
        "[validation]\n"
        "enabled = true\n"
        "reference_layer = 0\n"
        "output_json = \"out/world/terrain_validation.json\"\n";
}

struct FlythroughResult {
    bool completed = false;
    double max_altitude_error_m = 0.0;
    double elapsed_s = 0.0;
    std::vector<agbot::render::RenderScene::Marker> trail;
};

// Fly a Dubins-planned rectangular circuit above the city and trace the actual
// 6-DOF flight path into the scene as markers.
FlythroughResult fly_circuit(double cruise_alt_m, double airspeed_mps) {
    FlythroughResult flight;
    agbot::vehicles::FixedWingModel cessna;
    agbot::vehicles::FixedWingAutopilot autopilot;

    cfg::ParamTable planner_params;
    planner_params["turn_radius_m"] = cfg::ParamValue(450.0);
    planner_params["sample_spacing_m"] = cfg::ParamValue(40.0);
    const agbot::nav::DubinsAirplanePlanner planner(planner_params);

    const std::array<agbot::nav::AirPose, 4> corners = {{
        {-1200.0, -1200.0, 0.0, cruise_alt_m},
        {1200.0, -1200.0, 1.5707963, cruise_alt_m},
        {1200.0, 1200.0, 3.1415926, cruise_alt_m},
        {-1200.0, 1200.0, -1.5707963, cruise_alt_m},
    }};
    std::vector<fs::Vec3> route;
    for (std::size_t leg = 0; leg < corners.size(); ++leg) {
        const auto plan = planner.plan(corners[leg], corners[(leg + 1) % corners.size()]);
        if (!plan.ok) {
            return flight;
        }
        route.insert(route.end(), plan.path.points.begin(), plan.path.points.end());
    }

    agbot::vehicles::EntityState state = cessna.set_initial_trim(
        cruise_alt_m, airspeed_mps, corners[0].heading_rad, corners[0].x, corners[0].z);
    autopilot.reset(cessna.trim_controls());

    constexpr double kDt = 0.02;
    constexpr double kLookaheadM = 250.0;
    std::size_t target_index = 0;
    double marker_accum_s = 0.0;
    const double time_budget_s = 1.35 * (8.0 * 2400.0) / airspeed_mps;
    while (flight.elapsed_s < time_budget_s) {
        while (target_index + 1 < route.size()) {
            const double dx = route[target_index].x - state.position.x;
            const double dz = route[target_index].z - state.position.z;
            if (std::sqrt(dx * dx + dz * dz) > kLookaheadM) {
                break;
            }
            ++target_index;
        }
        if (target_index + 1 >= route.size()) {
            flight.completed = true;
            break;
        }
        const fs::Vec3& target = route[target_index];
        agbot::vehicles::AutopilotCommand command;
        command.heading_rad =
            std::atan2(target.z - state.position.z, target.x - state.position.x);
        command.altitude_m = cruise_alt_m;
        command.airspeed_mps = airspeed_mps;
        cessna.set_controls(autopilot.update(state, cessna.body_rates(), command, kDt));
        state = cessna.step(state, {}, kDt);
        flight.elapsed_s += kDt;
        flight.max_altitude_error_m = std::max(
            flight.max_altitude_error_m, std::abs(state.position.y - cruise_alt_m));
        marker_accum_s += kDt;
        if (marker_accum_s >= 4.0) {
            marker_accum_s = 0.0;
            flight.trail.push_back({static_cast<float>(state.position.x),
                                    static_cast<float>(state.position.y),
                                    static_cast<float>(state.position.z),
                                    1.0f, 0.85f, 0.1f, 8.0f});
        }
    }
    return flight;
}

struct StreetRouteResult {
    bool attempted = false;
    bool ok = false;
    double length_m = 0.0;
    double euclidean_m = 0.0;
    std::vector<agbot::render::RenderScene::Marker> trail;
};

// Plan a delivery-robot route along the compiled OSM road graph and trace it
// into the scene. Soft-skips when the compile found no road data.
StreetRouteResult plan_street_route(const std::vector<agbot::worldgen::ExtractedFeature>& roads,
                                    const fs::GeoCoordinate& origin) {
    StreetRouteResult street;
    if (roads.empty()) {
        return street;
    }
    street.attempted = true;
    auto network = std::make_shared<agbot::worldgen::RoadNetwork>(
        agbot::worldgen::RoadNetwork::build(
            roads, origin, agbot::worldgen::road_network_params_from({})));
    agbot::nav::RoadGraphPlanner planner;
    planner.set_network(network);
    const fs::Vec3 start{-1200.0, 0.0, -800.0};
    const fs::Vec3 goal{1200.0, 0.0, 800.0};
    const agbot::nav::PlanResult route = planner.plan(agbot::nav::Costmap{}, start, goal);
    street.ok = route.ok;
    if (route.ok && route.path.points.size() > 1) {
        street.euclidean_m = std::sqrt((goal.x - start.x) * (goal.x - start.x) +
                                       (goal.z - start.z) * (goal.z - start.z));
        for (std::size_t i = 1; i < route.path.points.size(); ++i) {
            const fs::Vec3& a = route.path.points[i - 1];
            const fs::Vec3& b = route.path.points[i];
            street.length_m +=
                std::sqrt((b.x - a.x) * (b.x - a.x) + (b.z - a.z) * (b.z - a.z));
        }
        for (std::size_t i = 0; i < route.path.points.size(); i += 6) {
            const fs::Vec3& p = route.path.points[i];
            street.trail.push_back({static_cast<float>(p.x), 8.0f, static_cast<float>(p.z),
                                    0.15f, 0.9f, 0.3f, 6.0f});
        }
    }
    return street;
}

} // namespace

int main(int argc, char** argv) {
    const bool check_mode = argc > 1 && std::string(argv[1]) == "--check";
    const std::filesystem::path source_dir = AGBOT_FLIGHT_SIM_SOURCE_DIR;

    // --- Compile the world artifact ----------------------------------------
    const std::filesystem::path dem_path = source_dir / "data/terrain/manhattan_3dep_dem.tif";
    const std::string dem_geotiff = std::filesystem::exists(dem_path) ? dem_path.string() : "";

    agbot::worldgen::WorldCompileSpec spec;
    spec.seed = 1337;
    spec.terrain_config_toml = terrain_config_toml(dem_geotiff);
    if (!dem_geotiff.empty()) {
        spec.terrain_license = "USGS 3DEP (public domain)";
        spec.terrain_uri =
            "https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer";
        spec.terrain_version = "3DEP exportImage 512px";
        spec.terrain_vertical_datum = "NAVD88";
        spec.terrain_authoritative = true;
        spec.water_mask = true;
    } else {
        spec.terrain_license = "AWS Terrarium (mixed source licenses)";
        spec.terrain_uri = "s3://elevation-tiles-prod/terrarium";
    }

    spec.buildings_path = (source_dir / "data/worldgen/manhattan_buildings.geojson").string();
    spec.buildings_license = "NYC Open Data (public domain)";
    spec.buildings_uri = "https://data.cityofnewyork.us/Housing-Development/Building-Footprints";
    // NYC building ground_elevation is referenced to NAVD88, matching 3DEP.
    spec.buildings_vertical_datum = dem_geotiff.empty() ? "" : "NAVD88";
    spec.building_params["height_attr"] = cfg::ParamValue(std::string("height_roof"));
    spec.building_params["height_units"] = cfg::ParamValue(std::string("feet"));
    spec.building_params["base_elev_attr"] = cfg::ParamValue(std::string("ground_elevation"));
    spec.building_params["base_units"] = cfg::ParamValue(std::string("feet"));
    spec.building_params["id_attr"] = cfg::ParamValue(std::string("bin"));
    spec.building_params["min_area_m2"] = cfg::ParamValue(10.0);

    spec.roads_path = (source_dir / "data/worldgen/manhattan_roads.json").string();
    spec.roads_license = "OpenStreetMap (ODbL)";
    spec.roads_uri = "https://overpass-api.de/api/interpreter";

    spec.basemap_source_dir = source_dir;
    spec.basemap_license = "OpenStreetMap (ODbL)";

    agbot::worldgen::WorldCompileResult world = agbot::worldgen::compile_world(spec);
    if (!world.ok) {
        if (world.error_code == "buildings_file_missing") {
            std::cerr << "SKIP: building data missing; run "
                         "worldgen/tools/fetch_nyc_buildings.sh first ("
                      << world.error_detail << ")\n";
            return 77; // ctest SKIP_RETURN_CODE
        }
        std::cerr << "world compile failed: " << world.error_code << " — "
                  << world.error_detail << "\n";
        return 1;
    }

    // --- Demonstration overlays --------------------------------------------
    const FlythroughResult flight = fly_circuit(400.0, 55.0);
    const StreetRouteResult street = plan_street_route(world.roads, world.origin);

    world.scene.markers.push_back({0.0f, 320.0f, 0.0f, 1.0f, 0.25f, 0.2f, 12.0f});
    world.scene.markers.insert(world.scene.markers.end(), flight.trail.begin(),
                               flight.trail.end());
    world.scene.markers.insert(world.scene.markers.end(), street.trail.begin(),
                               street.trail.end());

    // --- Persist .agbscn + .agbworld ---------------------------------------
    const std::filesystem::path out_dir = source_dir / "out/world";
    const agbot::worldgen::WorldWriteResult written =
        agbot::worldgen::write_world_artifacts(world, out_dir, "manhattan");
    if (!written.ok) {
        std::cerr << "world write failed: " << written.error_code << " — "
                  << written.error_detail << "\n";
        return 1;
    }

    // --- Stats + evidence ---------------------------------------------------
    const agbot::worldgen::WorldQuality& q = world.manifest.quality;
    std::cout << "manhattan world scene: " << written.scene_path.string() << "\n"
              << "  manifest: " << written.manifest_path.string() << " (world_hash "
              << world.manifest.world_hash << ")\n"
              << "  terrain grid " << world.terrain.elevation.width << "x"
              << world.terrain.elevation.height << ", elevation " << q.terrain_min_m << ".."
              << q.terrain_max_m << " m (source: " << world.terrain.source_algorithm << ")\n"
              << "  terrain datum: " << world.manifest.crs_policy.vertical_datum
              << ", Gate 2 RMSE vs authoritative DEM: " << q.terrain_rmse_m << " m (MAE "
              << q.terrain_mae_m << ", bias " << q.terrain_bias_m << ")\n"
              << "  terrain cells: " << q.terrain_cell_count << " ("
              << q.terrain_authoritative_cells << " authoritative, " << q.terrain_water_cells
              << " water, " << q.terrain_nodata_cells << " nodata), state "
              << to_string(world.manifest.tiles.front().elevation_state) << "\n"
              << "  buildings " << q.building_count << ", max height "
              << q.max_building_height_m << " m, median " << q.median_building_height_m
              << " m, footprint area " << q.building_footprint_area_m2 << " m2, courtyards "
              << q.building_with_courtyard_count << "\n"
              << "  height source: measured " << q.height_from_measured << ", attr "
              << q.height_from_attribute << ", levels " << q.height_from_levels << ", default "
              << q.height_from_default << "\n"
              << "  city mesh " << q.city_vertex_count << " verts, " << q.city_triangle_count
              << " tris, " << q.city_batch_count << " batches\n"
              << "  terrain basemap: "
              << (world.terrain_textured ? "OSM tiles draped" : "height-colored fallback") << "\n"
              << "  street route: "
              << (street.attempted
                      ? (street.ok ? std::to_string(street.length_m) + " m over roads (" +
                                         std::to_string(street.euclidean_m) + " m euclidean)"
                                   : std::string("FAILED"))
                      : std::string("skipped (no road data)"))
              << "\n"
              << "  cessna circuit: " << (flight.completed ? "completed" : "incomplete") << " in "
              << flight.elapsed_s << " s, max altitude error " << flight.max_altitude_error_m
              << " m, trail markers " << flight.trail.size() << "\n";

    if (check_mode) {
        int failures = 0;
        auto expect = [&failures](bool condition, const char* label) {
            std::cout << (condition ? "PASS " : "FAIL ") << label << "\n";
            failures += condition ? 0 : 1;
        };
        // Authoritative-DEM runs are identified by the recorded NAVD88 datum
        // (set only when the 3DEP GeoTIFF was consumed).
        const bool authoritative = world.manifest.crs_policy.vertical_datum == "NAVD88";
        bool terrain_source_is_3dep = false;
        for (const auto& source : world.manifest.sources) {
            if (source.source_id == "terrain") {
                terrain_source_is_3dep =
                    source.license.find("3DEP") != std::string::npos &&
                    source.vertical_datum == "NAVD88";
            }
        }
        expect(world.terrain.elevation.width >= 64 && world.terrain.elevation.height >= 64,
               "terrain grid resolved");
        // Bare-earth 3DEP reaches ~-16 m at native resolution (harbor/excavation);
        // both sources stay well under Lower Manhattan's high ground.
        expect(q.terrain_min_m > -30.0f && q.terrain_max_m < 150.0f &&
                   q.terrain_max_m > q.terrain_min_m,
               "manhattan elevation range plausible");
        expect(!authoritative || terrain_source_is_3dep,
               "authoritative run records a 3DEP/NAVD88 terrain source");
        // Gate 2: the compiled terrain must track its authoritative reference DEM.
        // Authoritative 3DEP is held to a tighter bound than the Terrarium fallback.
        expect(q.terrain_rmse_m < (authoritative ? 2.0 : 5.0),
               "Gate 2: fused terrain stays anchored to the reference DEM");
        expect(dem_geotiff.empty() || authoritative,
               "3DEP DEM, when present, is compiled as the authoritative terrain");
        // No-silent-zero: the tile carries an explicit elevation state.
        const auto& tile0 = world.manifest.tiles.front();
        const bool state_consistent =
            authoritative
                ? (tile0.elevation_state == agbot::worldgen::ElevationState::Authoritative)
                : (tile0.elevation_state == agbot::worldgen::ElevationState::Fallback);
        expect(state_consistent, "tile elevation_state matches the terrain source class");
        expect(q.terrain_cell_count > 0 &&
                   q.terrain_authoritative_cells + q.terrain_nodata_cells <= q.terrain_cell_count,
               "terrain cell accounting is consistent");
        expect(!authoritative || (q.terrain_nodata_cells == 0 &&
                                  q.terrain_authoritative_cells == q.terrain_cell_count),
               "authoritative AOI has full DEM coverage (no silent-zero gaps)");
        // Gate 3 (buildings): geometry + height sanity vs the authoritative NYC source.
        expect(q.building_count > 1000, "more than 1000 buildings imported");
        expect(q.max_building_height_m > 150.0 && q.max_building_height_m < 400.0,
               "tallest building 150-400 m");
        expect(q.median_building_height_m > 5.0 && q.median_building_height_m < 120.0,
               "Gate 3: median building height plausible");
        expect(q.building_footprint_area_m2 > 100000.0,
               "Gate 3: total footprint area is substantial");
        expect(q.height_from_attribute > q.building_count / 2,
               "Gate 3: most heights come from the authoritative height attribute");
        expect(q.height_from_measured + q.height_from_attribute + q.height_from_levels +
                       q.height_from_default ==
                   q.building_count,
               "Gate 3: every building has a ranked height provenance");
        expect(q.city_triangle_count > 50000, "city mesh has >50k triangles");
        expect(q.city_batch_count > 10, "spatial batching active");
        expect(world.manifest.world_hash != 0, "world manifest carries a content hash");
        expect(!world.manifest.tiles.empty() &&
                   world.manifest.tiles.front().provenance.size() >= 2,
               "tile records terrain + building provenance");
        expect(!street.attempted || street.ok, "street route plans over the OSM road graph");
        expect(!street.ok || (street.length_m > street.euclidean_m &&
                              street.length_m < 2.5 * street.euclidean_m),
               "street route length plausible (1..2.5x euclidean)");
        expect(!street.ok || street.trail.size() > 10, "street route traced into the scene");
        expect(flight.completed, "cessna completes the Dubins circuit over the city");
        expect(flight.max_altitude_error_m < 30.0, "cessna altitude held within 30 m");
        expect(flight.trail.size() > 30, "flight trail traced into the scene");
        const auto readback = agbot::render::read_scene_file(written.scene_path);
        expect(readback.ok() &&
                   readback.scene.static_meshes.size() + readback.scene.textured_meshes.size() == 2,
               "scene file round-trips with 2 meshes");
        expect(!world.terrain_textured || readback.scene.textured_meshes.size() == 1,
               "draped basemap terrain survives scene round-trip");
        if (failures != 0) {
            std::cout << failures << " failing checks\n";
            return 1;
        }
        std::cout << "world demo check passed\n";
    }
    return 0;
}
