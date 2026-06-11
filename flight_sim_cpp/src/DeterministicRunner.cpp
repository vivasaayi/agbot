#include "agbot_flight_sim/DeterministicRunner.hpp"

#include "agbot_flight_sim/TelemetryRecorder.hpp"

#include <cstdio>
#include <random>
#include <sstream>

namespace agbot::flight_sim {

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
           << "\"simulator_version\":\"" << simulator_version << "\""
           << ",\"contract_version\":\"" << contract_version << "\""
           << ",\"seed\":" << seed
           << ",\"timestep_s\":" << timestep_s
           << ",\"record_interval_s\":" << record_interval_s
           << ",\"mission_name\":\"" << mission_name << "\""
           << ",\"mission_hash\":\"" << mission_hash << "\""
           << ",\"step_count\":" << step_count
           << ",\"sample_count\":" << sample_count
           << ",\"prng_nonce\":" << prng_nonce
           << ",\"output_hash\":\"" << output_hash << "\""
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
    manifest.seed = config.seed;
    manifest.timestep_s = config.timestep_s;
    manifest.record_interval_s = config.record_interval_s;
    manifest.mission_name = mission.name;
    manifest.mission_hash = to_hex(fnv1a64(mission_to_json(mission)));
    manifest.step_count = step_count;
    manifest.sample_count = sample_count;
    manifest.prng_nonce = prng_nonce;
    manifest.output_hash = to_hex(fnv1a64(result.trace_jsonl));
    manifest.completed = simulation.is_complete();

    return result;
}

} // namespace agbot::flight_sim
