#include "agbot_config/Params.hpp"
#include "agbot_config/StrategyRegistry.hpp"
#include "agbot_config/Toml.hpp"

#include <cmath>
#include <iostream>
#include <memory>
#include <string>

namespace {

int failures = 0;

void expect(bool condition, const std::string& label) {
    if (condition) {
        std::cout << "PASS " << label << "\n";
    } else {
        std::cout << "FAIL " << label << "\n";
        ++failures;
    }
}

const char* kSampleConfig = R"toml(
# world simulator pipeline config
[pipeline]
target_gsd_m = 10.0
vertical_datum = "egm2008"
max_tiles = 64
enabled = true
aoi = { min_lat = 40.70, min_lon = -74.02, max_lat = 40.82, max_lon = -73.93 }

[[layer]]
algorithm = "dem_fusion"
weight = 1.0
  [layer.params]
  source = "terrarium"
  resample = "bicubic"

[[layer]]
algorithm = "mono_depth"
weight = 0.5
  [layer.params]
  patch_size = 518
  scales = [1.0, 0.5, 0.25]
)toml";

struct FakeStrategy {
    virtual ~FakeStrategy() = default;
    virtual std::string name() const = 0;
};

struct AlphaStrategy : FakeStrategy {
    std::string name() const override { return "alpha"; }
};

void test_toml_parsing() {
    const auto result = agbot::config::parse_toml(kSampleConfig);
    expect(result.ok, "toml parses");
    if (!result.ok) {
        std::cout << "  error: " << result.error << "\n";
        return;
    }
    const auto* pipeline = agbot::config::find_table(result.root, "pipeline");
    expect(pipeline != nullptr, "pipeline table exists");
    if (pipeline == nullptr) {
        return;
    }
    expect(std::abs(agbot::config::double_or(*pipeline, "target_gsd_m", 0.0) - 10.0) < 1e-9,
           "float value");
    expect(agbot::config::integer_or(*pipeline, "max_tiles", 0) == 64, "integer value");
    expect(agbot::config::bool_or(*pipeline, "enabled", false), "bool value");
    expect(agbot::config::string_or(*pipeline, "vertical_datum", "") == "egm2008", "string value");

    const auto* aoi = agbot::config::find_table(*pipeline, "aoi");
    expect(aoi != nullptr && std::abs(agbot::config::double_or(*aoi, "min_lat", 0.0) - 40.70) < 1e-9,
           "inline table");

    const auto* layers = agbot::config::find_array(result.root, "layer");
    expect(layers != nullptr && layers->size() == 2, "array of tables has two layers");
    if (layers != nullptr && layers->size() == 2) {
        const auto& first = (*layers)[0].as_table();
        expect(agbot::config::string_or(first, "algorithm", "") == "dem_fusion",
               "first layer algorithm");
        const auto* params = agbot::config::find_table(first, "params");
        expect(params != nullptr &&
               agbot::config::string_or(*params, "resample", "") == "bicubic",
               "nested layer params");
        const auto& second = (*layers)[1].as_table();
        const auto* second_params = agbot::config::find_table(second, "params");
        const auto* scales =
            second_params ? agbot::config::find_array(*second_params, "scales") : nullptr;
        expect(scales != nullptr && scales->size() == 3, "scalar array");
    }
}

void test_toml_errors() {
    const auto bad = agbot::config::parse_toml("key = = broken");
    expect(!bad.ok, "invalid toml rejected");
    expect(bad.error.find("line 1") != std::string::npos, "error carries line number");
}

void test_param_hash_stability() {
    const auto first = agbot::config::parse_toml(kSampleConfig);
    const auto second = agbot::config::parse_toml(kSampleConfig);
    expect(first.ok && second.ok, "hash inputs parse");
    expect(agbot::config::param_hash(first.root) == agbot::config::param_hash(second.root),
           "identical configs hash identically");

    auto mutated = first.root;
    mutated["pipeline"].as_table()["target_gsd_m"] = agbot::config::ParamValue(11.0);
    expect(agbot::config::param_hash(first.root) != agbot::config::param_hash(mutated),
           "changed param changes hash");
}

void test_strategy_registry() {
    agbot::config::StrategyRegistry<FakeStrategy> registry;
    registry.register_factory("alpha", [] { return std::make_unique<AlphaStrategy>(); });
    expect(registry.contains("alpha"), "registry contains registered strategy");
    expect(!registry.contains("beta"), "registry rejects unknown strategy");
    const auto strategy = registry.create("alpha");
    expect(strategy != nullptr && strategy->name() == "alpha", "registry creates strategy");
    expect(registry.create("missing") == nullptr, "unknown create returns nullptr");
}

} // namespace

int main() {
    test_toml_parsing();
    test_toml_errors();
    test_param_hash_stability();
    test_strategy_registry();
    if (failures != 0) {
        std::cout << failures << " failing checks\n";
        return 1;
    }
    std::cout << "all config tests passed\n";
    return 0;
}
