#include "agbot_flight_sim/DeterministicRegression.hpp"

#include "agbot_flight_sim/MissionLoader.hpp"
#include "agbot_flight_sim/TwinContractV1.hpp"

#include <algorithm>
#include <cctype>
#include <cstdint>
#include <fstream>
#include <optional>
#include <sstream>
#include <stdexcept>
#include <string_view>
#include <utility>

namespace agbot::flight_sim {
namespace {

std::string read_file(const std::filesystem::path& path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        throw std::runtime_error("unable to open golden regression fixture: " + path.string());
    }
    std::ostringstream buffer;
    buffer << file.rdbuf();
    return buffer.str();
}

std::string trim_trailing_newlines(std::string value) {
    while (!value.empty() && (value.back() == '\n' || value.back() == '\r')) {
        value.pop_back();
    }
    return value;
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

std::optional<std::string> scalar_for_key(std::string_view text, std::string_view key) {
    const std::string token = "\"" + std::string(key) + "\":";
    const std::size_t key_position = text.find(token);
    if (key_position == std::string_view::npos) {
        return std::nullopt;
    }

    std::size_t value_start = key_position + token.size();
    while (value_start < text.size() && std::isspace(static_cast<unsigned char>(text[value_start])) != 0) {
        ++value_start;
    }
    if (value_start >= text.size()) {
        return std::nullopt;
    }

    if (text[value_start] == '"') {
        std::string value;
        bool escaped = false;
        for (std::size_t index = value_start + 1; index < text.size(); ++index) {
            const char c = text[index];
            if (escaped) {
                value.push_back(c);
                escaped = false;
            } else if (c == '\\') {
                escaped = true;
            } else if (c == '"') {
                return value;
            } else {
                value.push_back(c);
            }
        }
        return std::nullopt;
    }

    std::size_t value_end = value_start;
    while (value_end < text.size() && text[value_end] != ',' && text[value_end] != '}') {
        ++value_end;
    }
    return std::string(text.substr(value_start, value_end - value_start));
}

std::string required_scalar(std::string_view text, std::string_view key) {
    if (const auto value = scalar_for_key(text, key)) {
        return *value;
    }
    throw std::runtime_error("golden manifest is missing key: " + std::string(key));
}

bool manifest_lidar_enabled(std::string_view manifest_json) {
    return manifest_json.find("\"lidar_config\":{\"enabled\":true") != std::string_view::npos;
}

RunConfig config_from_manifest(std::string_view manifest_json) {
    RunConfig config;
    config.seed = static_cast<std::uint64_t>(std::stoull(required_scalar(manifest_json, "seed")));
    config.timestep_s = std::stod(required_scalar(manifest_json, "timestep_s"));
    config.record_interval_s = std::stod(required_scalar(manifest_json, "record_interval_s"));
    config.lidar.enabled = manifest_lidar_enabled(manifest_json);
    return config;
}

GoldenRegressionCase load_case(const std::filesystem::path& golden_dir, const std::string& case_id) {
    const std::string mission_json = read_file(golden_dir / (case_id + ".mission.json"));
    const std::string manifest_json =
        trim_trailing_newlines(read_file(golden_dir / (case_id + ".trace.manifest.json")));

    GoldenRegressionCase regression_case;
    regression_case.case_id = case_id;
    regression_case.mission = MissionLoader::load_from_text(mission_json);
    regression_case.config = config_from_manifest(manifest_json);
    regression_case.golden_trace_jsonl = read_file(golden_dir / (case_id + ".trace.jsonl"));
    regression_case.golden_manifest_json = manifest_json;
    regression_case.golden_contract_version = required_scalar(manifest_json, "contract_version");
    regression_case.golden_output_hash = required_scalar(manifest_json, "output_hash");
    regression_case.golden_manifest_hash = sha256_hex(manifest_json);
    return regression_case;
}

GoldenRegressionCaseResult fail_result(
    const GoldenRegressionCase& regression_case,
    const RunResult& run,
    std::string code,
    std::string message) {
    GoldenRegressionCaseResult result;
    result.case_id = regression_case.case_id;
    result.passed = false;
    result.code = std::move(code);
    result.message = std::move(message);
    result.expected_contract_version = regression_case.golden_contract_version;
    result.actual_contract_version = run.manifest.contract_version;
    result.expected_output_hash = regression_case.golden_output_hash;
    result.actual_output_hash = run.manifest.output_hash;
    result.expected_manifest_hash = regression_case.golden_manifest_hash;
    result.actual_manifest_hash = sha256_hex(run.manifest.to_json());
    return result;
}

} // namespace

std::filesystem::path default_golden_regression_dir() {
    return std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR) / "tests" / "golden";
}

std::string deterministic_regression_environment() {
    std::ostringstream environment;
#if defined(__clang__)
    environment << "compiler=clang-" << __clang_major__ << "." << __clang_minor__ << "." << __clang_patchlevel__;
#elif defined(__GNUC__)
    environment << "compiler=gcc-" << __GNUC__ << "." << __GNUC_MINOR__ << "." << __GNUC_PATCHLEVEL__;
#else
    environment << "compiler=unknown";
#endif
#if defined(__APPLE__)
    environment << ";platform=apple";
#elif defined(__linux__)
    environment << ";platform=linux";
#elif defined(_WIN32)
    environment << ";platform=windows";
#else
    environment << ";platform=unknown";
#endif
#if defined(NDEBUG)
    environment << ";build=release";
#else
    environment << ";build=debug";
#endif
    return environment.str();
}

