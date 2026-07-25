#include "agbot_flight_sim/DeterministicRunner.hpp"

#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_flight_sim/LidarSimulator.hpp"
#include "agbot_flight_sim/TelemetryRecorder.hpp"
#include "agbot_flight_sim/TwinContractV1.hpp"

#include <cmath>
#include <cstdio>
#include <iomanip>
#include <random>
#include <sstream>
#include <vector>

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

std::string weather_config_json(Vec3 steady_wind_mps) {
    std::ostringstream stream;
    stream << std::fixed << std::setprecision(3)
           << "{\"wind_mps\":{\"x\":" << steady_wind_mps.x
           << ",\"y\":" << steady_wind_mps.y
           << ",\"z\":" << steady_wind_mps.z << "}";
    if (steady_wind_mps.length() > 1e-9) {
        stream << ",\"source\":\"steady_wind\"";
    }
    stream << "}";
    return stream.str();
}

std::string safety_config_json(const SafetyEnvelope& safety) {
    std::ostringstream stream;
    stream << std::fixed << std::setprecision(3)
           << "{\"min_battery_percent\":" << safety.min_battery_percent;
    if (std::isfinite(safety.max_altitude_m)) {
        stream << ",\"max_altitude_m\":" << safety.max_altitude_m;
    } else {
        stream << ",\"max_altitude_m\":\"unbounded\"";
    }
    const bool unbounded_geofence = !std::isfinite(safety.min_x_m)
        && !std::isfinite(safety.max_x_m)
        && !std::isfinite(safety.min_z_m)
        && !std::isfinite(safety.max_z_m);
    if (unbounded_geofence) {
        stream << ",\"geofence\":\"unbounded\"";
    } else {
        const auto append_bound = [&](std::string_view name, double value, bool lower) {
            stream << "\"" << name << "\":";
            if (std::isfinite(value)) {
                stream << value;
            } else {
                stream << "\"" << (lower ? "-unbounded" : "unbounded") << "\"";
            }
        };
        stream << ",\"geofence\":{";
        append_bound("min_x_m", safety.min_x_m, true);
        stream << ',';
        append_bound("max_x_m", safety.max_x_m, false);
        stream << ',';
        append_bound("min_z_m", safety.min_z_m, true);
        stream << ',';
        append_bound("max_z_m", safety.max_z_m, false);
        stream << '}';
    }
    stream << ",\"no_fly_zone_count\":" << safety.no_fly_zones.size();
    if (!safety.no_fly_zones.empty()) {
        stream << ",\"no_fly_zones\":[";
        for (std::size_t index = 0; index < safety.no_fly_zones.size(); ++index) {
            if (index > 0) {
                stream << ',';
            }
            const auto& zone = safety.no_fly_zones[index];
            stream << "{\"id\":\"" << escape_json(zone.id) << "\""
                   << ",\"x\":" << zone.center.x
                   << ",\"z\":" << zone.center.z
                   << ",\"radius_m\":" << zone.radius_m << "}";
        }
        stream << ']';
    }
    stream << "}";
    return stream.str();
}

std::string merge_json_arrays(const std::string& left, const std::string& right) {
    if (left == "[]") {
        return right;
    }
    if (right == "[]") {
        return left;
    }
    if (left.size() < 2 || right.size() < 2) {
        return left;
    }
    return "[" + left.substr(1, left.size() - 2) + "," + right.substr(1, right.size() - 2) + "]";
}

} // namespace

std::uint64_t fnv1a64(std::string_view bytes) {
    constexpr std::uint64_t kOffsetBasis = 14695981039346656037ULL;
    constexpr std::uint64_t kPrime = 1099511628211ULL;
    std::uint64_t hash = kOffsetBasis;
    for (const char byte : bytes) {
        hash ^= static_cast<std::uint64_t>(static_cast<unsigned char>(byte));
        hash *= kPrime;
    }
    return hash;
}

std::string to_hex(std::uint64_t value) {
    char buffer[17];
    std::snprintf(buffer, sizeof(buffer), "%016llx", static_cast<unsigned long long>(value));
    return std::string(buffer);
}

