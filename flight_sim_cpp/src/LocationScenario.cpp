#include "agbot_flight_sim/LocationScenario.hpp"

#include "agbot_flight_sim/DeterministicRunner.hpp"
#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/TwinContractV1.hpp"

#include <algorithm>
#include <cmath>
#include <iomanip>
#include <sstream>
#include <utility>

namespace agbot::flight_sim {
namespace {

std::string escape_json(std::string_view value) {
    std::ostringstream output;
    for (const char c : value) {
        switch (c) {
            case '"':
                output << "\\\"";
                break;
            case '\\':
                output << "\\\\";
                break;
            case '\n':
                output << "\\n";
                break;
            case '\r':
                output << "\\r";
                break;
            case '\t':
                output << "\\t";
                break;
            default:
                output << c;
                break;
        }
    }
    return output.str();
}

void write_geo_json(std::ostringstream& output, const GeoCoordinate& coordinate) {
    output << std::fixed << std::setprecision(7)
           << "{\"latitude\":" << coordinate.latitude
           << ",\"longitude\":" << coordinate.longitude
           << ",\"altitude_m\":" << coordinate.altitude_m << "}";
}

void write_bounds_json(std::ostringstream& output, const GeoBounds& bounds) {
    output << std::fixed << std::setprecision(7)
           << "{\"min_latitude\":" << bounds.min_latitude
           << ",\"min_longitude\":" << bounds.min_longitude
           << ",\"max_latitude\":" << bounds.max_latitude
           << ",\"max_longitude\":" << bounds.max_longitude << "}";
}

std::string location_scenario_json_without_hash(const LocationScenarioManifest& manifest) {
    std::ostringstream output;
    output << "{\"contract_version\":\"" << escape_json(manifest.contract_version) << "\""
           << ",\"center\":";
    write_geo_json(output, manifest.center);
    output << std::fixed << std::setprecision(3)
           << ",\"area_km2\":" << manifest.area_km2
           << ",\"scene_seed\":" << manifest.scene_seed
           << ",\"mission_hash\":\"" << escape_json(sha256_hex(mission_to_json(manifest.mission))) << "\""
           << ",\"terrain_profile\":{\"crs\":\"" << escape_json(manifest.terrain_profile.crs) << "\""
           << ",\"asserted\":" << (manifest.terrain_profile.asserted ? "true" : "false")
           << ",\"resolution\":" << manifest.terrain_profile.resolution
           << ",\"bounds\":";
    write_bounds_json(output, manifest.terrain_profile.bounds);
    output << "}"
           << ",\"terrain_tiles\":" << terrain_tiles_json(ElevationComposite{
                  {},
                  manifest.terrain_tiles,
                  manifest.terrain_profile,
              })
           << ",\"map_tiles\":" << map_texture_tiles_json(manifest.map_textures)
           << ",\"scene_hash\":\"" << escape_json(manifest.scene.scene_hash) << "\""
           << ",\"scene_status\":\"" << to_string(manifest.scene.status) << "\""
           << ",\"gaps\":[";
    for (std::size_t index = 0; index < manifest.gaps.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        output << "\"" << escape_json(manifest.gaps[index]) << "\"";
    }
    output << "]"
           << ",\"flyable\":" << (manifest.flyable ? "true" : "false")
           << "}";
    return output.str();
}

TerrainTileStatus available_terrain_tile(TileCoordinate tile) {
    return TerrainTileStatus{tile, TerrainTileState::Available, ""};
}

TerrainTileStatus missing_terrain_tile(TileCoordinate tile, std::string reason) {
    return TerrainTileStatus{tile, TerrainTileState::FlatFallback, std::move(reason)};
}

MapTextureTile available_map_tile(TileCoordinate tile) {
    return MapTextureTile{tile, MapTextureTileState::Available, "", 256, 256};
}

MapTextureTile unavailable_map_tile(TileCoordinate tile) {
    return MapTextureTile{
        tile,
        MapTextureTileState::TileUnavailable,
        "tile_source_unreachable",
        0,
        0,
    };
}

VegetationClassFeature default_crop_feature(const TerrainProfile& profile) {
    const GeoBounds& bounds = profile.bounds;
    return VegetationClassFeature{
        "location-default-crop",
        "crop_canopy",
        "mixed_crop",
        {
            {bounds.min_latitude, bounds.min_longitude, 0.0},
            {bounds.min_latitude, bounds.max_longitude, 0.0},
            {bounds.max_latitude, bounds.max_longitude, 0.0},
            {bounds.max_latitude, bounds.min_longitude, 0.0},
            {bounds.min_latitude, bounds.min_longitude, 0.0},
        },
        1.2,
        0.65,
    };
}

} // namespace

Mission mission_for_location(GeoCoordinate center, double area_km2) {
    const double radius_m = radius_m_for_area_km2(area_km2);
    const double half_extent_m = radius_m / std::sqrt(2.0);

    Mission mission;
    std::ostringstream name;
    name << "Location " << std::fixed << std::setprecision(6)
         << center.latitude << ", " << center.longitude;
    mission.name = name.str();
    mission.home = Vec3(0.0, 0.0, 0.0);
    mission.home_geo = center;
    mission.cruise_speed_mps = 12.0;
    mission.acceptance_radius_m = 3.0;

    const std::vector<std::pair<std::string, Vec3>> points = {
        {"takeoff", Vec3(0.0, 30.0, 0.0)},
        {"north_west", Vec3(-half_extent_m, 30.0, half_extent_m)},
        {"north_east", Vec3(half_extent_m, 30.0, half_extent_m)},
        {"south_east", Vec3(half_extent_m, 30.0, -half_extent_m)},
        {"south_west", Vec3(-half_extent_m, 30.0, -half_extent_m)},
        {"land", Vec3(0.0, 0.0, 0.0)},
    };

    for (const auto& [waypoint_name, position] : points) {
        Waypoint waypoint;
        waypoint.name = waypoint_name;
        waypoint.position = position;
        waypoint.geo = geo_from_local(position, center);
        waypoint.speed_mps = mission.cruise_speed_mps;
        if (waypoint_name == "takeoff") {
            waypoint.action = WaypointAction::Takeoff;
        } else if (waypoint_name == "land") {
            waypoint.action = WaypointAction::Land;
        } else {
            waypoint.action = WaypointAction::FlyThrough;
        }
        mission.waypoints.push_back(waypoint);
    }

    return mission;
}

LocationScenarioManifest load_location_scenario(const LocationScenarioRequest& request) {
    LocationScenarioManifest manifest;
    manifest.contract_version = kTwinContractVersion;
    manifest.center = request.center;
    manifest.area_km2 = std::clamp(request.area_km2, 1.0, 400.0);
    manifest.scene_seed = request.scene_seed;
    manifest.mission = mission_for_location(request.center, manifest.area_km2);

    const double radius_m = radius_m_for_area_km2(manifest.area_km2);
    const GeoBounds bounds = terrain_bounds_for_mission(manifest.mission, 40.0)
        .value_or(GeoBounds::from_center(request.center, radius_m));
    manifest.terrain_profile = terrain_profile_for_bounds(bounds, 96);
    const std::vector<TileCoordinate> expected_tiles =
        terrain_tiles_for_bounds_limited(bounds, radius_m, 16, 10);

    std::vector<MapTextureTile> map_tiles;
    for (const TileCoordinate& tile : expected_tiles) {
        manifest.terrain_tiles.push_back(
            request.mark_tiles_unavailable
                ? missing_terrain_tile(tile, "tile_source_unreachable")
                : available_terrain_tile(tile));
        map_tiles.push_back(
            request.mark_tiles_unavailable
                ? unavailable_map_tile(tile)
                : available_map_tile(tile));
    }
    manifest.map_textures = composite_map_textures_with_state(
        map_tiles,
        bounds,
        96,
        expected_tiles);

    SceneSynthesisInput scene_input;
    scene_input.seed = request.scene_seed;
    scene_input.profile = manifest.terrain_profile;
    if (!request.mark_features_unavailable) {
        scene_input.vegetation.push_back(default_crop_feature(manifest.terrain_profile));
    }
    manifest.scene = synthesize_scene_manifest(scene_input);

    if (request.mark_tiles_unavailable) {
        manifest.gaps.push_back("tile_source_unreachable");
    }
    if (manifest.scene.status == SceneSynthesisStatus::Unpopulated) {
        manifest.gaps.push_back("feature_source_unreachable");
    }
    if (!manifest.terrain_profile.asserted) {
        manifest.gaps.push_back("terrain_profile_not_asserted");
    }

    DroneSimulation simulation(manifest.mission);
    simulation.arm();
    for (int step = 0; step < 3 && !simulation.is_complete(); ++step) {
        simulation.step(0.05);
    }
    manifest.flyable = manifest.terrain_profile.asserted && !manifest.mission.waypoints.empty();
    manifest.scenario_hash = sha256_hex(location_scenario_json_without_hash(manifest));
    return manifest;
}

bool LocationScenarioManifest::has_gap(const std::string& gap) const {
    return std::find(gaps.begin(), gaps.end(), gap) != gaps.end();
}

std::string LocationScenarioManifest::to_json() const {
    std::string json = location_scenario_json_without_hash(*this);
    json.pop_back();
    json += ",\"scenario_hash\":\"" + escape_json(scenario_hash) + "\"}";
    return json;
}

} // namespace agbot::flight_sim
