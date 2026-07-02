#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_config/StrategyRegistry.hpp"
#include "agbot_terrain/Raster.hpp"

#include <string>

namespace agbot::terrain {

// Reason-coded estimate result; no exceptions cross this boundary.
struct EstimateResult {
    bool ok = false;
    std::string error; // e.g. "onnx_runtime_unavailable", "dem_prior_missing"
    HeightField field;
};

// One elevation-producing algorithm (DEM fusion, learned mono-depth,
// synthetic detail, ...). Implementations are registered by name in the
// global estimator registry so pipelines are hot-swappable from TOML.
class ElevationEstimator {
public:
    virtual ~ElevationEstimator() = default;

    [[nodiscard]] virtual std::string name() const = 0;
    [[nodiscard]] virtual bool accepts(const ImageryBundle& bundle) const = 0;
    [[nodiscard]] virtual EstimateResult estimate(
        const ImageryBundle& bundle,
        const agbot::config::ParamTable& params) const = 0;
};

// Global registry with all built-in estimators self-registered:
//   "dem_fusion", "synthetic_detail", "mono_depth_onnx".
[[nodiscard]] agbot::config::StrategyRegistry<ElevationEstimator>& estimator_registry();

} // namespace agbot::terrain
