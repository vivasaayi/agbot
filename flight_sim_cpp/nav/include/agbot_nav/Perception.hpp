#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_nav/Sensing.hpp"
#include "agbot_vehicles/ParamRegistry.hpp"

#include <memory>
#include <string>

namespace agbot::nav {

struct PerceptionResult {
    // Full labeled cloud (classes rewritten to kClassGround / kClassObstacle).
    PointCloud labeled;
    // Obstacle-only subset for mapping.
    PointCloud obstacles;
};

class IPerception {
public:
    virtual ~IPerception() = default;
    virtual PerceptionResult segment(const SensorFrame& frame) = 0;
    [[nodiscard]] virtual std::string name() const = 0;
};

// Points with y < ground_height_m + max_step_m are ground, everything else is
// an obstacle. Params: ground_height_m, max_step_m.
class HeightThresholdGroundSeg final : public IPerception {
public:
    HeightThresholdGroundSeg() = default;
    explicit HeightThresholdGroundSeg(const agbot::config::ParamTable& params);

    PerceptionResult segment(const SensorFrame& frame) override;
    [[nodiscard]] std::string name() const override { return "height_threshold"; }

private:
    double ground_height_m_ = 0.0;
    double max_step_m_ = 0.15;
};

// Grid-based segmentation: per XZ cell, compute min/max y. A cell is an
// obstacle cell when its vertical extent exceeds step_threshold_m or its
// minimum sits above the ground band. Params: cell_size_m, step_threshold_m,
// ground_height_m, ground_band_m.
class GridStepGroundSeg final : public IPerception {
public:
    GridStepGroundSeg() = default;
    explicit GridStepGroundSeg(const agbot::config::ParamTable& params);

    PerceptionResult segment(const SensorFrame& frame) override;
    [[nodiscard]] std::string name() const override { return "grid_step"; }

private:
    double cell_size_m_ = 0.5;
    double step_threshold_m_ = 0.15;
    double ground_height_m_ = 0.0;
    double ground_band_m_ = 0.3;
};

using PerceptionRegistry = agbot::vehicles::ParamRegistry<IPerception>;
[[nodiscard]] const PerceptionRegistry& default_perception_registry();

} // namespace agbot::nav
