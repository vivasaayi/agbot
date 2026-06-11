#include "agbot_flight_sim/SimulationOps.hpp"

#include <algorithm>
#include <fstream>
#include <sstream>
#include <stdexcept>
#include <string_view>
#include <utility>

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

std::string status_text(HealthStatus status) {
    return status == HealthStatus::Pass ? "pass" : "fail";
}

void add_check(HealthReport& report, std::string name, HealthStatus status, std::string message) {
    report.checks.push_back({std::move(name), status, std::move(message)});
}

std::size_t count_trace_files(const std::filesystem::path& trace_dir) {
    if (trace_dir.empty() || !std::filesystem::exists(trace_dir)) {
        return 0;
    }
    if (!std::filesystem::is_directory(trace_dir)) {
        return 0;
    }
    std::size_t count = 0;
    for (const auto& entry : std::filesystem::directory_iterator(trace_dir)) {
        if (entry.is_regular_file() && entry.path().extension() == ".jsonl") {
            ++count;
        }
    }
    return count;
}

bool present_nonempty_file(const std::filesystem::path& path) {
    return !path.empty()
        && std::filesystem::exists(path)
        && std::filesystem::is_regular_file(path)
        && std::filesystem::file_size(path) > 0;
}

struct TraceFile {
    std::filesystem::path path;
    std::filesystem::file_time_type modified_at;
};

} // namespace

bool HealthReport::ok() const {
    return std::all_of(checks.begin(), checks.end(), [](const HealthCheckResult& check) {
        return check.status == HealthStatus::Pass;
    });
}

std::string HealthReport::to_json() const {
    std::ostringstream output;
    output << "{\"ok\":" << (ok() ? "true" : "false") << ",\"checks\":{";
    for (std::size_t index = 0; index < checks.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        const auto& check = checks[index];
        output << "\"" << escape_json(check.name) << "\":{"
               << "\"status\":\"" << status_text(check.status) << "\""
               << ",\"message\":\"" << escape_json(check.message) << "\""
               << "}";
    }
    output << "}}";
    return output.str();
}

std::string TraceRetentionResult::deleted_json() const {
    std::ostringstream output;
    output << "[";
    for (std::size_t index = 0; index < deleted_paths.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        output << "\"" << escape_json(deleted_paths[index].generic_string()) << "\"";
    }
    output << "]";
    return output.str();
}

HealthReport evaluate_simulation_health(const HealthCheckConfig& config) {
    HealthReport report;

    add_check(
        report,
        "runner_mode",
        config.deterministic_runner ? HealthStatus::Pass : HealthStatus::Fail,
        config.deterministic_runner ? "deterministic runner mode configured" : "runner mode is not deterministic");

    add_check(
        report,
        "prng_seeded",
        config.seed.has_value() ? HealthStatus::Pass : HealthStatus::Fail,
        config.seed.has_value() ? "explicit seed present" : "missing explicit seed");

    const bool cache_path_valid = config.terrain_cache_dir.empty()
        || !std::filesystem::exists(config.terrain_cache_dir)
        || std::filesystem::is_directory(config.terrain_cache_dir);
    add_check(
        report,
        "terrain_cache_state",
        cache_path_valid ? HealthStatus::Pass : HealthStatus::Fail,
        cache_path_valid ? "terrain cache path is usable" : "terrain cache path is not a directory");

    add_check(
        report,
        "last_run_manifest_present",
        present_nonempty_file(config.last_manifest_path) ? HealthStatus::Pass : HealthStatus::Fail,
        present_nonempty_file(config.last_manifest_path) ? "last-run manifest present" : "last-run manifest missing");

    const bool trace_dir_valid = config.trace_dir.empty()
        || !std::filesystem::exists(config.trace_dir)
        || std::filesystem::is_directory(config.trace_dir);
    const std::size_t trace_count = trace_dir_valid ? count_trace_files(config.trace_dir) : 0;
    const bool retention_compliant =
        trace_dir_valid && (config.trace_retention_keep == 0 || trace_count <= config.trace_retention_keep);
    add_check(
        report,
        "trace_retention_compliant",
        retention_compliant ? HealthStatus::Pass : HealthStatus::Fail,
        retention_compliant ? "trace count within retention policy" : "trace directory invalid or count exceeds retention policy");

    return report;
}

TraceRetentionResult enforce_trace_retention(const std::filesystem::path& trace_dir, std::size_t keep_count) {
    if (keep_count == 0) {
        throw std::invalid_argument("trace retention keep count must be positive");
    }
    std::filesystem::create_directories(trace_dir);

    std::vector<TraceFile> traces;
    for (const auto& entry : std::filesystem::directory_iterator(trace_dir)) {
        if (entry.is_regular_file() && entry.path().extension() == ".jsonl") {
            traces.push_back({entry.path(), entry.last_write_time()});
        }
    }
    std::sort(traces.begin(), traces.end(), [](const TraceFile& left, const TraceFile& right) {
        if (left.modified_at == right.modified_at) {
            return left.path.generic_string() > right.path.generic_string();
        }
        return left.modified_at > right.modified_at;
    });

    TraceRetentionResult result;
    result.keep_count = keep_count;
    for (std::size_t index = keep_count; index < traces.size(); ++index) {
        const auto trace_path = traces[index].path;
        if (std::filesystem::remove(trace_path)) {
            result.deleted_paths.push_back(trace_path);
        }

        auto manifest_path = trace_path;
        manifest_path.replace_extension(".manifest.json");
        if (std::filesystem::exists(manifest_path) && std::filesystem::remove(manifest_path)) {
            result.deleted_paths.push_back(manifest_path);
        }
    }
    return result;
}

std::uintmax_t clear_tile_cache(const std::filesystem::path& cache_dir) {
    std::filesystem::create_directories(cache_dir);
    std::uintmax_t removed = 0;
    for (const auto& entry : std::filesystem::directory_iterator(cache_dir)) {
        removed += std::filesystem::remove_all(entry.path());
    }
    return removed;
}

std::filesystem::path default_map_tile_cache_dir() {
    return std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out" / "map_tiles";
}

std::filesystem::path default_elevation_tile_cache_dir() {
    return std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "out" / "elevation_tiles";
}

} // namespace agbot::flight_sim
