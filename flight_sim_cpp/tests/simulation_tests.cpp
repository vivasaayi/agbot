#include "agbot_flight_sim/DeterministicRunner.hpp"
#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/FaultInjection.hpp"
#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_flight_sim/LidarSimulator.hpp"
#include "agbot_flight_sim/MissionLoader.hpp"
#include "agbot_flight_sim/MissionPreview.hpp"
#include "agbot_flight_sim/SensorModel.hpp"
#include "agbot_flight_sim/SafetyRules.hpp"
#include "agbot_flight_sim/SimulationOps.hpp"
#include "agbot_flight_sim/TelemetryRecorder.hpp"
#include "agbot_flight_sim/TelemetryReplay.hpp"
#include "agbot_flight_sim/TraceDiff.hpp"
#include "agbot_flight_sim/TwinBackend.hpp"
#include "agbot_flight_sim/TwinContractV1.hpp"

#include <cassert>
#include <chrono>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <optional>
#include <sstream>
#include <string>
#include <vector>

using agbot::flight_sim::DroneMode;
using agbot::flight_sim::DroneSimulation;
using agbot::flight_sim::ElevationTile;
using agbot::flight_sim::GeoBounds;
using agbot::flight_sim::GeoCoordinate;
using agbot::flight_sim::MissionLoader;
using agbot::flight_sim::ManualControlInput;
using agbot::flight_sim::ControlMode;
using agbot::flight_sim::SimulationEventType;
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

const char* kFieldPreviewMissionJson = R"json(
{
  "name": "Field Preview Mission",
  "home_position": {
    "latitude": 40.0005,
    "longitude": -96.0005,
    "altitude": 0.0
  },
  "field_boundary": {
    "field_id": "north-field",
    "coordinates": [
      { "latitude": 40.0000, "longitude": -96.0010 },
      { "latitude": 40.0000, "longitude": -96.0000 },
      { "latitude": 40.0010, "longitude": -96.0000 },
      { "latitude": 40.0010, "longitude": -96.0010 },
      { "latitude": 40.0000, "longitude": -96.0010 }
    ]
  },
  "waypoints": [
    {
      "sequence": 0,
      "position": {
        "latitude": 40.00025,
        "longitude": -96.00075,
        "altitude": 25.0
      },
      "command": 22
    },
    {
      "sequence": 1,
      "position": {
        "latitude": 40.00075,
        "longitude": -96.00025,
        "altitude": 25.0
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

void test_mission_preview_overlay_aligns_field_boundary_and_coverage() {
    const auto mission = MissionLoader::load_from_text(kFieldPreviewMissionJson);
    const auto overlay = agbot::flight_sim::build_mission_preview_overlay(mission);

    assert(overlay.has_boundary);
    assert(overlay.status == "field boundary aligned");
    assert(overlay.boundary_local.size() == 5);
    assert(overlay.mission_path_local.size() == 3);
    assert(overlay.coverage_fraction > 0.99);
    assert(std::abs(overlay.boundary_local.front().x - overlay.boundary_local.back().x) < 1e-6);
    assert(std::abs(overlay.boundary_local.front().z - overlay.boundary_local.back().z) < 1e-6);
}

void test_mission_preview_overlay_reports_missing_boundary() {
    const auto mission = MissionLoader::load_from_text(kGeoMissionJson);
    const auto overlay = agbot::flight_sim::build_mission_preview_overlay(mission);

    assert(!overlay.has_boundary);
    assert(overlay.status == "no boundary");
    assert(overlay.boundary_local.empty());
    assert(overlay.coverage_fraction == 0.0);
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

void test_simulation_events_follow_documented_normal_order() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    DroneSimulation simulation(std::move(mission));

    constexpr double dt_s = 1.0 / 60.0;
    for (int i = 0; i < 60 * 45 && !simulation.is_complete(); ++i) {
        simulation.step(dt_s);
    }

    const auto& events = simulation.events();
    assert(!events.empty());
    assert(events.size() % 4 == 0);

    std::vector<DroneMode> status_modes;
    for (std::size_t index = 0; index < events.size(); index += 4) {
        assert(events[index].type == SimulationEventType::Position);
        assert(events[index + 1].type == SimulationEventType::Sensor);
        assert(events[index + 2].type == SimulationEventType::Battery);
        assert(events[index + 3].type == SimulationEventType::Status);
        assert(events[index + 3].mode == events[index].mode);
        assert(events[index + 3].battery_percent == events[index + 2].battery_percent);
        if (status_modes.empty() || status_modes.back() != events[index + 3].mode) {
            status_modes.push_back(events[index + 3].mode);
        }
    }

    bool saw_takeoff = false;
    bool saw_flying = false;
    bool saw_loiter = false;
    bool saw_landing = false;
    bool saw_completed = false;
    for (const DroneMode mode : status_modes) {
        if (mode == DroneMode::Takeoff) {
            saw_takeoff = true;
        } else if (mode == DroneMode::Flying) {
            assert(saw_takeoff);
            saw_flying = true;
        } else if (mode == DroneMode::Loiter) {
            assert(saw_flying);
            saw_loiter = true;
        } else if (mode == DroneMode::Landing) {
            assert(saw_loiter);
            saw_landing = true;
        } else if (mode == DroneMode::Completed) {
            assert(saw_landing);
            saw_completed = true;
        }
    }
    assert(saw_takeoff);
    assert(saw_flying);
    assert(saw_loiter);
    assert(saw_landing);
    assert(saw_completed);
}

void test_simulation_emergency_event_suppresses_normal_events() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    DroneSimulation simulation(std::move(mission));
    simulation.step(1.0 / 60.0);
    simulation.clear_events();

    simulation.request_emergency_abort();
    simulation.step(1.0 / 60.0);
    simulation.step(1.0 / 60.0);

    const auto& events = simulation.events();
    assert(events.size() == 1);
    assert(events.front().type == SimulationEventType::Emergency);
    assert(events.front().mode == DroneMode::Failsafe);
    assert(events.front().safety_code.has_value());
    assert(events.front().safety_code.value() == agbot::flight_sim::SafetyViolationCode::EmergencyAbort);
    assert(simulation.state().mode == DroneMode::Failsafe);
    assert(simulation.last_safety_violation().has_value());
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

void test_steady_wind_disturbs_ground_track() {
    auto calm_mission = MissionLoader::load_from_text(kMissionJson);
    DroneSimulation calm(std::move(calm_mission));
    calm.step(1.0);

    auto windy_mission = MissionLoader::load_from_text(kMissionJson);
    DroneSimulation windy(std::move(windy_mission));
    windy.set_wind({3.0, 0.0, 0.0});
    windy.step(1.0);

    assert(windy.wind().x == 3.0);
    assert(windy.state().position.x > calm.state().position.x + 0.1);
}

void test_mission_round_trip() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    const std::string json = agbot::flight_sim::mission_to_json(mission);
    const auto reloaded = MissionLoader::load_from_text(json);
    assert(reloaded.name == mission.name);
    assert(reloaded.waypoints.size() == mission.waypoints.size());
}

void test_twin_backend_executes_shared_command_and_returns_telemetry() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    agbot::flight_sim::TwinBackend backend(std::move(mission));

    agbot::flight_sim::FlightCommandV1 arm_command;
    arm_command.command_id = "cmd-arm-1";
    arm_command.command_type = agbot::flight_sim::TwinCommandType::Arm;
    arm_command.issued_at_unix_ms = 1'800'000'000'000;

    const auto arm_ack = backend.dispatch(arm_command);
    assert(arm_ack.accepted);
    assert(!arm_ack.error.has_value());
    assert(arm_ack.telemetry.has_value());
    assert(arm_ack.telemetry->armed);
    assert(arm_ack.telemetry->contract_version == agbot::flight_sim::kTwinContractVersion);
    assert(arm_ack.telemetry->command_id == "cmd-arm-1");

    agbot::flight_sim::FlightCommandV1 step_command;
    step_command.command_id = "cmd-step-1";
    step_command.command_type = agbot::flight_sim::TwinCommandType::Step;
    step_command.issued_at_unix_ms = 1'800'000'000'250;
    step_command.step_duration_s = 0.25;

    const auto step_ack = backend.dispatch(step_command);
    assert(step_ack.accepted);
    assert(step_ack.telemetry.has_value());
    assert(step_ack.telemetry->time_s > 0.0);
    assert(step_ack.telemetry->target_waypoint_index == backend.simulation().state().target_waypoint_index);
    assert(step_ack.to_json().find("\"command_id\":\"cmd-step-1\"") != std::string::npos);
    assert(step_ack.to_json().find("\"telemetry\":{") != std::string::npos);
}

void test_twin_backend_unavailable_fails_closed_without_telemetry() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    agbot::flight_sim::TwinBackendConfig config;
    config.available = false;
    agbot::flight_sim::TwinBackend backend(std::move(mission), config);

    agbot::flight_sim::FlightCommandV1 command;
    command.command_id = "cmd-no-backend";
    command.command_type = agbot::flight_sim::TwinCommandType::Arm;
    command.issued_at_unix_ms = 1'800'000'000'000;

    const auto ack = backend.dispatch(command);
    assert(!ack.accepted);
    assert(ack.error.has_value());
    assert(ack.error->code == "twin_unavailable");
    assert(!ack.telemetry.has_value());
    assert(!backend.simulation().state().armed);
}

void test_failsafe_low_battery() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    agbot::flight_sim::SimulationConfig config;
    config.min_battery_percent = 100.0;
    DroneSimulation simulation(std::move(mission), config);
    simulation.step(1.0);
    assert(simulation.state().mode == DroneMode::Failsafe);
    assert(simulation.last_safety_violation().has_value());
    assert(simulation.last_safety_violation()->code == agbot::flight_sim::SafetyViolationCode::LowBatteryAbort);
}

