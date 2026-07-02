#pragma once

#include "agbot_terrain/Raster.hpp"

#include <cstdint>
#include <filesystem>
#include <string>

namespace agbot::terrain {

// Error metrics of an estimated surface against a reference heightfield,
// computed over cells where both grids have data.
struct ValidationMetrics {
    std::size_t sample_count = 0;
    double rmse = 0.0;
    double mae = 0.0;
    double bias = 0.0;    // mean(estimate - reference)
    double nmad = 0.0;    // 1.4826 * median(|error - median(error)|)
    double max_abs = 0.0;
    double pct_within_1m = 0.0;
    double pct_within_5m = 0.0;
};

struct ValidationReport {
    bool ok = false;
    std::string error; // e.g. "validation_grid_mismatch", "validation_no_samples"
    std::string reference_name;
    ValidationMetrics metrics;
    std::uint64_t param_hash = 0;
    std::uint64_t fused_raster_hash = 0;
};

[[nodiscard]] ValidationMetrics compute_metrics(const Raster& estimate, const Raster& reference);

// Deterministic JSON (fixed key order, fixed float formatting). Creates
// parent directories. Returns false with a reason code in *error on failure.
[[nodiscard]] bool write_validation_json(
    const std::filesystem::path& path,
    const ValidationReport& report,
    std::string* error);

} // namespace agbot::terrain
