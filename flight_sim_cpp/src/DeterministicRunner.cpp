#include "agbot_flight_sim/DeterministicRunner.hpp"

#include "agbot_flight_sim/TelemetryRecorder.hpp"
#include "agbot_flight_sim/TwinContractV1.hpp"

#include <cstdio>
#include <iomanip>
#include <random>
#include <sstream>

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

std::string default_weather_config_json() {
    return "{\"wind_mps\":{\"x\":0.000,\"y\":0.000,\"z\":0.000}}";
}

std::string default_sensor_config_json() {
    return "{\"profiles\":[],\"noise_model\":\"none\"}";
}

std::string default_safety_config_json() {
    SimulationConfig config;
    std::ostringstream stream;
    stream << std::fixed << std::setprecision(3)
           << "{\"min_battery_percent\":" << config.min_battery_percent
           << ",\"max_altitude_m\":\"unbounded\""
           << ",\"geofence\":\"unbounded\""
           << ",\"no_fly_zone_count\":" << config.safety.no_fly_zones.size()
           << "}";
    return stream.str();
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
           << ",\"seed\":" << seed
           << ",\"timestep_s\":" << timestep_s
           << ",\"record_interval_s\":" << record_interval_s
           << ",\"mission_name\":\"" << escape_json(mission_name) << "\""
           << ",\"mission_hash\":\"" << escape_json(mission_hash) << "\""
           << ",\"step_count\":" << step_count
           << ",\"sample_count\":" << sample_count
           << ",\"prng_nonce\":" << prng_nonce
           << ",\"terrain_tiles\":" << terrain_tiles_json
           << ",\"terrain_tiles_hash\":\"" << escape_json(terrain_tiles_hash) << "\""
           << ",\"weather_config\":" << weather_config_json
           << ",\"weather_config_hash\":\"" << escape_json(weather_config_hash) << "\""
           << ",\"sensor_config\":" << sensor_config_json
           << ",\"sensor_config_hash\":\"" << escape_json(sensor_config_hash) << "\""
           << ",\"safety_config\":" << safety_config_json
           << ",\"safety_config_hash\":\"" << escape_json(safety_config_hash) << "\""
           << ",\"output_hash\":\"" << escape_json(output_hash) << "\""
           << ",\"completed\":" << (completed ? "true" : "false")
           << "}";
    return stream.str();
}

RunResult run_deterministic(const Mission& mission, const RunConfig& config) {
    // Seeded PRNG. Currently the autopilot physics is deterministic and
    // seed-independent; the stream is plumbed here so that future sensor noise
    // and fault injection (stories 02-08/02-30) are reproducible by seed. The
    // first draw is recorded in the manifest so the seed is provably driving a
    // stream even before noise is wired in.
    std::mt19937_64 prng(config.seed);
    const std::uint64_t prng_nonce = prng();

    DroneSimulation simulation(mission);

    std::ostringstream trace;
    std::uint64_t step_count = 0;
    std::uint64_t sample_count = 0;
    double next_record_s = 0.0;

    const auto record = [&](const DroneState& state) {
        trace << format_telemetry_sample(state) << "\n";
        ++sample_count;
    };

    while (!simulation.is_complete() && simulation.state().mission_time_s < config.max_time_s) {
        if (simulation.state().mission_time_s >= next_record_s) {
            record(simulation.state());
            next_record_s += config.record_interval_s;
        }
        simulation.step(config.timestep_s);
        ++step_count;
    }
    // Always record the terminal state so the trace ends at the true outcome.
    record(simulation.state());

    RunResult result;
    result.trace_jsonl = trace.str();

    RunManifest& manifest = result.manifest;
    manifest.simulator_version = kSimulatorVersion;
    manifest.contract_version = kTwinContractVersion;
    manifest.contract_schema_hash = twin_contract_v1_schema().schema_hash;
    manifest.seed = config.seed;
    manifest.timestep_s = config.timestep_s;
    manifest.record_interval_s = config.record_interval_s;
    manifest.mission_name = mission.name;
    manifest.mission_hash = sha256_hex(mission_to_json(mission));
    manifest.step_count = step_count;
    manifest.sample_count = sample_count;
    manifest.prng_nonce = prng_nonce;
    manifest.terrain_tiles_json = "[]";
    manifest.terrain_tiles_hash = sha256_hex(manifest.terrain_tiles_json);
    manifest.weather_config_json = default_weather_config_json();
    manifest.weather_config_hash = sha256_hex(manifest.weather_config_json);
    manifest.sensor_config_json = default_sensor_config_json();
    manifest.sensor_config_hash = sha256_hex(manifest.sensor_config_json);
    manifest.safety_config_json = default_safety_config_json();
    manifest.safety_config_hash = sha256_hex(manifest.safety_config_json);
    manifest.output_hash = sha256_hex(result.trace_jsonl);
    manifest.completed = simulation.is_complete();

    return result;
}

} // namespace agbot::flight_sim