std::string RunManifest::to_json() const {
    std::ostringstream stream;
    stream << "{"
           << "\"simulator_version\":\"" << escape_json(simulator_version) << "\""
           << ",\"contract_version\":\"" << escape_json(contract_version) << "\""
           << ",\"contract_schema_hash\":\"" << escape_json(contract_schema_hash) << "\""
           << ",\"run_id\":\"" << escape_json(run_id) << "\""
           << ",\"seed\":" << seed
           << ",\"timestep_s\":" << timestep_s
           << ",\"record_interval_s\":" << record_interval_s
           << ",\"mission_name\":\"" << escape_json(mission_name) << "\""
           << ",\"mission_hash\":\"" << escape_json(mission_hash) << "\"";
    if (!plant_model.empty()) {
        stream << ",\"plant_model\":\"" << escape_json(plant_model) << "\"";
    }
    stream << ",\"step_count\":" << step_count
           << ",\"sample_count\":" << sample_count
           << ",\"prng_nonce\":" << prng_nonce
           << ",\"terrain_tiles\":" << terrain_tiles_json
           << ",\"terrain_tiles_hash\":\"" << escape_json(terrain_tiles_hash) << "\""
           << ",\"weather_config\":" << weather_config_json
           << ",\"weather_config_hash\":\"" << escape_json(weather_config_hash) << "\""
           << ",\"sensor_config\":" << sensor_config_json
           << ",\"sensor_config_hash\":\"" << escape_json(sensor_config_hash) << "\""
           << ",\"lidar_config\":" << lidar_config_json
           << ",\"lidar_config_hash\":\"" << escape_json(lidar_config_hash) << "\""
           << ",\"lidar_scan_count\":" << lidar_scan_count
           << ",\"lidar_output_hash\":\"" << escape_json(lidar_output_hash) << "\""
           << ",\"safety_config\":" << safety_config_json
           << ",\"safety_config_hash\":\"" << escape_json(safety_config_hash) << "\"";
    if (!validation_report_json.empty()) {
        stream << ",\"validation_report\":" << validation_report_json
               << ",\"validation_report_hash\":\"" << escape_json(validation_report_hash) << "\"";
    }
    stream << ",\"trace_retention_keep\":" << trace_retention_keep
           << ",\"trace_retention_deleted\":" << trace_retention_deleted_json
           << ",\"faults\":" << faults_json
           << ",\"faults_hash\":\"" << escape_json(faults_hash) << "\""
           << ",\"fault_events\":" << fault_events_json
           << ",\"fault_events_hash\":\"" << escape_json(fault_events_hash) << "\""
           << ",\"output_hash\":\"" << escape_json(output_hash) << "\"";
    if (!termination_reason.empty()) {
        stream << ",\"termination_reason\":\"" << escape_json(termination_reason) << "\"";
    }
    if (!safety_violation.empty()) {
        stream << ",\"safety_violation\":\"" << escape_json(safety_violation) << "\"";
    }
    stream << ",\"completed\":" << (completed ? "true" : "false") << "}";
    return stream.str();
}

