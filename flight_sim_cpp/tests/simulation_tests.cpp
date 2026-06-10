#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_flight_sim/MissionLoader.hpp"
#include "agbot_flight_sim/TelemetryRecorder.hpp"
#include "agbot_flight_sim/TelemetryReplay.hpp"

#include <cassert>
#include <cmath>
#include <filesystem>
#include <iostream>

using agbot::flight_sim::DroneMode;
using agbot::flight_sim::DroneSimulation;
using agbot::flight_sim::ElevationTile;
using agbot::flight_sim::GeoBounds;
using agbot::flight_sim::GeoCoordinate;
using agbot::flight_sim::MissionLoader;
using agbot::flight_sim::ManualControlInput;
using agbot::flight_sim::ControlMode;
using agbot::flight_sim::TileCoordinate;
using agbot::flight_sim::TelemetryRecorder;
using agbot::flight_sim::TelemetryReplay;

namespace {

const char* kMissionJson = R"json(
{
  "name": "Unit Test Mission",
  "home": { "x": 0.0, "y": 0.0, "z": 0.0 },
  "cruise_speed_mps": 10.0,
  "acceptance_radius_m": 0.5,
  "waypoints": [
    { "name": "takeoff", "action": "takeoff", "x": 0.0, "y": 10.0, "z": 0.0 },
    { "name": "leg_1", "action": "fly", "x": 20.0, "y": 10.0, "z": 0.0 },
    { "name": "hover", "action": "loiter", "x": 20.0, "y": 10.0, "z": 20.0, "hold_seconds": 1.0 },
    { "name": "land", "action": "land", "x": 0.0, "y": 0.0, "z": 0.0 }
  ]
}
)json";

const char* kGeoMissionJson = R"json(
{
  "name": "Geo Test Mission",
  "home_position": {
    "latitude": 37.7749,
    "longitude": -122.4194,
    "altitude": 0.0
  },
  "waypoints": [
    {
      "sequence": 0,
      "position": {
        "latitude": 37.7750,
        "longitude": -122.4195,
        "altitude": 30.0
      },
      "command": 22
    },
    {
      "sequence": 1,
      "position": {
        "latitude": 37.7751,
        "longitude": -122.4195,
        "altitude": 30.0
      },
      "command": 16
    }
  ]
}
)json";

void write_terrarium_pixel(std::vector<std::uint8_t>& pixels, int index, float elevation_m) {
    const double encoded = static_cast<double>(elevation_m) + 32768.0;
    const auto r = static_cast<std::uint8_t>(std::floor(encoded / 256.0));
    const auto g = static_cast<std::uint8_t>(std::floor(encoded - static_cast<double>(r) * 256.0));
    const auto b = static_cast<std::uint8_t>(std::round((encoded - std::floor(encoded)) * 256.0));
    const std::size_t offset = static_cast<std::size_t>(index * 4);
    pixels[offset] = r;
    pixels[offset + 1] = g;
    pixels[offset + 2] = b;
    pixels[offset + 3] = 255;
}

void test_loads_mission() {
    const auto mission = MissionLoader::load_from_text(kMissionJson);
    assert(mission.name == "Unit Test Mission");
    assert(mission.waypoints.size() == 4);
    assert(std::abs(mission.cruise_speed_mps - 10.0) < 1e-9);
    assert(std::abs(mission.acceptance_radius_m - 0.5) < 1e-9);
}

void test_loads_geodetic_mission() {
    const auto mission = MissionLoader::load_from_text(kGeoMissionJson);
    assert(mission.name == "Geo Test Mission");
    assert(mission.home_geo.has_value());
    assert(mission.waypoints.size() == 2);
    assert(mission.waypoints[0].geo.has_value());
    assert(mission.waypoints[0].position.y == 30.0);
    assert(std::abs(mission.waypoints[0].position.x) > 1.0);
    assert(std::abs(mission.waypoints[0].position.z) > 1.0);

    const std::string json = agbot::flight_sim::mission_to_json(mission);
    const auto reloaded = MissionLoader::load_from_text(json);
    assert(reloaded.home_geo.has_value());
    assert(reloaded.waypoints[0].geo.has_value());
    assert(std::abs(reloaded.waypoints[0].geo->latitude - 37.7750) < 1e-6);
}

void test_mission_completes() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    DroneSimulation simulation(std::move(mission));

    constexpr double dt_s = 1.0 / 60.0;
    for (int i = 0; i < 60 * 45 && !simulation.is_complete(); ++i) {
        simulation.step(dt_s);
    }

    assert(simulation.state().mode == DroneMode::Completed);
    assert(simulation.state().target_waypoint_index == 4);
    assert(simulation.state().battery_percent > 90.0);
}

void test_manual_controls_move_drone() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    DroneSimulation simulation(std::move(mission));
    simulation.set_control_mode(ControlMode::Manual);
    simulation.arm();

    ManualControlInput input;
    input.arm = true;
    input.takeoff = true;
    input.pitch = 1.0;
    simulation.set_manual_input(input);

    for (int i = 0; i < 60 * 3; ++i) {
        simulation.step(1.0 / 60.0);
    }

    assert(simulation.state().position.y > 5.0);
    assert(simulation.state().position.z > 5.0);
    assert(simulation.state().mode != DroneMode::Completed);
}

