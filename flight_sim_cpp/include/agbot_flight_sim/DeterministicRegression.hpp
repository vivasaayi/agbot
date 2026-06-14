#pragma once

#include "agbot_flight_sim/DeterministicRunner.hpp"
#include "agbot_flight_sim/Mission.hpp"
#include "agbot_flight_sim/TraceDiff.hpp"

#include <filesystem>
#include <string>
#include <vector>

namespace agbot::flight_sim {

struct GoldenRegressionCase {
    std::string case_id;
    Mission mission;
    RunConfig config;
    std::string golden_trace_jsonl;
    std::string golden_manifest_json;
    std::string golden_contract_version;
    std::string golden_output_hash;
    std::string golden_manifest_hash;
};

struct GoldenRegressionCaseResult {
    std::string case_id;
    bool passed = false;
    std::string code;
    std::string message;
    std::string expected_contract_version;
    std::string actual_contract_version;
    std::string expected_output_hash;
    std::string actual_output_hash;
    std::string expected_manifest_hash;
    std::string actual_manifest_hash;
    TraceDiffResult trace_diff;

    [[nodiscard]] std::string to_json() const;
};

struct GoldenRegressionSuiteResult {
    std::string environment;
    std::vector<GoldenRegressionCaseResult> cases;

    [[nodiscard]] bool passed() const;
    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] std::filesystem::path default_golden_regression_dir();
[[nodiscard]] std::string deterministic_regression_environment();
[[nodiscard]] std::vector<std::string> reference_regression_case_ids();
[[nodiscard]] std::vector<GoldenRegressionCase> load_golden_regression_cases(
    const std::filesystem::path& golden_dir = default_golden_regression_dir());
[[nodiscard]] GoldenRegressionCaseResult run_golden_regression_case(
    const GoldenRegressionCase& regression_case);
[[nodiscard]] GoldenRegressionSuiteResult run_golden_regression_suite(
    const std::vector<GoldenRegressionCase>& cases);
[[nodiscard]] GoldenRegressionSuiteResult run_golden_regression_suite(
    const std::filesystem::path& golden_dir = default_golden_regression_dir());

} // namespace agbot::flight_sim