RunResult run_deterministic(const Mission& mission, const RunConfig& config) {
    validate_fault_plan(config.faults);

    // Seeded PRNG. Currently the autopilot physics is deterministic and
    // seed-independent; the stream is plumbed here so that future sensor noise
    // and fault injection (stories 02-08/02-30) are reproducible by seed. The
    // first draw is recorded in the manifest so the seed is provably driving a
    // stream even before noise is wired in.
    std::mt19937_64 prng(config.seed);
    const std::uint64_t prng_nonce = prng();

    SimulationConfig simulation_config;
    simulation_config.plant_model = config.plant_model;
    simulation_config.safety = config.safety;
    simulation_config.min_battery_percent = config.safety.min_battery_percent;
    if (config.terrain.has_value()) {
        simulation_config.terrain = config.terrain->mesh;
    }
    DroneSimulation simulation(mission, simulation_config);

    std::ostringstream trace;
    std::ostringstream lidar_trace;
    std::uint64_t step_count = 0;
    std::uint64_t sample_count = 0;
    std::uint64_t lidar_scan_count = 0;
    double next_record_s = 0.0;
    std::vector<FaultEvent> fault_events;
    const TerrainMesh lidar_terrain = config.terrain.has_value()
        ? config.terrain->mesh
        : build_lidar_flat_terrain_for_mission(mission);

    const auto record = [&](const DroneState& state, std::uint64_t step) {
        if (sensor_stream_suppressed(config.faults, step)) {
            return;
        }
        const DroneState observed = apply_observation_faults(state, config.faults, step);
        trace << format_telemetry_sample(observed) << "\n";
        ++sample_count;
        if (config.lidar.enabled) {
            DroneState lidar_state = observed;
            if (config.terrain.has_value()) {
                lidar_state.position.y =
                    simulation.world_elevation_m(observed.position);
            }
            const LidarScan scan = raycast_lidar_scan(
                lidar_state, lidar_terrain, config.lidar, config.seed, step);
            lidar_trace << scan.to_json() << "\n";
            ++lidar_scan_count;
        }
    };

    while (!simulation.is_complete() && simulation.state().mission_time_s < config.max_time_s) {
        append_fault_events_for_step(config.faults, step_count, fault_events);
        simulation.inject_battery_drop(battery_drop_for_step(config.faults, step_count));
        simulation.set_actuator_response_factor(actuator_response_for_step(config.faults, step_count));
        simulation.set_wind(config.steady_wind_mps + wind_fault_for_step(config.faults, step_count));
        simulation.set_guidance_state(apply_observation_faults(simulation.state(), config.faults, step_count));
        if (simulation.state().mission_time_s >= next_record_s) {
            record(simulation.state(), step_count);
            next_record_s += config.record_interval_s;
        }
        simulation.step(config.timestep_s);
        ++step_count;
    }
    // Always record the terminal state so the trace ends at the true outcome.
    append_fault_events_for_step(config.faults, step_count, fault_events);
    record(simulation.state(), step_count);

    RunResult result;
    result.trace_jsonl = trace.str();
    result.lidar_scans_jsonl = lidar_trace.str();

    RunManifest& manifest = result.manifest;
    manifest.simulator_version = kSimulatorVersion;
    manifest.contract_version = kTwinContractVersion;
    manifest.contract_schema_hash = twin_contract_v1_schema().schema_hash;
    manifest.seed = config.seed;
    manifest.timestep_s = config.timestep_s;
    manifest.record_interval_s = config.record_interval_s;
    manifest.mission_name = mission.name;
    manifest.mission_hash = sha256_hex(mission_to_json(mission));
    if (config.plant_model != PlantModel::Simple) {
        manifest.plant_model = to_string(config.plant_model);
    }
    manifest.faults_json = config.faults.to_json();
    manifest.faults_hash = sha256_hex(manifest.faults_json);
    const std::string terrain_evidence = config.terrain.has_value()
        ? terrain_tiles_json(*config.terrain)
        : terrain_tiles_json_for_mission_fallback(mission, 96);
    manifest.terrain_tiles_json = merge_json_arrays(
        terrain_evidence, terrain_tiles_json_for_faults(config.faults));
    manifest.terrain_tiles_hash = sha256_hex(manifest.terrain_tiles_json);
    manifest.weather_config_json = weather_config_json(config.steady_wind_mps);
    manifest.weather_config_hash = sha256_hex(manifest.weather_config_json);
    manifest.sensor_config_json = sensor_config_json(config.sensor_profile);
    manifest.sensor_config_hash = sha256_hex(manifest.sensor_config_json);
    manifest.lidar_config_json = lidar_config_json(config.lidar);
    manifest.lidar_config_hash = sha256_hex(manifest.lidar_config_json);
    manifest.safety_config_json = safety_config_json(config.safety);
    manifest.safety_config_hash = sha256_hex(manifest.safety_config_json);
    {
        std::ostringstream run_id_input;
        run_id_input << std::fixed << std::setprecision(9)
                     << manifest.simulator_version << "|"
                     << manifest.contract_version << "|"
                     << manifest.contract_schema_hash << "|"
                     << manifest.seed << "|"
                     << manifest.timestep_s << "|"
                     << manifest.record_interval_s << "|"
                     << config.max_time_s << "|"
                     << manifest.mission_hash << "|"
                     << manifest.faults_hash << "|"
                     << manifest.terrain_tiles_hash << "|"
                     << manifest.weather_config_hash << "|"
                     << manifest.sensor_config_hash << "|"
                     << manifest.lidar_config_hash;
        if (!manifest.plant_model.empty()) {
            run_id_input << "|" << manifest.plant_model;
        }
        const std::string default_safety_hash = sha256_hex(safety_config_json(SafetyEnvelope {}));
        if (manifest.safety_config_hash != default_safety_hash) {
            run_id_input << "|" << manifest.safety_config_hash;
        }
        manifest.run_id = sha256_hex(run_id_input.str());
    }
    manifest.step_count = step_count;
    manifest.sample_count = sample_count;
    manifest.lidar_scan_count = lidar_scan_count;
    manifest.prng_nonce = prng_nonce;
    manifest.fault_events_json = fault_events_to_json(fault_events);
    manifest.fault_events_hash = sha256_hex(manifest.fault_events_json);
    manifest.output_hash = sha256_hex(result.trace_jsonl);
    manifest.lidar_output_hash = sha256_hex(result.lidar_scans_jsonl);
    manifest.completed = simulation.state().mode == DroneMode::Completed;
    if (!manifest.completed) {
        if (simulation.state().mode == DroneMode::Failsafe) {
            manifest.termination_reason = "failsafe";
            if (simulation.last_safety_violation().has_value()) {
                manifest.safety_violation = to_string(simulation.last_safety_violation()->code);
            }
        } else {
            manifest.termination_reason = "time_limit";
        }
    }

    return result;
}

} // namespace agbot::flight_sim
