#include "agbot_flight_sim/DeterministicRunner.hpp"
#include "agbot_flight_sim/MissionLoader.hpp"
#include "agbot_flight_sim/MissionValidation.hpp"
#include "agbot_flight_sim/SimulationOps.hpp"
#include "agbot_flight_sim/AssetPaths.hpp"
#include "agbot_flight_sim/TwinContractV1.hpp"

#include "agbot_config/Toml.hpp"
#include "agbot_render/WorldPackage.hpp"

#include <array>
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

// Default seed used when neither the CLI nor the settings file supplies one,
// so the simulator runs deterministically with zero arguments.
constexpr std::uint64_t kDefaultSeed = 1;

struct Args {
    std::filesystem::path mission_path = default_sample_mission_path();
    std::optional<std::filesystem::path> terrain_package_path;
    std::filesystem::path output_path = agbot::flight_sim::sim_out_dir() / "telemetry.jsonl";
    std::optional<std::uint64_t> seed; // resolved from settings file / default when absent
    double timestep_ms = 1000.0 / 60.0;
    double record_interval_s = 0.25;
    double max_time_s = 600.0;
    agbot::flight_sim::PlantModel plant_model = agbot::flight_sim::PlantModel::Simple;
    agbot::flight_sim::SafetyEnvelope safety;
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

std::array<double, 4> parse_geofence_csv(const std::string& text) {
    std::array<double, 4> values {};
    std::size_t start = 0;
    for (std::size_t index = 0; index < values.size(); ++index) {
        const std::size_t end = text.find(',', start);
        if ((index < values.size() - 1 && end == std::string::npos)
            || (index == values.size() - 1 && end != std::string::npos)) {
            throw std::runtime_error("--geofence must be formatted as min_x,max_x,min_z,max_z");
        }
        const std::size_t length = end == std::string::npos ? std::string::npos : end - start;
        values[index] = std::stod(text.substr(start, length));
        start = end == std::string::npos ? text.size() : end + 1;
    }
    return values;
}

[[noreturn]] void print_usage_and_exit(int code) {
    std::cout << "Usage: agbot_flight_sim_headless [options]\n"
              << "  --seed N             Seed for deterministic run (default from config/flight_sim.toml, else 1).\n"
              << "  --timestep-ms MS     Fixed timestep in milliseconds (default 16.667).\n"
              << "  --record-interval S  Telemetry sampling interval in seconds (default 0.25).\n"
              << "  --mission PATH       Mission JSON to fly (default: bundled sample).\n"
              << "  --terrain-package PATH\n"
              << "                       L3 .agbworld terrain package used by LiDAR and run evidence.\n"
              << "  --output PATH        Telemetry JSONL output (default: out/telemetry.jsonl).\n"
              << "                       A <output>.manifest.json is written alongside it.\n"
              << "  --max-time S         Max mission seconds before giving up (default 600).\n"
              << "  --plant MODEL        Physics plant: simple or multirotor (default simple).\n"
              << "  --max-altitude M     Preflight and runtime altitude ceiling in meters.\n"
              << "  --min-battery PCT    Preflight and runtime battery abort threshold.\n"
              << "  --geofence BOUNDS    Preflight and runtime min_x,max_x,min_z,max_z bounds.\n"
              << "  --wind-mps X,Y,Z     Steady wind vector in m/s applied to airborne ground track.\n"
              << "  --sensor-profile NAME\n"
              << "                       Sensor calibration/noise profile: ideal, cheap_gps_b2, rtk_gps_a1,\n"
              << "                       noisy_imu, lidar_a3, multispectral_camera.\n"
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

// Apply defaults from the user-editable settings file (config/flight_sim.toml)
// so a bare `agbot_flight_sim_headless` invocation runs with sensible, tunable
// defaults. Missing file or fields fall back to the compiled defaults; a
// malformed file is reported and ignored rather than fatal.
void apply_settings_defaults(Args& args) {
    args.seed = kDefaultSeed;

    const std::filesystem::path settings_path =
        agbot::flight_sim::sim_config_dir() / "flight_sim.toml";
    std::error_code ec;
    if (!std::filesystem::exists(settings_path, ec)) {
        return;
    }

    const agbot::config::TomlParseResult parsed =
        agbot::config::parse_toml_file(settings_path);
    if (!parsed.ok) {
        std::cerr << "warning: ignoring invalid settings file "
                  << settings_path << ": " << parsed.error << "\n";
        return;
    }

    // Accept keys either at the top level or under a [run] table.
    const agbot::config::ParamTable* run =
        agbot::config::find_table(parsed.root, "run");
    const agbot::config::ParamTable& table = run != nullptr ? *run : parsed.root;

    args.seed = static_cast<std::uint64_t>(agbot::config::integer_or(
        table, "seed", static_cast<std::int64_t>(kDefaultSeed)));
    args.timestep_ms = agbot::config::double_or(table, "timestep_ms", args.timestep_ms);
    args.record_interval_s =
        agbot::config::double_or(table, "record_interval_s", args.record_interval_s);
    args.max_time_s = agbot::config::double_or(table, "max_time_s", args.max_time_s);
    args.plant_model = agbot::flight_sim::plant_model_from_string(
        agbot::config::string_or(table, "plant_model", to_string(args.plant_model)));
}

Args parse_args(int argc, char** argv) {
    Args args;
    apply_settings_defaults(args);
    for (int index = 1; index < argc; ++index) {
        const std::string current = argv[index];
        if (current == "--mission" && index + 1 < argc) {
            args.mission_path = argv[++index];
        } else if (current == "--terrain-package" && index + 1 < argc) {
            args.terrain_package_path = std::filesystem::path(argv[++index]);
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
        } else if (current == "--plant" && index + 1 < argc) {
            args.plant_model = agbot::flight_sim::plant_model_from_string(argv[++index]);
        } else if (current == "--max-altitude" && index + 1 < argc) {
            args.safety.max_altitude_m = std::stod(argv[++index]);
        } else if (current == "--min-battery" && index + 1 < argc) {
            args.safety.min_battery_percent = std::stod(argv[++index]);
        } else if (current == "--geofence" && index + 1 < argc) {
            const auto bounds = parse_geofence_csv(argv[++index]);
            args.safety.min_x_m = bounds[0];
            args.safety.max_x_m = bounds[1];
            args.safety.min_z_m = bounds[2];
            args.safety.max_z_m = bounds[3];
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
        // Should not happen: apply_settings_defaults always seeds a default.
        args.seed = kDefaultSeed;
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
    if (args.safety.max_altitude_m <= 0.0) {
        throw std::runtime_error("--max-altitude must be positive");
    }
    if (args.safety.min_battery_percent < 0.0 || args.safety.min_battery_percent > 100.0) {
        throw std::runtime_error("--min-battery must be between 0 and 100");
    }
    if (args.safety.min_x_m > args.safety.max_x_m || args.safety.min_z_m > args.safety.max_z_m) {
        throw std::runtime_error("--geofence minimum bounds must not exceed maximum bounds");
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
        std::optional<agbot::flight_sim::RuntimeTerrain>
            runtime_terrain;
        if (args.terrain_package_path.has_value()) {
            if (!mission.home_geo.has_value()) {
                throw std::runtime_error(
                    "--terrain-package requires a georeferenced mission");
            }
            const auto terrain = agbot::render::load_world_terrain(
                *args.terrain_package_path, *mission.home_geo);
            if (!terrain.ok()) {
                throw std::runtime_error(
                    "Unable to load terrain package: " + *terrain.error);
            }
            if (!agbot::flight_sim::terrain_covers_mission(
                    terrain.terrain, mission)) {
                throw std::runtime_error(
                    "Terrain package does not cover the mission home and waypoints");
            }
            runtime_terrain = terrain.terrain;
        }

        agbot::flight_sim::MissionValidationConfig validation_config;
        validation_config.safety = args.safety;
        if (runtime_terrain.has_value()) {
            validation_config.terrain = runtime_terrain->mesh;
        }
        const auto validation = agbot::flight_sim::validate_mission(mission, validation_config);
        const std::filesystem::path validation_path =
            std::filesystem::path(args.output_path).replace_extension(".validation.json");
        write_file(validation_path, validation.to_json() + "\n");
        if (validation.blocked) {
            std::cout << "Mission: " << validation.mission_name << "\n"
                      << "Preflight: blocked\n"
                      << "Validation: " << validation_path << "\n";
            return 4;
        }

        RunConfig config;
        config.seed = *args.seed;
        config.plant_model = args.plant_model;
        config.timestep_s = args.timestep_ms / 1000.0;
        config.record_interval_s = args.record_interval_s;
        config.max_time_s = args.max_time_s;
        config.safety = args.safety;
        config.steady_wind_mps = args.steady_wind_mps;
        config.sensor_profile = args.sensor_profile;
        config.lidar = args.lidar;
        config.faults = args.faults;
        if (runtime_terrain.has_value()) {
            config.terrain = *runtime_terrain;
        }

        RunResult result = run_deterministic(mission, config);
        result.manifest.validation_report_json = validation.to_json();
        result.manifest.validation_report_hash =
            agbot::flight_sim::sha256_hex(result.manifest.validation_report_json);

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
                  << "Outcome: " << (result.manifest.completed
                      ? "completed"
                      : result.manifest.termination_reason) << "\n"
                  << "Samples: " << result.manifest.sample_count << "\n"
                  << "LiDAR scans: " << result.manifest.lidar_scan_count << "\n"
                  << "Output hash: " << result.manifest.output_hash << "\n"
                  << "Telemetry: " << args.output_path << "\n"
                  << "LiDAR: " << (args.lidar.enabled ? lidar_path.string() : "disabled") << "\n"
                  << "Validation: " << validation_path << "\n"
                  << "Manifest: " << manifest_path << "\n";

        if (result.manifest.completed) {
            return 0;
        }
        return result.manifest.termination_reason == "failsafe" ? 3 : 2;
    } catch (const std::exception& error) {
        std::cerr << "agbot_flight_sim_headless: " << error.what() << "\n";
        return 1;
    }
}