void test_safety_parity_harness_covers_required_rules() {
    const auto cases = agbot::flight_sim::default_safety_parity_cases();
    const auto missing = agbot::flight_sim::missing_required_safety_coverage(cases);
    assert(missing.empty());

    for (const auto& parity_case : cases) {
        const auto violation = agbot::flight_sim::evaluate_safety(parity_case.sample, parity_case.envelope);
        assert(violation.has_value());
        assert(violation->code == parity_case.expected_code);
        assert(!violation->rule_id.empty());
    }
}

void test_drone_simulation_enforces_altitude_safety_rule() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    agbot::flight_sim::SimulationConfig config;
    config.safety.max_altitude_m = 5.0;
    DroneSimulation simulation(std::move(mission), config);

    for (int i = 0; i < 60 * 10 && !simulation.is_complete(); ++i) {
        simulation.step(1.0 / 60.0);
    }

    assert(simulation.state().mode == DroneMode::Failsafe);
    assert(simulation.last_safety_violation().has_value());
    assert(simulation.last_safety_violation()->code == agbot::flight_sim::SafetyViolationCode::AltitudeCeilingViolation);
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
    const TileCoordinate expected_tile = agbot::flight_sim::tile_for_geo(center, 14);
    const auto composite = agbot::flight_sim::composite_elevation_with_state({}, bounds, 4, {expected_tile});
    assert(composite.tile_states.size() == 1);
    assert(composite.tile_states[0].state == agbot::flight_sim::TerrainTileState::FlatFallback);
    assert(composite.tile_states[0].coordinate.x == expected_tile.x);
    assert(std::string(agbot::flight_sim::to_string(composite.tile_states[0].state)) == "flat_fallback");

    const auto& heightmap = composite.heightmap;
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

