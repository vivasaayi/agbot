#include "agbot_flight_sim/DeterministicRunner.hpp"
#include "agbot_flight_sim/MissionLoader.hpp"
#include "agbot_flight_sim/SimulationOps.hpp"

#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <optional>
#include <stdexcept>
#include <string>
#include <utility>

using agbot::flight_sim::MissionLoader;
using agbot::flight_sim::RunConfig;
using agbot::flight_sim::RunResult;
using agbot::flight_sim::default_sample_mission_path;
using agbot::flight_sim::kSimulatorVersion;
using agbot::flight_sim::kTwinContractVersion;
using agbot::flight_sim::run_deterministic;

namespace {

struct Args {
    std::filesystem::path mission_path = default_sample_mission_path();
    std::filesystem::path output_path = std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out" / "telemetry.jsonl";
    std::optional<std::uint64_t> seed; // required: deterministic mode demands an explicit seed
    double timestep_ms = 1000.0 / 60.0;
    double record_interval_s = 0.25;
    double max_time_s = 600.0;
    std::optional<std::size_t> trace_retention_keep;
    agbot::flight_sim::Vec3 steady_wind_mps;
    agbot::flight_sim::SensorCalibrationProfile sensor_profile = agbot::flight_sim::ideal_sensor_profile();
    agbot::flight_sim::LidarRaycastConfig lidar;
    agbot::flight_sim::FaultInjectionPlan faults;
};

agbot::flight_sim::Vec3 parse_vec3_csv(const std::string& text, const std::string& field) {
    const std::size_t first_comma = text.find(',');
    const std::size_t second_comma = first_comma == std::string::npos
        ? std::string::npos
        : text.find(',', first_comma + 1);
    if (first_comma == std::string::npos || second_comma == std::string::npos
        || text.find(',', second_comma + 1) != std::string::npos) {
        throw std::runtime_error(field + " must be formatted as X,Y,Z");
    }

    return {
        std::stod(text.substr(0, first_comma)),
        std::stod(text.substr(first_comma + 1, second_comma - first_comma - 1)),
        std::stod(text.substr(second_comma + 1)),
    };
}

std::pair<std::uint32_t, std::uint32_t> parse_u32_pair_csv(const std::string& text, const std::string& field) {
    const std::size_t comma = text.find(',');
    if (comma == std::string::npos || text.find(',', comma + 1) != std::string::npos) {
        throw std::runtime_error(field + " must be formatted as H,V");
    }

    const auto horizontal = static_cast<std::uint32_t>(std::stoul(text.substr(0, comma)));
    const auto vertical = static_cast<std::uint32_t>(std::stoul(text.substr(comma + 1)));
    return {horizontal, vertical};
}

[[noreturn]] void print_usage_and_exit(int code) {
    std::cout << "Usage: agbot_flight_sim_headless --seed N [options]\n"
              << "  --seed N             REQUIRED. Seed for deterministic run (refuses to start without it).\n"
              << "  --timestep-ms MS     Fixed timestep in milliseconds (default 16.667).\n"
              << "  --record-interval S  Telemetry sampling interval in seconds (default 0.25).\n"
              << "  --mission PATH       Mission JSON to fly (default: bundled sample).\n"
              << "  --output PATH        Telemetry JSONL output (default: out/telemetry.jsonl).\n"
              << "                       A <output>.manifest.json is written alongside it.\n"
              << "  --max-time S         Max mission seconds before giving up (default 600).\n"
              << "  --wind-mps X,Y,Z     Steady wind vector in m/s applied to airborne ground track.\n"
              << "  --sensor-profile NAME\n"
              << "                       Sensor calibration/noise profile: ideal, cheap_gps, rtk_gps, noisy_imu.\n"
              << "  --disable-lidar      Do not emit the deterministic LiDAR JSONL sidecar.\n"
              << "  --lidar-samples H,V  Horizontal samples and vertical rings (default 36,3).\n"
              << "  --lidar-max-range M  Maximum LiDAR raycast range in meters (default 80).\n"
              << "  --lidar-range-noise M\n"
              << "                       Seeded uniform range noise in meters (default 0).\n"
              << "  --trace-retention-keep N\n"
              << "                       Delete older JSONL traces in the output directory after keeping N newest runs.\n"
              << "  --fault SPEC         Add seeded fault: class:seed:start_step:end_step:magnitude[:target].\n"
              << "                       Use '-' for open end_step. Classes: wind_gust, gps_drift, imu_noise,\n"
              << "                       sensor_dropout, comm_loss, low_battery, stale_terrain, bad_tile, actuator_lag.\n";
    std::exit(code);
}

Args parse_args(int argc, char** argv) {
    Args args;
    for (int index = 1; index < argc; ++index) {
        const std::string current = argv[index];
        if (current == "--mission" && index + 1 < argc) {
            args.mission_path = argv[++index];
        } else if (current == "--output" && index + 1 < argc) {
            args.output_path = argv[++index];
        } else if (current == "--seed" && index + 1 < argc) {
            args.seed = static_cast<std::uint64_t>(std::stoull(argv[++index]));
        } else if (current == "--timestep-ms" && index + 1 < argc) {
            args.timestep_ms = std::stod(argv[++index]);
        } else if (current == "--record-interval" && index + 1 < argc) {
            args.record_interval_s = std::stod(argv[++index]);
        } else if (current == "--max-time" && index + 1 < argc) {
            args.max_time_s = std::stod(argv[++index]);
        } else if (current == "--wind-mps" && index + 1 < argc) {
            args.steady_wind_mps = parse_vec3_csv(argv[++index], "--wind-mps");
        } else if (current == "--sensor-profile" && index + 1 < argc) {
            args.sensor_profile = agbot::flight_sim::sensor_profile_by_name(argv[++index]);
        } else if (current == "--disable-lidar") {
            args.lidar.enabled = false;
        } else if (current == "--lidar-samples" && index + 1 < argc) {
            const auto [horizontal, vertical] = parse_u32_pair_csv(argv[++index], "--lidar-samples");
            args.lidar.horizontal_samples = horizontal;
            args.lidar.vertical_samples = vertical;
        } else if (current == "--lidar-max-range" && index + 1 < argc) {
            args.lidar.max_range_m = std::stod(argv[++index]);
        } else if (current == "--lidar-range-noise" && index + 1 < argc) {
            args.lidar.range_noise_m = std::stod(argv[++index]);
        } else if (current == "--trace-retention-keep" && index + 1 < argc) {
            args.trace_retention_keep = static_cast<std::size_t>(std::stoull(argv[++index]));
        } else if (current == "--fault" && index + 1 < argc) {
            args.faults.faults.push_back(agbot::flight_sim::parse_fault_spec(argv[++index]));
        } else if (current == "--help" || current == "-h") {
            print_usage_and_exit(0);
        } else {
            throw std::runtime_error("Unknown argument: " + current);
        }
    }
    if (!args.seed.has_value()) {
        throw std::runtime_error("deterministic mode requires --seed");
    }
    if (args.timestep_ms <= 0.0) {
        throw std::runtime_error("--timestep-ms must be positive");
    }
    if (args.trace_retention_keep.has_value() && *args.trace_retention_keep == 0) {
        throw std::runtime_error("--trace-retention-keep must be positive");
    }
    if (args.lidar.enabled && (args.lidar.horizontal_samples == 0 || args.lidar.vertical_samples == 0)) {
        throw std::runtime_error("--lidar-samples values must be positive");
    }
    if (args.lidar.max_range_m <= 0.0) {
        throw std::runtime_error("--lidar-max-range must be positive");
    }
    if (args.lidar.range_noise_m < 0.0) {
        throw std::runtime_error("--lidar-range-noise must be non-negative");
    }
    agbot::flight_sim::validate_fault_plan(args.faults);
    return args;
}

void write_file(const std::filesystem::path& path, const std::string& contents) {
    if (!path.parent_path().empty()) {
        std::filesystem::create_directories(path.parent_path());
    }
    std::ofstream stream(path, std::ios::binary);
    if (!stream) {
        throw std::runtime_error("Unable to open output: " + path.string());
    }
    stream << contents;
}

} // namespace

