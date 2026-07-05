#include "agbot_nav/Perception.hpp"

#include <cmath>
#include <cstdint>
#include <map>
#include <utility>

namespace agbot::nav {

namespace {

void push_labeled(PerceptionResult& result, const Vec3& point, bool obstacle) {
    const std::uint32_t label = obstacle ? kClassObstacle : kClassGround;
    result.labeled.points.push_back(point);
    result.labeled.classes.push_back(label);
    if (obstacle) {
        result.obstacles.points.push_back(point);
        result.obstacles.classes.push_back(label);
    }
}

} // namespace

HeightThresholdGroundSeg::HeightThresholdGroundSeg(const agbot::config::ParamTable& params) {
    ground_height_m_ = agbot::config::double_or(params, "ground_height_m", ground_height_m_);
    max_step_m_ = agbot::config::double_or(params, "max_step_m", max_step_m_);
}

PerceptionResult HeightThresholdGroundSeg::segment(const SensorFrame& frame) {
    PerceptionResult result;
    result.labeled.points.reserve(frame.cloud.size());
    result.labeled.classes.reserve(frame.cloud.size());
    for (const Vec3& point : frame.cloud.points) {
        push_labeled(result, point, point.y >= ground_height_m_ + max_step_m_);
    }
    return result;
}

GridStepGroundSeg::GridStepGroundSeg(const agbot::config::ParamTable& params) {
    cell_size_m_ = agbot::config::double_or(params, "cell_size_m", cell_size_m_);
    step_threshold_m_ = agbot::config::double_or(params, "step_threshold_m", step_threshold_m_);
    ground_height_m_ = agbot::config::double_or(params, "ground_height_m", ground_height_m_);
    ground_band_m_ = agbot::config::double_or(params, "ground_band_m", ground_band_m_);
}

PerceptionResult GridStepGroundSeg::segment(const SensorFrame& frame) {
    struct CellStats {
        double min_y = 0.0;
        double max_y = 0.0;
        bool initialized = false;
    };
    std::map<std::pair<std::int64_t, std::int64_t>, CellStats> cells;
    const double cell = cell_size_m_ > 1e-6 ? cell_size_m_ : 0.5;

    auto key_for = [&](const Vec3& point) {
        return std::make_pair(
            static_cast<std::int64_t>(std::floor(point.x / cell)),
            static_cast<std::int64_t>(std::floor(point.z / cell)));
    };

    for (const Vec3& point : frame.cloud.points) {
        CellStats& stats = cells[key_for(point)];
        if (!stats.initialized) {
            stats.min_y = point.y;
            stats.max_y = point.y;
            stats.initialized = true;
        } else {
            stats.min_y = std::min(stats.min_y, point.y);
            stats.max_y = std::max(stats.max_y, point.y);
        }
    }

    PerceptionResult result;
    result.labeled.points.reserve(frame.cloud.size());
    result.labeled.classes.reserve(frame.cloud.size());
    for (const Vec3& point : frame.cloud.points) {
        const CellStats& stats = cells[key_for(point)];
        const bool cell_has_step = (stats.max_y - stats.min_y) > step_threshold_m_;
        const bool cell_above_ground = stats.min_y > ground_height_m_ + ground_band_m_;
        bool obstacle = cell_above_ground;
        if (cell_has_step) {
            obstacle = point.y > stats.min_y + step_threshold_m_;
        }
        push_labeled(result, point, obstacle);
    }
    return result;
}

const PerceptionRegistry& default_perception_registry() {
    static const PerceptionRegistry registry = [] {
        PerceptionRegistry built;
        built.register_factory(
            "height_threshold",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<IPerception> {
                return std::make_unique<HeightThresholdGroundSeg>(params);
            });
        built.register_factory(
            "grid_step",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<IPerception> {
                return std::make_unique<GridStepGroundSeg>(params);
            });
        return built;
    }();
    return registry;
}

} // namespace agbot::nav