void test_terrain_profile_asserts_crs_extent_resolution_and_samples_elevation() {
    const GeoCoordinate center {37.7749, -122.4194, 0.0};
    const TileCoordinate tile_coordinate = agbot::flight_sim::tile_for_geo(center, 14);
    const GeoBounds bounds = tile_coordinate.bounds();
    const GeoCoordinate sample_coordinate = bounds.center();

    std::vector<std::uint8_t> pixels(2 * 2 * 4, 0);
    write_terrarium_pixel(pixels, 0, 10.0f);
    write_terrarium_pixel(pixels, 1, 20.0f);
    write_terrarium_pixel(pixels, 2, 30.0f);
    write_terrarium_pixel(pixels, 3, 40.0f);
    const auto tile = agbot::flight_sim::elevation_tile_from_terrarium_rgba(
        tile_coordinate,
        2,
        2,
        pixels);
    assert(tile.has_value());

    const auto composite = agbot::flight_sim::composite_elevation_with_state({*tile}, bounds, 3, {tile_coordinate});
    assert(composite.profile.asserted);
    assert(composite.profile.crs == "EPSG:4326");
    assert(composite.profile.resolution == 3);
    assert(composite.profile.resolution_x_m > 0.0);
    assert(composite.profile.resolution_y_m > 0.0);
    assert(composite.profile.contains(sample_coordinate));

    const auto sample = composite.sample_at(sample_coordinate);
    assert(sample.has_value());
    assert(sample->state == agbot::flight_sim::TerrainTileState::Available);
    assert(std::abs(sample->elevation_m - 25.0f) < 0.01f);

    const auto assertion = composite.assert_elevation_at(sample_coordinate, 25.0f, 0.01f);
    assert(assertion.ok);
    assert(assertion.reason == "terrain_georeference_asserted");
}

