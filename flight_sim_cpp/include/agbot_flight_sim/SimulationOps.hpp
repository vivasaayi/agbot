#pragma once

#include <cstdint>
#include <filesystem>
#include <optional>
#include <string>
#include <vector>

namespace agbot::flight_sim {

enum class HealthStatus {
    Pass,
    Fail,
};

struct HealthCheckResult {
    std::string name;
    HealthStatus status = HealthStatus::Fail;
    std::string message;
};

struct HealthReport {
    std::vector<HealthCheckResult> checks;

    [[nodiscard]] bool ok() const;
    [[nodiscard]] std::string to_json() const;
};

struct HealthCheckConfig {
    bool deterministic_runner = true;
    std::optional<std::uint64_t> seed;
    std::filesystem::path terrain_cache_dir;
    std::filesystem::path trace_dir;
    std::filesystem::path last_manifest_path;
    std::size_t trace_retention_keep = 0;
};

struct TraceRetentionResult {
    std::size_t keep_count = 0;
    std::vector<std::filesystem::path> deleted_paths;

    [[nodiscard]] std::string deleted_json() const;
};

[[nodiscard]] HealthReport evaluate_simulation_health(const HealthCheckConfig& config);
[[nodiscard]] TraceRetentionResult enforce_trace_retention(
    const std::filesystem::path& trace_dir,
    std::size_t keep_count);
[[nodiscard]] std::uintmax_t clear_tile_cache(const std::filesystem::path& cache_dir);
[[nodiscard]] std::filesystem::path default_map_tile_cache_dir();
[[nodiscard]] std::filesystem::path default_elevation_tile_cache_dir();

} // namespace agbot::flight_sim