std::vector<std::string> reference_regression_case_ids() {
    return {
        "reference_takeoff_land",
        "reference_goto",
        "reference_orbit_loiter",
    };
}

std::vector<GoldenRegressionCase> load_golden_regression_cases(const std::filesystem::path& golden_dir) {
    std::vector<GoldenRegressionCase> cases;
    for (const std::string& case_id : reference_regression_case_ids()) {
        cases.push_back(load_case(golden_dir, case_id));
    }
    return cases;
}

GoldenRegressionCaseResult run_golden_regression_case(const GoldenRegressionCase& regression_case) {
    RunResult run = run_deterministic(regression_case.mission, regression_case.config);

    if (!is_compatible_contract_version(kTwinContractVersion, regression_case.golden_contract_version)) {
        return fail_result(
            regression_case,
            run,
            "incompatible_contract_version",
            "incompatible contract version: golden=" + regression_case.golden_contract_version
                + " current=" + kTwinContractVersion);
    }

    const TraceDiffResult diff = diff_trace_text(regression_case.golden_trace_jsonl, run.trace_jsonl);
    if (!diff.identical) {
        GoldenRegressionCaseResult result = fail_result(
            regression_case,
            run,
            "trace_diverged",
            diff.message);
        result.trace_diff = diff;
        return result;
    }

    if (run.manifest.output_hash != regression_case.golden_output_hash) {
        return fail_result(
            regression_case,
            run,
            "manifest_output_hash_mismatch",
            "manifest output_hash mismatch for " + regression_case.case_id);
    }

    const std::string actual_manifest_hash = sha256_hex(run.manifest.to_json());
    if (actual_manifest_hash != regression_case.golden_manifest_hash) {
        GoldenRegressionCaseResult result = fail_result(
            regression_case,
            run,
            "manifest_hash_mismatch",
            "scenario manifest hash mismatch for " + regression_case.case_id);
        result.actual_manifest_hash = actual_manifest_hash;
        return result;
    }

    GoldenRegressionCaseResult result;
    result.case_id = regression_case.case_id;
    result.passed = true;
    result.code = "passed";
    result.message = "golden trace and scenario manifest hashes matched";
    result.expected_contract_version = regression_case.golden_contract_version;
    result.actual_contract_version = run.manifest.contract_version;
    result.expected_output_hash = regression_case.golden_output_hash;
    result.actual_output_hash = run.manifest.output_hash;
    result.expected_manifest_hash = regression_case.golden_manifest_hash;
    result.actual_manifest_hash = actual_manifest_hash;
    result.trace_diff = {};
    return result;
}

GoldenRegressionSuiteResult run_golden_regression_suite(const std::vector<GoldenRegressionCase>& cases) {
    GoldenRegressionSuiteResult suite;
    suite.environment = deterministic_regression_environment();
    for (const GoldenRegressionCase& regression_case : cases) {
        suite.cases.push_back(run_golden_regression_case(regression_case));
    }
    return suite;
}

GoldenRegressionSuiteResult run_golden_regression_suite(const std::filesystem::path& golden_dir) {
    return run_golden_regression_suite(load_golden_regression_cases(golden_dir));
}

bool GoldenRegressionSuiteResult::passed() const {
    return std::all_of(cases.begin(), cases.end(), [](const GoldenRegressionCaseResult& result) {
        return result.passed;
    });
}

std::string GoldenRegressionCaseResult::to_json() const {
    std::ostringstream output;
    output << "{\"case_id\":\"" << escape_json(case_id) << "\""
           << ",\"status\":\"" << (passed ? "pass" : "fail") << "\""
           << ",\"code\":\"" << escape_json(code) << "\""
           << ",\"message\":\"" << escape_json(message) << "\""
           << ",\"expected_contract_version\":\"" << escape_json(expected_contract_version) << "\""
           << ",\"actual_contract_version\":\"" << escape_json(actual_contract_version) << "\""
           << ",\"expected_output_hash\":\"" << escape_json(expected_output_hash) << "\""
           << ",\"actual_output_hash\":\"" << escape_json(actual_output_hash) << "\""
           << ",\"expected_manifest_hash\":\"" << escape_json(expected_manifest_hash) << "\""
           << ",\"actual_manifest_hash\":\"" << escape_json(actual_manifest_hash) << "\"";
    if (!trace_diff.identical) {
        output << ",\"trace_diff\":{\"step_index\":" << trace_diff.step_index
               << ",\"field_path\":\"" << escape_json(trace_diff.field_path) << "\""
               << ",\"left_value\":\"" << escape_json(trace_diff.left_value) << "\""
               << ",\"right_value\":\"" << escape_json(trace_diff.right_value) << "\"}";
    }
    output << "}";
    return output.str();
}

std::string GoldenRegressionSuiteResult::to_json() const {
    std::ostringstream output;
    output << "{\"status\":\"" << (passed() ? "pass" : "fail") << "\""
           << ",\"environment\":\"" << escape_json(environment) << "\""
           << ",\"case_count\":" << cases.size()
           << ",\"cases\":[";
    for (std::size_t index = 0; index < cases.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        output << cases[index].to_json();
    }
    output << "]}";
    return output.str();
}

} // namespace agbot::flight_sim