void test_missing_terrain_tile_is_sampled_and_manifested_as_flat_fallback() {
    const GeoCoordinate center {37.7749, -122.4194, 0.0};
    const GeoBounds bounds = GeoBounds::from_center(center, 500.0);
    const TileCoordinate expected_tile = agbot::flight_sim::tile_for_geo(center, 14);
    const auto composite = agbot::flight_sim::composite_elevation_with_state({}, bounds, 4, {expected_tile});

    const auto sample = composite.sample_at(center);
    assert(sample.has_value());
    assert(sample->state == agbot::flight_sim::TerrainTileState::FlatFallback);
    assert(sample->elevation_m == 0.0f);

    const std::string manifest = agbot::flight_sim::terrain_tiles_json(composite);
    assert(manifest.find("\"state\":\"flat_fallback\"") != std::string::npos);
    assert(manifest.find("\"crs\":\"EPSG:4326\"") != std::string::npos);
    assert(manifest.find("\"resolution_x_m\":") != std::string::npos);
    assert(manifest.find("\"bounds\":{") != std::string::npos);

    const auto failed_assertion = composite.assert_elevation_at(center, 5.0f, 0.1f);
    assert(!failed_assertion.ok);
    assert(failed_assertion.reason == "elevation_mismatch");
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

void test_lidar_raycast_ranges_match_flat_terrain_cloud_tolerance() {
    const std::vector<float> heightmap(9, 0.0f);
    const auto terrain = agbot::flight_sim::build_terrain_mesh(heightmap, 3, 40.0, 40.0);

    agbot::flight_sim::DroneState state;
    state.position = {0.0, 10.0, 0.0};
    state.mission_time_s = 0.25;

    agbot::flight_sim::LidarRaycastConfig config;
    config.horizontal_samples = 4;
    config.vertical_samples = 2;
    config.vertical_fov_deg = 90.0;
    config.max_range_m = 30.0;
    config.range_noise_m = 0.0;

    const auto scan = agbot::flight_sim::raycast_lidar_scan(state, terrain, config, 42, 3);

    assert(scan.status == "ok");
    assert(scan.points.size() == 5);
    assert(std::abs(scan.points[0].range_m - 10.0) <= 0.05);
    for (std::size_t index = 1; index < scan.points.size(); ++index) {
        assert(std::abs(scan.points[index].range_m - 14.1421) <= 0.05);
        assert(std::abs(scan.points[index].position_m.y) <= 0.05);
    }
}

void test_lidar_raycast_seeded_cloud_json_is_reproducible() {
    const std::vector<float> heightmap(9, 0.0f);
    const auto terrain = agbot::flight_sim::build_terrain_mesh(heightmap, 3, 40.0, 40.0);

    agbot::flight_sim::DroneState state;
    state.position = {0.0, 10.0, 0.0};

    agbot::flight_sim::LidarRaycastConfig config;
    config.horizontal_samples = 4;
    config.vertical_samples = 2;
    config.vertical_fov_deg = 90.0;
    config.max_range_m = 30.0;
    config.range_noise_m = 0.02;

    const auto a = agbot::flight_sim::raycast_lidar_scan(state, terrain, config, 9001, 7);
    const auto b = agbot::flight_sim::raycast_lidar_scan(state, terrain, config, 9001, 7);
    const auto c = agbot::flight_sim::raycast_lidar_scan(state, terrain, config, 9002, 7);

    assert(a.to_json() == b.to_json());
    assert(a.to_json() != c.to_json());
    assert(a.to_json().find("\"scan_id\":\"") != std::string::npos);
    assert(a.to_json().find("\"timestamp\":\"1970-01-01T00:00:00.000Z\"") != std::string::npos);
    assert(a.to_json().find("\"distance\":") != std::string::npos);
    assert(a.to_json().find("\"quality\":") != std::string::npos);
    assert(agbot::flight_sim::lidar_config_json(config).find("\"profile\":\"sim_lidar_a3\"") != std::string::npos);
}

void test_lidar_raycast_empty_scene_returns_empty_capture_scan() {
    agbot::flight_sim::DroneState state;
    state.position = {0.0, 10.0, 0.0};

    agbot::flight_sim::LidarRaycastConfig config;
    const agbot::flight_sim::TerrainMesh empty_terrain;
    const auto scan = agbot::flight_sim::raycast_lidar_scan(state, empty_terrain, config, 42, 0);

    assert(scan.status == "empty_scene");
    assert(scan.points.empty());
    assert(scan.to_json().find("\"points\":[]") != std::string::npos);
}

void test_lidar_flat_fallback_terrain_covers_offset_mission_footprint() {
    agbot::flight_sim::Mission mission;
    mission.home = {100.0, 0.0, -50.0};
    mission.waypoints.push_back({"offset_takeoff", {100.0, 10.0, -50.0}});
    mission.waypoints.push_back({"offset_leg", {140.0, 10.0, -20.0}});

    const auto terrain = agbot::flight_sim::build_lidar_flat_terrain_for_mission(mission, 8, 20.0);

    agbot::flight_sim::DroneState state;
    state.position = {120.0, 10.0, -35.0};
    agbot::flight_sim::LidarRaycastConfig config;
    config.horizontal_samples = 4;
    config.vertical_samples = 2;
    config.max_range_m = 30.0;

    const auto scan = agbot::flight_sim::raycast_lidar_scan(state, terrain, config, 42, 1);
    assert(scan.status == "ok");
    assert(!scan.points.empty());
}

agbot::flight_sim::RunConfig unit_run_config() {
    agbot::flight_sim::RunConfig config;
    config.seed = 42;
    config.timestep_s = 1.0 / 60.0;
    config.record_interval_s = 0.25;
    config.max_time_s = 600.0;
    return config;
}

// Story 02-25: the same mission + seed + timestep produces byte-identical
// output, and the manifest hashes match (TELEM byte-identity).
void test_deterministic_runner_is_byte_identical() {
    const auto mission = MissionLoader::load_from_text(kMissionJson);
    const auto a = agbot::flight_sim::run_deterministic(mission, unit_run_config());
    const auto b = agbot::flight_sim::run_deterministic(mission, unit_run_config());

    assert(a.trace_jsonl == b.trace_jsonl);
    assert(a.manifest.output_hash == b.manifest.output_hash);
    assert(a.manifest.mission_hash == b.manifest.mission_hash);
    assert(a.manifest.to_json() == b.manifest.to_json());
    assert(a.manifest.sample_count > 0);
    assert(a.manifest.completed);
}

// Story 02-25: the seed actually drives the PRNG stream. The autopilot physics
// is currently seed-independent, so the trace is identical while the manifest
// nonce diverges — proving the seed is live for future noise/fault injection.
void test_deterministic_runner_seed_drives_prng() {
    const auto mission = MissionLoader::load_from_text(kMissionJson);
    auto config_a = unit_run_config();
    auto config_b = unit_run_config();
    config_b.seed = 1337;

    const auto a = agbot::flight_sim::run_deterministic(mission, config_a);
    const auto b = agbot::flight_sim::run_deterministic(mission, config_b);

    assert(a.manifest.prng_nonce != b.manifest.prng_nonce);
    assert(a.trace_jsonl == b.trace_jsonl); // physics seed-independent today
}

void test_deterministic_runner_emits_capture_shaped_lidar_jsonl() {
    const auto mission = MissionLoader::load_from_text(kMissionJson);
    const auto a = agbot::flight_sim::run_deterministic(mission, unit_run_config());
    const auto b = agbot::flight_sim::run_deterministic(mission, unit_run_config());

    assert(!a.lidar_scans_jsonl.empty());
    assert(a.lidar_scans_jsonl == b.lidar_scans_jsonl);
    assert(a.manifest.lidar_scan_count == a.manifest.sample_count);
    assert(a.manifest.lidar_output_hash == agbot::flight_sim::sha256_hex(a.lidar_scans_jsonl));
    assert(a.manifest.lidar_config_hash == agbot::flight_sim::sha256_hex(a.manifest.lidar_config_json));
    assert(a.lidar_scans_jsonl.find("\"scan_id\":\"") != std::string::npos);
    assert(a.lidar_scans_jsonl.find("\"timestamp\":\"") != std::string::npos);
    assert(a.lidar_scans_jsonl.find("\"points\":[") != std::string::npos);
    assert(a.lidar_scans_jsonl.find("\"angle\":") != std::string::npos);
    assert(a.lidar_scans_jsonl.find("\"distance\":") != std::string::npos);
    assert(a.lidar_scans_jsonl.find("\"quality\":") != std::string::npos);
}

// Story 02-28: the manifest records the contract version and deterministic hashes.
void test_run_manifest_records_contract_and_hashes() {
    const auto mission = MissionLoader::load_from_text(kMissionJson);
    const auto result = agbot::flight_sim::run_deterministic(mission, unit_run_config());
    const std::string json = result.manifest.to_json();
    const auto& contract = agbot::flight_sim::twin_contract_v1_schema();

    assert(json.find("\"contract_version\":\"1.0.0\"") != std::string::npos);
    assert(json.find("\"contract_schema_hash\":\"") != std::string::npos);
    assert(json.find("\"run_id\":\"") != std::string::npos);
    assert(result.manifest.contract_schema_hash == contract.schema_hash);
    assert(json.find("\"seed\":42") != std::string::npos);
    assert(json.find("\"trace_retention_keep\":0") != std::string::npos);
    assert(json.find("\"trace_retention_deleted\":[]") != std::string::npos);
    assert(json.find("\"faults\":[]") != std::string::npos);
    assert(json.find("\"fault_events\":[]") != std::string::npos);
    assert(result.manifest.run_id.size() == 64);
    assert(json.find("\"terrain_tiles\":[]") != std::string::npos);
    assert(json.find("\"weather_config_hash\":\"") != std::string::npos);
    assert(json.find("\"sensor_config_hash\":\"") != std::string::npos);
    assert(json.find("\"lidar_config_hash\":\"") != std::string::npos);
    assert(json.find("\"lidar_scan_count\":") != std::string::npos);
    assert(json.find("\"lidar_output_hash\":\"") != std::string::npos);
    assert(json.find("\"safety_config_hash\":\"") != std::string::npos);
    assert(result.manifest.output_hash.size() == 64);
    assert(result.manifest.mission_hash.size() == 64);
    assert(result.manifest.terrain_tiles_hash == agbot::flight_sim::sha256_hex("[]"));
    assert(result.manifest.output_hash == agbot::flight_sim::sha256_hex(result.trace_jsonl));
    assert(result.manifest.lidar_output_hash == agbot::flight_sim::sha256_hex(result.lidar_scans_jsonl));
    assert(result.manifest.mission_hash == agbot::flight_sim::sha256_hex(agbot::flight_sim::mission_to_json(mission)));
    assert(result.manifest.output_hash != result.manifest.mission_hash);
}

void test_run_manifest_records_geodetic_terrain_fallback_evidence() {
    const auto mission = MissionLoader::load_from_text(kGeoMissionJson);
    const auto result = agbot::flight_sim::run_deterministic(mission, unit_run_config());
    const std::string json = result.manifest.to_json();

    assert(result.manifest.terrain_tiles_json.find("\"state\":\"flat_fallback\"") != std::string::npos);
    assert(result.manifest.terrain_tiles_json.find("\"crs\":\"EPSG:4326\"") != std::string::npos);
    assert(result.manifest.terrain_tiles_json.find("\"resolution\":96") != std::string::npos);
    assert(result.manifest.terrain_tiles_json.find("\"bounds\":{") != std::string::npos);
    assert(result.manifest.terrain_tiles_hash == agbot::flight_sim::sha256_hex(result.manifest.terrain_tiles_json));
    assert(json.find("\"terrain_tiles\":[{") != std::string::npos);
}

void test_zero_wind_keeps_deterministic_trace_identical() {
    const auto mission = MissionLoader::load_from_text(kMissionJson);
    auto zero_wind = unit_run_config();
    zero_wind.steady_wind_mps = {0.0, 0.0, 0.0};

    const auto baseline = agbot::flight_sim::run_deterministic(mission, unit_run_config());
    const auto zero = agbot::flight_sim::run_deterministic(mission, zero_wind);

    assert(zero.trace_jsonl == baseline.trace_jsonl);
    assert(zero.manifest.output_hash == baseline.manifest.output_hash);
    assert(zero.manifest.weather_config_hash == baseline.manifest.weather_config_hash);
}

void test_steady_wind_is_reproducible_and_manifested() {
    const auto mission = MissionLoader::load_from_text(kMissionJson);
    auto windy_config = unit_run_config();
    windy_config.steady_wind_mps = {3.0, 0.0, 0.0};

    const auto baseline = agbot::flight_sim::run_deterministic(mission, unit_run_config());
    const auto a = agbot::flight_sim::run_deterministic(mission, windy_config);
    const auto b = agbot::flight_sim::run_deterministic(mission, windy_config);

    assert(a.trace_jsonl == b.trace_jsonl);
    assert(a.manifest.to_json() == b.manifest.to_json());
    assert(a.trace_jsonl != baseline.trace_jsonl);
    assert(a.manifest.weather_config_json.find("\"wind_mps\":{\"x\":3.000") != std::string::npos);
    assert(a.manifest.weather_config_json.find("\"source\":\"steady_wind\"") != std::string::npos);
    assert(a.manifest.weather_config_hash == agbot::flight_sim::sha256_hex(a.manifest.weather_config_json));

    const auto diff = agbot::flight_sim::diff_trace_text(baseline.trace_jsonl, a.trace_jsonl);
    assert(!diff.identical);
    assert(diff.field_path == "position.x");
}

void test_zero_noise_sensor_profile_is_exact() {
    agbot::flight_sim::DroneState state;
    state.position = {12.0, 34.0, -5.0};
    state.velocity = {1.0, 2.0, 3.0};
    state.yaw_rad = 0.25;
    state.pitch_rad = -0.10;
    state.roll_rad = 0.05;

    const auto profile = agbot::flight_sim::ideal_sensor_profile();
    const auto reading = agbot::flight_sim::calibrated_sensor_reading(state, profile, 42, 7);

    assert(reading.profile_name == "ideal");
    assert(reading.gps_position_m.x == state.position.x);
    assert(reading.gps_position_m.y == state.position.y);
    assert(reading.gps_position_m.z == state.position.z);
    assert(reading.imu_yaw_rad == state.yaw_rad);
    assert(reading.imu_pitch_rad == state.pitch_rad);
    assert(reading.imu_roll_rad == state.roll_rad);
    assert(reading.barometer_altitude_m == state.position.y);
    assert(reading.magnetometer_heading_rad == state.yaw_rad);
    assert(reading.to_json().find("\"profile\":\"ideal\"") != std::string::npos);
}

void test_seeded_sensor_noise_is_reproducible_and_inspectable() {
    agbot::flight_sim::DroneState state;
    state.position = {12.0, 34.0, -5.0};
    state.velocity = {1.0, 2.0, 3.0};
    state.yaw_rad = 0.25;
    state.pitch_rad = -0.10;
    state.roll_rad = 0.05;

    auto profile = agbot::flight_sim::sensor_profile_by_name("cheap_gps");
    profile.imu_attitude_noise_rad = 0.02;
    profile.barometer_altitude_noise_m = 0.4;
    profile.magnetometer_heading_noise_rad = 0.03;

    const auto a = agbot::flight_sim::calibrated_sensor_reading(state, profile, 9001, 17);
    const auto b = agbot::flight_sim::calibrated_sensor_reading(state, profile, 9001, 17);
    const auto c = agbot::flight_sim::calibrated_sensor_reading(state, profile, 9002, 17);

    assert(a.to_json() == b.to_json());
    assert(a.to_json() != c.to_json());
    assert(std::abs(a.gps_position_m.x - state.position.x) <= profile.gps_position_noise_m);
    assert(std::abs(a.imu_yaw_rad - state.yaw_rad) <= profile.imu_attitude_noise_rad);
    assert(std::abs(a.barometer_altitude_m - state.position.y) <= profile.barometer_altitude_noise_m);
    assert(a.to_json().find("\"distribution\":\"deterministic_uniform\"") != std::string::npos);

    const std::string config = agbot::flight_sim::sensor_config_json(profile);
    assert(config.find("\"profile\":\"cheap_gps\"") != std::string::npos);
    assert(config.find("\"gps_position_noise_m\":1.500") != std::string::npos);
}

// Story 02-24: the first TwinContractV1 slice names the shared wire schemas
// and their required fields so downstream consumers can detect schema drift.
void test_twin_contract_v1_schema_covers_required_types() {
    const auto& schema = agbot::flight_sim::twin_contract_v1_schema();

    assert(schema.version == agbot::flight_sim::kTwinContractVersion);
    assert(schema.schema_hash.size() == 64);
    assert(schema.has_type("FlightCommandV1"));
    assert(schema.has_type("TelemetryV1"));
    assert(schema.has_type("SimulationTraceV1"));
    assert(schema.has_type("ScenarioManifestV1"));
    assert(schema.has_type("TwinErrorV1"));
    assert(schema.has_type("TwinCommandAckV1"));
    assert(schema.has_type("TwinCapabilitiesV1"));
    assert(schema.type_has_field("TelemetryV1", "battery_percent"));
    assert(schema.type_has_field("TwinCommandAckV1", "telemetry"));
    assert(schema.type_has_field("SimulationTraceV1", "contract_version"));
    assert(schema.type_has_field("ScenarioManifestV1", "contract_schema_hash"));
    assert(schema.type_has_field("ScenarioManifestV1", "run_id"));
    assert(schema.type_has_field("ScenarioManifestV1", "terrain_tiles_hash"));
    assert(schema.type_has_field("ScenarioManifestV1", "safety_config_hash"));
    assert(schema.type_has_field("ScenarioManifestV1", "trace_retention_deleted"));
    assert(schema.type_has_field("ScenarioManifestV1", "faults"));
    assert(schema.type_has_field("ScenarioManifestV1", "fault_events"));
    assert(schema.has_capability("deterministic_runner"));
    assert(schema.has_capability("scenario_manifest"));
    assert(schema.has_capability("simulation_health"));
    assert(schema.has_capability("fault_injection"));
    assert(schema.has_capability("terrain_crs_extent_assertions"));
    assert(schema.has_capability("wind_field"));
    assert(schema.has_capability("sensor_noise_calibration"));
    assert(schema.has_capability("lidar_raycast"));
    assert(schema.has_capability("twin_backend_api"));
    assert(schema.to_json().find("\"schema_hash\":\"" + schema.schema_hash + "\"") != std::string::npos);
}

void test_twin_contract_version_compatibility() {
    assert(agbot::flight_sim::is_compatible_contract_version("1.0.0", "1.0.1"));
    assert(agbot::flight_sim::is_compatible_contract_version("1.0.0", "1.1.0"));
    assert(!agbot::flight_sim::is_compatible_contract_version("1.1.0", "1.0.0"));
    assert(!agbot::flight_sim::is_compatible_contract_version("1.0.0", "2.0.0"));
    assert(!agbot::flight_sim::is_compatible_contract_version("1.0.0", "not-semver"));
}

void test_trace_diff_reports_divergent_field() {
    const auto mission = MissionLoader::load_from_text(kMissionJson);
    const auto result = agbot::flight_sim::run_deterministic(mission, unit_run_config());
    std::string altered = result.trace_jsonl;
    const std::size_t position = altered.find("\"x\":0.000");
    assert(position != std::string::npos);
    altered.replace(position, std::string("\"x\":0.000").size(), "\"x\":1.000");

    const auto diff = agbot::flight_sim::diff_trace_text(result.trace_jsonl, altered);
    assert(!diff.identical);
    assert(diff.step_index == 0);
    assert(diff.field_path == "position.x");
    assert(diff.left_value == "0.000");
    assert(diff.right_value == "1.000");

    const auto identical = agbot::flight_sim::diff_trace_text(result.trace_jsonl, result.trace_jsonl);
    assert(identical.identical);
}

// Stories 02-01 / 02-02: golden-telemetry regression. The committed golden
// trace pins physics + flight-controller behavior; any change to the step loop
// or telemetry shape fails this test with a byte mismatch.
void test_deterministic_runner_matches_golden() {
    const std::filesystem::path golden_path =
        std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "tests" / "golden" / "unit_mission.jsonl";

    std::ifstream golden_stream(golden_path, std::ios::binary);
    assert(golden_stream && "golden fixture tests/golden/unit_mission.jsonl is missing");
    std::ostringstream buffer;
    buffer << golden_stream.rdbuf();
    const std::string golden = buffer.str();

    const auto mission = MissionLoader::load_from_text(kMissionJson);
    const auto result = agbot::flight_sim::run_deterministic(mission, unit_run_config());

    assert(result.trace_jsonl == golden);
}

void test_fnv1a64_is_stable_and_distinct() {
    assert(agbot::flight_sim::fnv1a64("") == 14695981039346656037ULL);
    assert(agbot::flight_sim::fnv1a64("agbot") == agbot::flight_sim::fnv1a64("agbot"));
    assert(agbot::flight_sim::fnv1a64("agbot") != agbot::flight_sim::fnv1a64("agboT"));
    assert(agbot::flight_sim::to_hex(255) == "00000000000000ff");
}

void test_simulation_health_reports_pass_and_seed_failure() {
    const auto root = std::filesystem::temp_directory_path() / "agbot_flight_sim_health_test";
    std::filesystem::remove_all(root);
    const auto cache_dir = root / "map_tiles";
    const auto trace_dir = root / "runs";
    const auto manifest_path = root / "telemetry.manifest.json";
    std::filesystem::create_directories(cache_dir);
    std::filesystem::create_directories(trace_dir);
    {
        std::ofstream manifest(manifest_path);
        manifest << "{\"completed\":true}\n";
    }

    agbot::flight_sim::HealthCheckConfig config;
    config.seed = 42;
    config.terrain_cache_dir = cache_dir;
    config.trace_dir = trace_dir;
    config.last_manifest_path = manifest_path;
    config.trace_retention_keep = 3;

    const auto healthy = agbot::flight_sim::evaluate_simulation_health(config);
    const std::string healthy_json = healthy.to_json();
    assert(healthy.ok());
    assert(healthy_json.find("\"runner_mode\":{\"status\":\"pass\"") != std::string::npos);
    assert(healthy_json.find("\"prng_seeded\":{\"status\":\"pass\"") != std::string::npos);
    assert(healthy_json.find("\"terrain_cache_state\":{\"status\":\"pass\"") != std::string::npos);
    assert(healthy_json.find("\"last_run_manifest_present\":{\"status\":\"pass\"") != std::string::npos);
    assert(healthy_json.find("\"trace_retention_compliant\":{\"status\":\"pass\"") != std::string::npos);

    config.seed = std::nullopt;
    const auto unseeded = agbot::flight_sim::evaluate_simulation_health(config);
    const std::string unseeded_json = unseeded.to_json();
    assert(!unseeded.ok());
    assert(unseeded_json.find("\"prng_seeded\":{\"status\":\"fail\"") != std::string::npos);

    std::filesystem::remove_all(root);
}

void test_trace_retention_deletes_oldest_trace_and_records_evidence() {
    const auto root = std::filesystem::temp_directory_path() / "agbot_flight_sim_retention_test";
    std::filesystem::remove_all(root);
    std::filesystem::create_directories(root);
    const auto old_trace = root / "run_001.jsonl";
    const auto mid_trace = root / "run_002.jsonl";
    const auto new_trace = root / "run_003.jsonl";
    {
        std::ofstream(old_trace) << "{}\n";
        std::ofstream(mid_trace) << "{}\n";
        std::ofstream(new_trace) << "{}\n";
    }

    const auto now = std::filesystem::file_time_type::clock::now();
    std::filesystem::last_write_time(old_trace, now - std::chrono::seconds(30));
    std::filesystem::last_write_time(mid_trace, now - std::chrono::seconds(20));
    std::filesystem::last_write_time(new_trace, now - std::chrono::seconds(10));

    const auto result = agbot::flight_sim::enforce_trace_retention(root, 2);
    assert(result.keep_count == 2);
    assert(result.deleted_paths.size() == 1);
    assert(result.deleted_json().find("run_001.jsonl") != std::string::npos);
    assert(!std::filesystem::exists(old_trace));
    assert(std::filesystem::exists(mid_trace));
    assert(std::filesystem::exists(new_trace));

    std::filesystem::remove_all(root);
}

void test_tile_cache_clear_removes_entries_but_keeps_directory() {
    const auto root = std::filesystem::temp_directory_path() / "agbot_flight_sim_cache_test";
    std::filesystem::remove_all(root);
    const auto nested = root / "12" / "655" / "1583.tile";
    std::filesystem::create_directories(nested.parent_path());
    {
        std::ofstream(nested) << "tile";
    }

    const std::uintmax_t removed = agbot::flight_sim::clear_tile_cache(root);
    assert(removed >= 1);
    assert(std::filesystem::exists(root));
    assert(std::filesystem::is_empty(root));

    std::filesystem::remove_all(root);
}

void test_fault_injection_gps_drift_is_seeded_and_reproducible() {
    const auto mission = MissionLoader::load_from_text(kMissionJson);
    auto faulted_config = unit_run_config();
    faulted_config.faults.faults.push_back({
        agbot::flight_sim::FaultClass::GpsDrift,
        9001,
        0,
        std::nullopt,
        2.0,
        "gps",
    });

    const auto baseline = agbot::flight_sim::run_deterministic(mission, unit_run_config());
    const auto a = agbot::flight_sim::run_deterministic(mission, faulted_config);
    const auto b = agbot::flight_sim::run_deterministic(mission, faulted_config);

    assert(a.trace_jsonl == b.trace_jsonl);
    assert(a.manifest.to_json() == b.manifest.to_json());
    assert(a.trace_jsonl != baseline.trace_jsonl);
    assert(a.manifest.faults_json.find("\"class\":\"gps_drift\"") != std::string::npos);
    assert(a.manifest.fault_events_json.find("\"class\":\"gps_drift\"") != std::string::npos);

    const auto diff = agbot::flight_sim::diff_trace_text(baseline.trace_jsonl, a.trace_jsonl);
    assert(!diff.identical);
    assert(diff.field_path == "position.x" || diff.field_path == "position.y" || diff.field_path == "position.z");
}

void test_fault_injection_sensor_dropout_prunes_samples_and_records_event() {
    const auto mission = MissionLoader::load_from_text(kMissionJson);
    auto faulted_config = unit_run_config();
    faulted_config.faults.faults.push_back({
        agbot::flight_sim::FaultClass::SensorDropout,
        1234,
        100,
        220,
        0.0,
        "telemetry",
    });

    const auto baseline = agbot::flight_sim::run_deterministic(mission, unit_run_config());
    const auto faulted = agbot::flight_sim::run_deterministic(mission, faulted_config);

    assert(faulted.trace_jsonl != baseline.trace_jsonl);
    assert(faulted.manifest.sample_count < baseline.manifest.sample_count);
    assert(faulted.manifest.faults_json.find("\"class\":\"sensor_dropout\"") != std::string::npos);
    assert(faulted.manifest.fault_events_json.find("\"class\":\"sensor_dropout\"") != std::string::npos);
}

void test_fault_injection_bad_tile_marks_flat_fallback_in_manifest() {
    const auto mission = MissionLoader::load_from_text(kMissionJson);
    auto faulted_config = unit_run_config();
    faulted_config.faults.faults.push_back({
        agbot::flight_sim::FaultClass::BadTile,
        777,
        0,
        std::nullopt,
        0.0,
        "terrain/tile/z12/x655/y1583",
    });

    const auto faulted = agbot::flight_sim::run_deterministic(mission, faulted_config);
    const std::string manifest = faulted.manifest.to_json();

    assert(faulted.manifest.terrain_tiles_json.find("\"state\":\"flat_fallback\"") != std::string::npos);
    assert(manifest.find("\"fault_seed\":777") != std::string::npos);
    assert(manifest.find("\"class\":\"bad_tile\"") != std::string::npos);
}

void test_fault_injection_rejects_fault_without_seed() {
    agbot::flight_sim::FaultInjectionPlan plan;
    plan.faults.push_back({
        agbot::flight_sim::FaultClass::GpsDrift,
        std::nullopt,
        0,
        std::nullopt,
        1.0,
        "gps",
    });

    bool rejected = false;
    try {
        agbot::flight_sim::validate_fault_plan(plan);
    } catch (const std::invalid_argument&) {
        rejected = true;
    }
    assert(rejected);
}

} // namespace

