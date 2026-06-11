#include "agbot_flight_sim/DeterministicRunner.hpp"
#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_flight_sim/MissionLoader.hpp"
#include "agbot_flight_sim/SafetyRules.hpp"
#include "agbot_flight_sim/SimulationOps.hpp"
#include "agbot_flight_sim/TelemetryRecorder.hpp"
#include "agbot_flight_sim/TelemetryReplay.hpp"
#include "agbot_flight_sim/TraceDiff.hpp"
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
    assert(result.manifest.run_id.size() == 64);
    assert(json.find("\"terrain_tiles\":[]") != std::string::npos);
    assert(json.find("\"weather_config_hash\":\"") != std::string::npos);
    assert(json.find("\"sensor_config_hash\":\"") != std::string::npos);
    assert(json.find("\"safety_config_hash\":\"") != std::string::npos);
    assert(result.manifest.output_hash.size() == 64);
    assert(result.manifest.mission_hash.size() == 64);
    assert(result.manifest.terrain_tiles_hash == agbot::flight_sim::sha256_hex("[]"));
    assert(result.manifest.output_hash == agbot::flight_sim::sha256_hex(result.trace_jsonl));
    assert(result.manifest.mission_hash == agbot::flight_sim::sha256_hex(agbot::flight_sim::mission_to_json(mission)));
    assert(result.manifest.output_hash != result.manifest.mission_hash);
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
    assert(schema.has_type("TwinCapabilitiesV1"));
    assert(schema.type_has_field("TelemetryV1", "battery_percent"));
    assert(schema.type_has_field("SimulationTraceV1", "contract_version"));
    assert(schema.type_has_field("ScenarioManifestV1", "contract_schema_hash"));
    assert(schema.type_has_field("ScenarioManifestV1", "run_id"));
    assert(schema.type_has_field("ScenarioManifestV1", "terrain_tiles_hash"));
    assert(schema.type_has_field("ScenarioManifestV1", "safety_config_hash"));
    assert(schema.type_has_field("ScenarioManifestV1", "trace_retention_deleted"));
    assert(schema.has_capability("deterministic_runner"));
    assert(schema.has_capability("scenario_manifest"));
    assert(schema.has_capability("simulation_health"));
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

} // namespace

int main() {
    test_loads_mission();
    test_loads_geodetic_mission();
    test_mission_completes();
    test_manual_controls_move_drone();
    test_mission_round_trip();
    test_failsafe_low_battery();
    test_safety_parity_harness_covers_required_rules();
    test_drone_simulation_enforces_altitude_safety_rule();
    test_telemetry_recorder_close_is_idempotent();
    test_geo_terrain_area_and_tiles();
    test_decodes_terrarium_elevation_without_png_dependency();
    test_composites_empty_elevation_as_flat_zero();
    test_builds_terrain_mesh_from_heightmap();
    test_deterministic_runner_is_byte_identical();
    test_deterministic_runner_seed_drives_prng();
    test_run_manifest_records_contract_and_hashes();
    test_twin_contract_v1_schema_covers_required_types();
    test_twin_contract_version_compatibility();
    test_trace_diff_reports_divergent_field();
    test_deterministic_runner_matches_golden();
    test_fnv1a64_is_stable_and_distinct();
    test_simulation_health_reports_pass_and_seed_failure();
    test_trace_retention_deletes_oldest_trace_and_records_evidence();
    test_tile_cache_clear_removes_entries_but_keeps_directory();
    std::cout << "agbot_flight_sim_tests passed\n";
    return 0;
}
