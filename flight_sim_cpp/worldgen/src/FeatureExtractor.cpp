#include "agbot_worldgen/FeatureExtractor.hpp"

#include "agbot_worldgen/extractors/VectorImport.hpp"

#include <memory>

namespace agbot::worldgen {

agbot::config::StrategyRegistry<FeatureExtractor>& extractor_registry() {
    static agbot::config::StrategyRegistry<FeatureExtractor> registry = [] {
        agbot::config::StrategyRegistry<FeatureExtractor> built;
        built.register_factory(VectorImportExtractor::kId, [] {
            return std::unique_ptr<FeatureExtractor>(new VectorImportExtractor());
        });
        return built;
    }();
    return registry;
}

} // namespace agbot::worldgen
