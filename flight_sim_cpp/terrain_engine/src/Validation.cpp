#include "agbot_terrain/Validation.hpp"

#include <algorithm>
#include <cmath>
#include <fstream>
#include <iomanip>
#include <sstream>
#include <vector>

namespace agbot::terrain {
namespace {

double median_of_sorted(std::vector<double>& values) {
    std::sort(values.begin(), values.end());
    const std::size_t n = values.size();
    if (n == 0) {
        return 0.0;
    }
    if (n % 2 == 1) {
        return values[n / 2];
    }
    return 0.5 * (values[n / 2 - 1] + values[n / 2]);
}

void append_metric(std::ostringstream& out, const char* key, double value, bool trailing_comma) {
    out << "  \"" << key << "\": " << std::fixed << std::setprecision(6) << value;
    if (trailing_comma) {
        out << ',';
    }
    out << '\n';
}

} // namespace

ValidationMetrics compute_metrics(const Raster& estimate, const Raster& reference) {
    ValidationMetrics metrics;
    if (!estimate.valid() || !reference.valid() ||
        estimate.width != reference.width || estimate.height != reference.height) {
        return metrics;
    }
    std::vector<double> errors;
    errors.reserve(estimate.values.size());
    double sum_sq = 0.0;
    double sum_abs = 0.0;
    double sum = 0.0;
    std::size_t within_1m = 0;
    std::size_t within_5m = 0;
    for (std::size_t i = 0; i < estimate.values.size(); ++i) {
        const float est = estimate.values[i];
        const float ref = reference.values[i];
        if (Raster::is_nodata(est) || Raster::is_nodata(ref)) {
            continue;
        }
        const double error = static_cast<double>(est) - static_cast<double>(ref);
        errors.push_back(error);
        sum_sq += error * error;
        sum_abs += std::abs(error);
        sum += error;
        metrics.max_abs = std::max(metrics.max_abs, std::abs(error));
        if (std::abs(error) <= 1.0) {
            ++within_1m;
        }
        if (std::abs(error) <= 5.0) {
            ++within_5m;
        }
    }
    metrics.sample_count = errors.size();
    if (errors.empty()) {
        return metrics;
    }
    const double n = static_cast<double>(errors.size());
    metrics.rmse = std::sqrt(sum_sq / n);
    metrics.mae = sum_abs / n;
    metrics.bias = sum / n;
    metrics.pct_within_1m = 100.0 * static_cast<double>(within_1m) / n;
    metrics.pct_within_5m = 100.0 * static_cast<double>(within_5m) / n;

    std::vector<double> centered = errors;
    const double median = median_of_sorted(centered);
    for (double& value : centered) {
        value = std::abs(value - median);
    }
    metrics.nmad = 1.4826 * median_of_sorted(centered);
    return metrics;
}

bool write_validation_json(
    const std::filesystem::path& path,
    const ValidationReport& report,
    std::string* error) {
    std::error_code ec;
    if (path.has_parent_path()) {
        std::filesystem::create_directories(path.parent_path(), ec);
        if (ec) {
            if (error != nullptr) {
                *error = "validation_json_mkdir_failed:" + ec.message();
            }
            return false;
        }
    }

    // Deterministic key order and formatting so identical runs produce
    // byte-identical reports.
    std::ostringstream out;
    out << "{\n";
    out << "  \"ok\": " << (report.ok ? "true" : "false") << ",\n";
    out << "  \"error\": \"" << report.error << "\",\n";
    out << "  \"reference\": \"" << report.reference_name << "\",\n";
    out << "  \"param_hash\": \"" << std::hex << std::setw(16) << std::setfill('0')
        << report.param_hash << std::dec << std::setfill(' ') << "\",\n";
    out << "  \"fused_raster_hash\": \"" << std::hex << std::setw(16) << std::setfill('0')
        << report.fused_raster_hash << std::dec << std::setfill(' ') << "\",\n";
    out << "  \"sample_count\": " << report.metrics.sample_count << ",\n";
    append_metric(out, "rmse_m", report.metrics.rmse, true);
    append_metric(out, "mae_m", report.metrics.mae, true);
    append_metric(out, "bias_m", report.metrics.bias, true);
    append_metric(out, "nmad_m", report.metrics.nmad, true);
    append_metric(out, "max_abs_m", report.metrics.max_abs, true);
    append_metric(out, "pct_within_1m", report.metrics.pct_within_1m, true);
    append_metric(out, "pct_within_5m", report.metrics.pct_within_5m, false);
    out << "}\n";

    std::ofstream stream(path, std::ios::binary | std::ios::trunc);
    if (!stream) {
        if (error != nullptr) {
            *error = "validation_json_open_failed";
        }
        return false;
    }
    stream << out.str();
    return stream.good();
}

} // namespace agbot::terrain
