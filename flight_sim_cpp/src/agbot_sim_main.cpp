#include "agbot_flight_sim/TraceDiff.hpp"
#include "agbot_flight_sim/SimulationOps.hpp"

#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <optional>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>

namespace {

std::string read_all(const std::filesystem::path& path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        throw std::runtime_error("unable to open trace: " + path.string());
    }
    std::ostringstream buffer;
    buffer << file.rdbuf();
    return buffer.str();
}

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

void print_usage() {
    std::cout << "Usage:\n"
              << "  agbot-sim diff <trace-a.jsonl> <trace-b.jsonl>\n"
              << "  agbot-sim health --seed N --last-manifest PATH [--trace-dir PATH] [--cache-dir PATH] [--retention-keep N]\n"
              << "  agbot-sim cache clear [--cache-dir PATH]\n";
}

struct HealthArgs {
    std::optional<std::uint64_t> seed;
    std::filesystem::path cache_dir = agbot::flight_sim::default_map_tile_cache_dir();
    std::filesystem::path trace_dir = std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out";
    std::filesystem::path last_manifest_path =
        std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out" / "telemetry.manifest.json";
    std::size_t retention_keep = 0;
};

HealthArgs parse_health_args(int argc, char** argv) {
    HealthArgs args;
    for (int index = 2; index < argc; ++index) {
        const std::string current = argv[index];
        if (current == "--seed" && index + 1 < argc) {
            args.seed = static_cast<std::uint64_t>(std::stoull(argv[++index]));
        } else if (current == "--cache-dir" && index + 1 < argc) {
            args.cache_dir = argv[++index];
        } else if (current == "--trace-dir" && index + 1 < argc) {
            args.trace_dir = argv[++index];
        } else if (current == "--last-manifest" && index + 1 < argc) {
            args.last_manifest_path = argv[++index];
        } else if (current == "--retention-keep" && index + 1 < argc) {
            args.retention_keep = static_cast<std::size_t>(std::stoull(argv[++index]));
        } else {
            throw std::runtime_error("unknown health argument: " + current);
        }
    }
    return args;
}

std::filesystem::path parse_cache_clear_args(int argc, char** argv) {
    std::filesystem::path cache_dir = agbot::flight_sim::default_map_tile_cache_dir();
    for (int index = 3; index < argc; ++index) {
        const std::string current = argv[index];
        if (current == "--cache-dir" && index + 1 < argc) {
            cache_dir = argv[++index];
        } else {
            throw std::runtime_error("unknown cache clear argument: " + current);
        }
    }
    return cache_dir;
}

} // namespace

int main(int argc, char** argv) {
    try {
        if (argc == 2 && (std::string(argv[1]) == "--help" || std::string(argv[1]) == "-h")) {
            print_usage();
            return 0;
        }
        if (argc >= 2 && std::string(argv[1]) == "health") {
            const HealthArgs args = parse_health_args(argc, argv);
            agbot::flight_sim::HealthCheckConfig config;
            config.seed = args.seed;
            config.terrain_cache_dir = args.cache_dir;
            config.trace_dir = args.trace_dir;
            config.last_manifest_path = args.last_manifest_path;
            config.trace_retention_keep = args.retention_keep;
            const auto report = agbot::flight_sim::evaluate_simulation_health(config);
            std::cout << report.to_json() << "\n";
            return report.ok() ? 0 : 1;
        }
        if (argc >= 3 && std::string(argv[1]) == "cache" && std::string(argv[2]) == "clear") {
            const auto cache_dir = parse_cache_clear_args(argc, argv);
            const std::uintmax_t removed = agbot::flight_sim::clear_tile_cache(cache_dir);
            std::cout << "{\"cache_dir\":\"" << escape_json(cache_dir.generic_string())
                      << "\",\"removed_entries\":" << removed << "}\n";
            return 0;
        }
        if (argc != 4 || std::string(argv[1]) != "diff") {
            print_usage();
            return 2;
        }

        const std::string left = read_all(argv[2]);
        const std::string right = read_all(argv[3]);
        const auto diff = agbot::flight_sim::diff_trace_text(left, right);
        std::cout << diff.message << "\n";
        return diff.identical ? 0 : 1;
    } catch (const std::exception& error) {
        std::cerr << "agbot-sim: " << error.what() << "\n";
        return 2;
    }
}