int main() {
    test_loads_mission();
    test_loads_geodetic_mission();
    test_mission_preview_overlay_aligns_field_boundary_and_coverage();
    test_mission_preview_overlay_reports_missing_boundary();
    test_mission_completes();
    test_simulation_events_follow_documented_normal_order();
    test_simulation_emergency_event_suppresses_normal_events();
    test_manual_controls_move_drone();
    test_steady_wind_disturbs_ground_track();
    test_mission_round_trip();
    test_twin_backend_executes_shared_command_and_returns_telemetry();
    test_twin_backend_unavailable_fails_closed_without_telemetry();
    test_failsafe_low_battery();
    test_safety_parity_harness_covers_required_rules();
    test_drone_simulation_enforces_altitude_safety_rule();
    test_telemetry_recorder_close_is_idempotent();
    test_geo_terrain_area_and_tiles();
    test_decodes_terrarium_elevation_without_png_dependency();
    test_composites_empty_elevation_as_flat_zero();
    test_terrain_profile_asserts_crs_extent_resolution_and_samples_elevation();
    test_missing_terrain_tile_is_sampled_and_manifested_as_flat_fallback();
    test_builds_terrain_mesh_from_heightmap();
    test_lidar_raycast_ranges_match_flat_terrain_cloud_tolerance();
    test_lidar_raycast_seeded_cloud_json_is_reproducible();
    test_lidar_raycast_empty_scene_returns_empty_capture_scan();
    test_lidar_flat_fallback_terrain_covers_offset_mission_footprint();
    test_deterministic_runner_is_byte_identical();
    test_deterministic_runner_seed_drives_prng();
    test_deterministic_runner_emits_capture_shaped_lidar_jsonl();
    test_run_manifest_records_contract_and_hashes();
    test_run_manifest_records_geodetic_terrain_fallback_evidence();
    test_zero_wind_keeps_deterministic_trace_identical();
    test_steady_wind_is_reproducible_and_manifested();
    test_zero_noise_sensor_profile_is_exact();
    test_seeded_sensor_noise_is_reproducible_and_inspectable();
    test_twin_contract_v1_schema_covers_required_types();
    test_twin_contract_version_compatibility();
    test_trace_diff_reports_divergent_field();
    test_deterministic_runner_matches_golden();
    test_fnv1a64_is_stable_and_distinct();
    test_simulation_health_reports_pass_and_seed_failure();
    test_trace_retention_deletes_oldest_trace_and_records_evidence();
    test_tile_cache_clear_removes_entries_but_keeps_directory();
    test_fault_injection_gps_drift_is_seeded_and_reproducible();
    test_fault_injection_sensor_dropout_prunes_samples_and_records_event();
    test_fault_injection_bad_tile_marks_flat_fallback_in_manifest();
    test_fault_injection_rejects_fault_without_seed();
    std::cout << "agbot_flight_sim_tests passed\n";
    return 0;
}
