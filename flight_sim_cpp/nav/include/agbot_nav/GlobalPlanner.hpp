#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_nav/NavTypes.hpp"
#include "agbot_vehicles/ParamRegistry.hpp"

#include <memory>
#include <string>

namespace agbot::nav {

struct PlanResult {
    bool ok = false;
    std::string reason;
    Path path;
};

class IGlobalPlanner {
public:
    virtual ~IGlobalPlanner() = default;
    virtual PlanResult plan(const Costmap& costmap, const Vec3& start, const Vec3& goal) = 0;
    [[nodiscard]] virtual std::string name() const = 0;
};

// 8-connected A* over the costmap with cost-aware traversal and deterministic
// tie-breaking (f, then g, then cell index). Unknown cells are traversable at
// unknown_cost. Optional string-pulling smoothing shortcuts waypoints along
// collision-free straight segments; shortcuts must stay below
// smooth_max_cost so smoothing cannot hug obstacles. Params:
// lethal_threshold, heuristic_weight, cost_weight, unknown_cost, smooth,
// smooth_max_cost.
class AStarPlanner final : public IGlobalPlanner {
public:
    AStarPlanner() = default;
    explicit AStarPlanner(const agbot::config::ParamTable& params);

    PlanResult plan(const Costmap& costmap, const Vec3& start, const Vec3& goal) override;
    [[nodiscard]] std::string name() const override { return "astar"; }

private:
    std::uint8_t lethal_threshold_ = 200;
    double heuristic_weight_ = 1.0;
    double cost_weight_ = 4.0;
    std::uint8_t unknown_cost_ = 0;
    bool smooth_ = true;
    std::uint8_t smooth_max_cost_ = 120;
};

// True if the straight segment between two world points stays below
// lethal_threshold on the costmap (unknown treated as unknown_cost).
[[nodiscard]] bool segment_is_traversable(
    const Costmap& costmap,
    const Vec3& from,
    const Vec3& to,
    std::uint8_t lethal_threshold,
    std::uint8_t unknown_cost);

using GlobalPlannerRegistry = agbot::vehicles::ParamRegistry<IGlobalPlanner>;
[[nodiscard]] const GlobalPlannerRegistry& default_global_planner_registry();

} // namespace agbot::nav
