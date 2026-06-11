#pragma once

#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/FaultInjection.hpp"
#include "agbot_flight_sim/LidarSimulator.hpp"
#include "agbot_flight_sim/Mission.hpp"
#include "agbot_flight_sim/SensorModel.hpp"

#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>

namespace agbot::flight_sim {

/// Version of the simulator's binary/behavior. Bump on any change that can
/// alter telemetry output so golden fixtures and manifests stay attributable.
inline constexpr char kSimulatorVersion[] = "0.1.0";

/// Version of the twin wire contract (commands, telemetry, trace, manifest).
/// This is the seed of TwinContractV1 (story 02-24): any breaking change to the
/// telemetry/manifest shape must bump this. The trace diff CLI and
/// cross-build/cross-platform checks reject traces produced under an
/// incompatible contract version.
inline constexpr char kTwinContractVersion[] = "1.0.0";

/// 64-bit FNV-1a hash, rendered as a lowercase hex string. Used for the
/// scenario manifest's deterministic input/output hashes. No external deps so
/// the result is stable across platforms for identical byte input.
[[nodiscard]] std::uint64_t fnv1a64(std::string_view bytes);
[[nodiscard]] std::string to_hex(std::uint64_t value);

/// Configuration for a single deterministic run. A run is fully reproducible
/// from (mission, seed, timestep) alone: no wall-clock, no unseeded RNG.
struct RunConfig {
    std::uint64_t seed = 0;
    double timestep_s = 1.0 / 60.0;
    double record_interval_s = 0.25;
    double max_time_s = 600.0;
    Vec3 steady_wind_mps;
    SensorCalibrationProfile sensor_profile = ideal_sensor_profile();
    LidarRaycastConfig lidar;
    FaultInjectionPlan faults;
};

/// Per-run scenario manifest (story 02-28, minimal first slice). Records the
/// inputs and output hashes that make a trace auditable back to its inputs.
struct RunManifest {
    std::string simulator_version;
    std::string contract_version;
    std::string contract_schema_hash;
    std::string run_id;
    std::uint64_t seed = 0;
    double timestep_s = 0.0;
    double record_interval_s = 0.0;
    std::string mission_name;
    std::string mission_hash;     // SHA-256 hash of the canonical mission JSON
    std::uint64_t step_count = 0; // fixed-timestep steps executed
    std::uint64_t sample_count = 0;
    std::uint64_t prng_nonce = 0; // first draw from the seeded PRNG; proves the
                                  // seed drives the stream even when physics is
                                  // currently seed-independent
    std::string terrain_tiles_json = "[]";
    std::string terrain_tiles_hash;
    std::string weather_config_json = "{}";
    std::string weather_config_hash;
    std::string sensor_config_json = "{}";
    std::string sensor_config_hash;
    std::string lidar_config_json = "{}";
    std::string lidar_config_hash;
    std::uint64_t lidar_scan_count = 0;
    std::string lidar_output_hash;
    std::string safety_config_json = "{}";
    std::string safety_config_hash;
    std::size_t trace_retention_keep = 0;
    std::string trace_retention_deleted_json = "[]";
    std::string faults_json = "[]";
    std::string faults_hash;
    std::string fault_events_json = "[]";
    std::string fault_events_hash;
    std::string output_hash;      // SHA-256 hash of the emitted JSONL trace
    bool completed = false;

    [[nodiscard]] std::string to_json() const;
};

struct RunResult {
    std::string trace_jsonl; // full telemetry trace, one JSON object per line
    std::string lidar_scans_jsonl; // capture-shaped LidarScan JSONL, one scan per recorded sample
    RunManifest manifest;
};

/// Run a mission deterministically and return its trace plus manifest. Running
/// the same mission with the same RunConfig is guaranteed to produce a
/// byte-identical trace and identical manifest hashes.
[[nodiscard]] RunResult run_deterministic(const Mission& mission, const RunConfig& config);

} // namespace agbot::flight_sim
