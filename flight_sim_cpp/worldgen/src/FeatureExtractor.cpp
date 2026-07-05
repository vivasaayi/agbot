#include "agbot_worldgen/FeatureExtractor.hpp"

#include "agbot_worldgen/extractors/ClassicalIndex.hpp"
#include "agbot_worldgen/extractors/OnnxSemSeg.hpp"
#include "agbot_worldgen/extractors/RoadImport.hpp"
#include "agbot_worldgen/extractors/VectorImport.hpp"

#include <memory>

namespace agbot::worldgen {

agbot::config::StrategyRegistry<FeatureExtractor>& extractor_registry() {
    static agbot::config::StrategyRegistry<FeatureExtractor> registry = [] {
        agbot::config::StrategyRegistry<FeatureExtractor> built;
        built.register_factory(VectorImportExtractor::kId, [] {
            return std::unique_ptr<FeatureExtractor>(new VectorImportExtractor());
        });
        built.register_factory(ClassicalIndexExtractor::kId, [] {
            return std::unique_ptr<FeatureExtractor>(new ClassicalIndexExtractor());
        });
        built.register_factory(OnnxSemSegExtractor::kId, [] {
            return std::unique_ptr<FeatureExtractor>(new OnnxSemSegExtractor());
        });
        built.register_factory(RoadImportExtractor::kId, [] {
            return std::unique_ptr<FeatureExtractor>(new RoadImportExtractor());
        });
        return built;
    }();
    return registry;
}

} // namespace agbot::worldgen
