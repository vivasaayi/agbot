#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_nav/GlobalPlanner.hpp"
#include "agbot_nav/NavTypes.hpp"

#include <cstdint>
#include <string>

namespace agbot::nav {

// Kinodynamic Hybrid-A* global planner over (x, z, heading). Expansions are
// bicycle motion primitives: constant-curvature arcs of arc_length_m at steer
// fractions {-1, -1/2, 0, +1/2, +1} of the maximum (curvature = fraction /
// min_turn_radius_m), forward and optionally reverse. States deduplicate on
// (cell, heading bin) with n_headings bins. Edge cost = arc length
// (x reverse_penalty when backing up) + steer_change_penalty * |steer
// fraction change| + cost_weight * mean costmap cost along the arc; cells at
// or above lethal_threshold block. Heuristic = heuristic_weight * euclidean
// distance. Every analytic_expansion_period expansions a Dubins shot to the
// goal pose is attempted (shared Dubins2D core) and accepted when collision
// free, which bakes the exact goal heading into the path tail. Deterministic:
// ties break on (f, g, insertion order). Poses outside the costmap are
// rejected, unknown cells traverse at unknown_cost.
//
// Params: min_turn_radius_m, arc_length_m, n_headings, allow_reverse,
// reverse_penalty, steer_change_penalty, lethal_threshold, heuristic_weight,
// cost_weight, unknown_cost, analytic_expansion_period, goal_xy_tolerance_m,
// goal_yaw_tolerance_rad, max_expansions.
class HybridAStarPlanner final : public IGlobalPlanner {
public:
    HybridAStarPlanner() = default;
    explicit HybridAStarPlanner(const agbot::config::ParamTable& params);

    // IGlobalPlanner entry point: start/goal headings default to the
    // start->goal bearing (the pipeline interface carries no orientation).
    PlanResult plan(const Costmap& costmap, const Vec3& start, const Vec3& goal) override;

    // Pose-to-pose planning with explicit headings; the goal heading must be
    // met within goal_yaw_tolerance_rad.
    [[nodiscard]] PlanResult plan_poses(
        const Costmap& costmap,
        const Pose2D& start,
        const Pose2D& goal);

    [[nodiscard]] std::string name() const override { return "hybrid_astar"; }

    [[nodiscard]] double min_turn_radius_m() const { return min_turn_radius_m_; }

private:
    double min_turn_radius_m_ = 1.5;
    double arc_length_m_ = 1.0;
    int n_headings_ = 24;
    bool allow_reverse_ = false;
    double reverse_penalty_ = 2.0;
    double steer_change_penalty_ = 0.5;
    std::uint8_t lethal_threshold_ = 200;
    double heuristic_weight_ = 1.0;
    double cost_weight_ = 2.0;
    std::uint8_t unknown_cost_ = 0;
    int analytic_expansion_period_ = 8;
    double goal_xy_tolerance_m_ = 1.0;
    double goal_yaw_tolerance_rad_ = 0.35;
    int max_expansions_ = 200000;
};

} // namespace agbot::nav
