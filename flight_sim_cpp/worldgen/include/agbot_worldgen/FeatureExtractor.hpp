#pragma once

#include "agbot_config/StrategyRegistry.hpp"
#include "agbot_worldgen/Feature.hpp"

#include <string>
#include <vector>

namespace agbot::worldgen {

// Strategy interface for feature extraction algorithms. Implementations must
// not throw across this boundary; failures are reported through the
// reason-coded ExtractionResult.
class FeatureExtractor {
public:
    virtual ~FeatureExtractor() = default;

    [[nodiscard]] virtual std::string id() const = 0;
    [[nodiscard]] virtual std::vector<FeatureClass> produces() const = 0;
    [[nodiscard]] virtual ExtractionResult extract(const ExtractionContext& context) const = 0;
};

// Process-wide extractor registry. Built-in extractors (vector_import) are
// registered on first access.
[[nodiscard]] agbot::config::StrategyRegistry<FeatureExtractor>& extractor_registry();

} // namespace agbot::worldgen