int main(int argc, char** argv) {
    try {
        const Args args = parse_args(argc, argv);

        auto mission = MissionLoader::load_from_file(args.mission_path);

        RunConfig config;
        config.seed = *args.seed;
        config.timestep_s = args.timestep_ms / 1000.0;
        config.record_interval_s = args.record_interval_s;
        config.max_time_s = args.max_time_s;
        config.steady_wind_mps = args.steady_wind_mps;
        config.sensor_profile = args.sensor_profile;
        config.lidar = args.lidar;
        config.faults = args.faults;

        RunResult result = run_deterministic(mission, config);

        // Run header: log the determinism inputs on every run (story 02-31).
        std::cout << "agbot_flight_sim_headless"
                  << " sim=" << kSimulatorVersion
                  << " contract=" << kTwinContractVersion
                  << " seed=" << *args.seed
                  << " timestep_ms=" << args.timestep_ms
                  << " run_id=" << result.manifest.run_id << "\n";

        const std::filesystem::path manifest_path =
            std::filesystem::path(args.output_path).replace_extension(".manifest.json");
        const std::filesystem::path lidar_path =
            std::filesystem::path(args.output_path).replace_extension(".lidar.jsonl");
        write_file(args.output_path, result.trace_jsonl);
        if (args.lidar.enabled) {
            write_file(lidar_path, result.lidar_scans_jsonl);
        }
        if (args.trace_retention_keep.has_value()) {
            const auto retention = agbot::flight_sim::enforce_trace_retention(
                std::filesystem::path(args.output_path).parent_path(),
                *args.trace_retention_keep);
            result.manifest.trace_retention_keep = retention.keep_count;
            result.manifest.trace_retention_deleted_json = retention.deleted_json();
        }
        write_file(manifest_path, result.manifest.to_json() + "\n");

        std::cout << "Mission: " << result.manifest.mission_name << "\n"
                  << "Completed: " << (result.manifest.completed ? "yes" : "no") << "\n"
                  << "Samples: " << result.manifest.sample_count << "\n"
                  << "LiDAR scans: " << result.manifest.lidar_scan_count << "\n"
                  << "Output hash: " << result.manifest.output_hash << "\n"
                  << "Telemetry: " << args.output_path << "\n"
                  << "LiDAR: " << (args.lidar.enabled ? lidar_path.string() : "disabled") << "\n"
                  << "Manifest: " << manifest_path << "\n";

        return result.manifest.completed ? 0 : 2;
    } catch (const std::exception& error) {
        std::cerr << "agbot_flight_sim_headless: " << error.what() << "\n";
        return 1;
    }
}