void test_mission_round_trip() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    const std::string json = agbot::flight_sim::mission_to_json(mission);
    const auto reloaded = MissionLoader::load_from_text(json);
    assert(reloaded.name == mission.name);
    assert(reloaded.waypoints.size() == mission.waypoints.size());
}

void test_failsafe_low_battery() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    agbot::flight_sim::SimulationConfig config;
    config.min_battery_percent = 100.0;
    DroneSimulation simulation(std::move(mission), config);
    simulation.step(1.0);
    assert(simulation.state().mode == DroneMode::Failsafe);
}

void test_telemetry_recorder_close_is_idempotent() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    DroneSimulation simulation(std::move(mission));
    const auto output = std::filesystem::temp_directory_path() / "agbot_flight_sim_recorder_test.jsonl";

    TelemetryRecorder recorder(output);
    recorder.write_sample(simulation.state());
    assert(recorder.is_open());
    recorder.close();
    assert(!recorder.is_open());
    recorder.close();

    const auto replay = TelemetryReplay::load_jsonl(output);
    assert(!replay.empty());
    std::filesystem::remove(output);
}

void test_geo_terrain_area_and_tiles() {
    const double radius = agbot::flight_sim::radius_m_for_area_km2(20.0);
    assert(std::abs(radius - 2523.1325) < 0.5);
    assert(agbot::flight_sim::zoom_for_radius_m(radius) == 12);

    const GeoCoordinate center {37.7749, -122.4194, 0.0};
    const GeoBounds bounds = GeoBounds::from_center(center, radius);
    assert(std::abs(bounds.width_m() - radius * 2.0) < 5.0);
    assert(std::abs(bounds.height_m() - radius * 2.0) < 5.0);

    const TileCoordinate tile = agbot::flight_sim::tile_for_geo(center, 12);
    const GeoBounds tile_bounds = tile.bounds();
    assert(center.latitude >= tile_bounds.min_latitude);
    assert(center.latitude <= tile_bounds.max_latitude);
    assert(center.longitude >= tile_bounds.min_longitude);
    assert(center.longitude <= tile_bounds.max_longitude);

    const auto tiles = agbot::flight_sim::tiles_for_bounds(bounds, 12);
    assert(!tiles.empty());
}

void test_decodes_terrarium_elevation_without_png_dependency() {
    std::vector<std::uint8_t> pixels(2 * 2 * 4, 0);
    write_terrarium_pixel(pixels, 0, 10.0f);
    write_terrarium_pixel(pixels, 1, 20.0f);
    write_terrarium_pixel(pixels, 2, 30.0f);
    write_terrarium_pixel(pixels, 3, 40.0f);

    const auto tile = agbot::flight_sim::elevation_tile_from_terrarium_rgba({12, 655, 1583}, 2, 2, pixels);
    assert(tile.has_value());
    assert(std::abs(tile->min_elevation_m - 10.0f) < 0.01f);
    assert(std::abs(tile->max_elevation_m - 40.0f) < 0.01f);
    assert(std::abs(tile->sample_bilinear(0.5, 0.5) - 25.0f) < 0.01f);

    const auto invalid = agbot::flight_sim::elevation_tile_from_terrarium_rgba({12, 0, 0}, 2, 2, {1, 2, 3});
    assert(!invalid.has_value());
}

void test_composites_empty_elevation_as_flat_zero() {
    const GeoCoordinate center {37.7749, -122.4194, 0.0};
    const GeoBounds bounds = GeoBounds::from_center(center, 500.0);
    const auto heightmap = agbot::flight_sim::composite_elevation({}, bounds, 4);
    assert(heightmap.size() == 16);
    for (const float elevation : heightmap) {
        assert(elevation == 0.0f);
    }

    const auto mesh = agbot::flight_sim::build_terrain_mesh(heightmap, 4, bounds.width_m(), bounds.height_m());
    assert(mesh.vertices.size() == 16);
    assert(mesh.indices.size() == 54);
    assert(!mesh.has_elevation);
    assert(mesh.min_elevation_m == 0.0f);
    assert(mesh.max_elevation_m == 0.0f);
}

void test_builds_terrain_mesh_from_heightmap() {
    const std::vector<float> heightmap {
        10.0f, 10.0f, 10.0f,
        10.0f, 20.0f, 10.0f,
        10.0f, 10.0f, 10.0f,
    };
    const auto mesh = agbot::flight_sim::build_terrain_mesh(heightmap, 3, 20.0, 20.0);
    assert(mesh.vertices.size() == 9);
    assert(mesh.indices.size() == 24);
    assert(mesh.has_elevation);
    assert(std::abs(mesh.min_elevation_m - 10.0f) < 0.001f);
    assert(std::abs(mesh.max_elevation_m - 20.0f) < 0.001f);
    assert(std::abs(mesh.vertices[4].position.y - 10.0) < 0.001);
    assert(std::abs(mesh.vertices[4].normal.length() - 1.0) < 0.001);
}

} // namespace

int main() {
    test_loads_mission();
    test_loads_geodetic_mission();
    test_mission_completes();
    test_manual_controls_move_drone();
    test_mission_round_trip();
    test_failsafe_low_battery();
    test_telemetry_recorder_close_is_idempotent();
    test_geo_terrain_area_and_tiles();
    test_decodes_terrarium_elevation_without_png_dependency();
    test_composites_empty_elevation_as_flat_zero();
    test_builds_terrain_mesh_from_heightmap();
    std::cout << "agbot_flight_sim_tests passed\n";
    return 0;
}
