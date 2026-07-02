#include "agbot_nav/HybridAStarPlanner.hpp"

#include "agbot_nav/Dubins2D.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <queue>
#include <vector>

namespace agbot::nav {

namespace {

using dubins2d::angle_diff;
using dubins2d::mod2pi;
using dubins2d::PlanarPose;

// Steer fractions of the maximum curvature used as motion primitives.
constexpr double kSteerFractions[5] = {-1.0, -0.5, 0.0, 0.5, 1.0};
constexpr int kSteerCount = 5;

struct SearchNode {
    double x = 0.0;
    double z = 0.0;
    double heading = 0.0;
    double g = std::numeric_limits<double>::infinity();
    int parent = -1;      // index into the node arena
    int steer_index = 2;  // primitive that produced this node (2 = straight)
    int direction = 1;    // +1 forward, -1 reverse
};

struct OpenEntry {
    double f = 0.0;
    double g = 0.0;
    std::uint64_t order = 0; // insertion counter for deterministic ties
    int node = -1;

    bool operator>(const OpenEntry& other) const {
        if (f != other.f) {
            return f > other.f;
        }
        if (g != other.g) {
            return g > other.g;
        }
        return order > other.order;
    }
};

// Pose after traveling signed arc length s at curvature kappa (heading rate
// per unit arc length; left positive).
PlanarPose propagate_arc(const PlanarPose& pose, double kappa, double s) {
    PlanarPose out = pose;
    if (std::abs(kappa) < 1e-12) {
        out.x += s * std::cos(pose.heading);
        out.z += s * std::sin(pose.heading);
        return out;
    }
    out.heading = pose.heading + kappa * s;
    out.x += (std::sin(out.heading) - std::sin(pose.heading)) / kappa;
    out.z -= (std::cos(out.heading) - std::cos(pose.heading)) / kappa;
    return out;
}

} // namespace

HybridAStarPlanner::HybridAStarPlanner(const agbot::config::ParamTable& params) {
    using agbot::config::bool_or;
    using agbot::config::double_or;
    using agbot::config::integer_or;
    min_turn_radius_m_ = double_or(params, "min_turn_radius_m", min_turn_radius_m_);
    arc_length_m_ = double_or(params, "arc_length_m", arc_length_m_);
    n_headings_ = static_cast<int>(std::clamp<std::int64_t>(
        integer_or(params, "n_headings", n_headings_), 4, 720));
    allow_reverse_ = bool_or(params, "allow_reverse", allow_reverse_);
    reverse_penalty_ = double_or(params, "reverse_penalty", reverse_penalty_);
    steer_change_penalty_ = double_or(params, "steer_change_penalty", steer_change_penalty_);
    lethal_threshold_ = static_cast<std::uint8_t>(std::clamp<std::int64_t>(
        integer_or(params, "lethal_threshold", lethal_threshold_), 1, 254));
    heuristic_weight_ = double_or(params, "heuristic_weight", heuristic_weight_);
    cost_weight_ = double_or(params, "cost_weight", cost_weight_);
    unknown_cost_ = static_cast<std::uint8_t>(std::clamp<std::int64_t>(
        integer_or(params, "unknown_cost", unknown_cost_), 0, 254));
    analytic_expansion_period_ = static_cast<int>(std::clamp<std::int64_t>(
        integer_or(params, "analytic_expansion_period", analytic_expansion_period_), 1, 100000));
    goal_xy_tolerance_m_ = double_or(params, "goal_xy_tolerance_m", goal_xy_tolerance_m_);
    goal_yaw_tolerance_rad_ =
        double_or(params, "goal_yaw_tolerance_rad", goal_yaw_tolerance_rad_);
    max_expansions_ = static_cast<int>(std::clamp<std::int64_t>(
        integer_or(params, "max_expansions", max_expansions_), 100, 10000000));
}

PlanResult HybridAStarPlanner::plan(const Costmap& costmap, const Vec3& start, const Vec3& goal) {
    const double bearing = std::atan2(goal.z - start.z, goal.x - start.x);
    return plan_poses(costmap, {start.x, start.z, bearing}, {goal.x, goal.z, bearing});
}

PlanResult HybridAStarPlanner::plan_poses(
    const Costmap& costmap,
    const Pose2D& start,
    const Pose2D& goal) {
    PlanResult result;
    if (costmap.width <= 0 || costmap.height <= 0) {
        result.reason = "empty_costmap";
        return result;
    }
    if (!(min_turn_radius_m_ > 0.0) || !(arc_length_m_ > 0.0)) {
        result.reason = "invalid_planner_parameters";
        return result;
    }

    // Cost lookup: outside the costmap is rejected (keeps the search finite),
    // unknown traverses at unknown_cost.
    const auto effective_cost = [&](double wx, double wz, std::uint8_t& cost) -> bool {
        int cx = 0;
        int cz = 0;
        if (!costmap.world_to_cell(wx, wz, cx, cz)) {
            return false;
        }
        const std::uint8_t raw = costmap.at(cx, cz);
        cost = (raw == OccupancyGrid::kUnknown) ? unknown_cost_ : raw;
        return true;
    };

    std::uint8_t start_cost = 0;
    if (!effective_cost(start.x, start.z, start_cost)) {
        result.reason = "start_outside_costmap";
        return result;
    }
    std::uint8_t goal_cost = 0;
    if (!effective_cost(goal.x, goal.z, goal_cost)) {
        result.reason = "goal_outside_costmap";
        return result;
    }
    if (goal_cost >= lethal_threshold_) {
        result.reason = "goal_in_lethal_cell";
        return result;
    }

    // Collision-check and average traversal cost along one primitive arc.
    // Returns false when a sample is lethal or leaves the costmap.
    const double check_step =
        std::max(1e-3, std::min(costmap.resolution_m * 0.5, arc_length_m_ * 0.25));
    const auto arc_is_free = [&](const PlanarPose& from, double kappa, double signed_s,
                                 double& mean_cost) -> bool {
        const int samples =
            std::max(1, static_cast<int>(std::ceil(std::abs(signed_s) / check_step)));
        double total = 0.0;
        for (int i = 1; i <= samples; ++i) {
            const double s = signed_s * static_cast<double>(i) / static_cast<double>(samples);
            const PlanarPose sample = propagate_arc(from, kappa, s);
            std::uint8_t cost = 0;
            if (!effective_cost(sample.x, sample.z, cost) || cost >= lethal_threshold_) {
                return false;
            }
            total += static_cast<double>(cost);
        }
        mean_cost = total / static_cast<double>(samples);
        return true;
    };

    const auto heading_bin = [&](double heading) -> int {
        const double normalized = mod2pi(heading);
        const int bin = static_cast<int>(std::floor(
            normalized * static_cast<double>(n_headings_) / dubins2d::kTwoPi));
        return std::clamp(bin, 0, n_headings_ - 1);
    };
    const auto state_key = [&](double wx, double wz, double heading, std::size_t& key) -> bool {
        int cx = 0;
        int cz = 0;
        if (!costmap.world_to_cell(wx, wz, cx, cz)) {
            return false;
        }
        key = (costmap.index(cx, cz) * static_cast<std::size_t>(n_headings_))
            + static_cast<std::size_t>(heading_bin(heading));
        return true;
    };

    const auto heuristic = [&](double wx, double wz) {
        const double dx = goal.x - wx;
        const double dz = goal.z - wz;
        return heuristic_weight_ * std::sqrt(dx * dx + dz * dz);
    };

    const std::size_t key_count = static_cast<std::size_t>(costmap.width)
        * static_cast<std::size_t>(costmap.height) * static_cast<std::size_t>(n_headings_);
    std::vector<double> best_g(key_count, std::numeric_limits<double>::infinity());
    std::vector<SearchNode> arena;
    arena.reserve(4096);
    std::priority_queue<OpenEntry, std::vector<OpenEntry>, std::greater<OpenEntry>> open;
    std::uint64_t push_order = 0;

    {
        SearchNode root;
        root.x = start.x;
        root.z = start.z;
        root.heading = start.yaw;
        root.g = 0.0;
        arena.push_back(root);
        std::size_t key = 0;
        if (state_key(start.x, start.z, start.yaw, key)) {
            best_g[key] = 0.0;
        }
        open.push({heuristic(start.x, start.z), 0.0, push_order++, 0});
    }

    const PlanarPose goal_pose{goal.x, goal.z, goal.yaw};
    const double sample_spacing =
        std::max(1e-3, std::min(costmap.resolution_m, arc_length_m_ * 0.5));

    // Rebuild the path root->node by replaying each node's producing
    // primitive at sample_spacing resolution.
    const auto reconstruct = [&](int node_index) {
        std::vector<int> chain;
        for (int i = node_index; i >= 0; i = arena[static_cast<std::size_t>(i)].parent) {
            chain.push_back(i);
        }
        std::reverse(chain.begin(), chain.end());
        Path path;
        path.points.push_back({start.x, 0.0, start.z});
        for (std::size_t c = 1; c < chain.size(); ++c) {
            const SearchNode& parent = arena[static_cast<std::size_t>(
                arena[static_cast<std::size_t>(chain[c])].parent)];
            const SearchNode& node = arena[static_cast<std::size_t>(chain[c])];
            const double kappa = kSteerFractions[node.steer_index] / min_turn_radius_m_;
            const double signed_s = static_cast<double>(node.direction) * arc_length_m_;
            const PlanarPose from{parent.x, parent.z, parent.heading};
            const int samples = std::max(
                1, static_cast<int>(std::ceil(arc_length_m_ / sample_spacing)));
            for (int i = 1; i <= samples; ++i) {
                const double s =
                    signed_s * static_cast<double>(i) / static_cast<double>(samples);
                const PlanarPose pose = propagate_arc(from, kappa, s);
                path.points.push_back({pose.x, 0.0, pose.z});
            }
        }
        return path;
    };

    // Dubins shot from a pose to the exact goal pose; appended when the whole
    // sampled arc chain stays collision free.
    const auto analytic_expansion = [&](const PlanarPose& from, Path& tail) -> bool {
        const dubins2d::ShortestPath shot =
            dubins2d::shortest_path(from, goal_pose, min_turn_radius_m_);
        if (!shot.ok) {
            return false;
        }
        const std::vector<dubins2d::Segment> segments(
            shot.segments.begin(), shot.segments.end());
        const int samples = std::max(
            1, static_cast<int>(std::ceil(shot.length_m / sample_spacing)));
        Path candidate;
        for (int i = 1; i <= samples; ++i) {
            const double s =
                shot.length_m * static_cast<double>(i) / static_cast<double>(samples);
            const PlanarPose pose =
                dubins2d::pose_at(segments, from, s, min_turn_radius_m_);
            std::uint8_t cost = 0;
            if (!effective_cost(pose.x, pose.z, cost) || cost >= lethal_threshold_) {
                return false;
            }
            candidate.points.push_back({pose.x, 0.0, pose.z});
        }
        tail = candidate;
        return true;
    };

    const int direction_count = allow_reverse_ ? 2 : 1;
    int expansions = 0;
    while (!open.empty()) {
        const OpenEntry entry = open.top();
        open.pop();
        const SearchNode current = arena[static_cast<std::size_t>(entry.node)];
        if (entry.g > current.g + 1e-12) {
            continue; // stale queue entry
        }
        if (++expansions > max_expansions_) {
            result.reason = "search_exhausted";
            return result;
        }

        // Direct goal test on the continuous state.
        const double goal_dx = goal.x - current.x;
        const double goal_dz = goal.z - current.z;
        if (std::sqrt(goal_dx * goal_dx + goal_dz * goal_dz) <= goal_xy_tolerance_m_
            && std::abs(angle_diff(current.heading, goal.yaw)) <= goal_yaw_tolerance_rad_) {
            result.path = reconstruct(entry.node);
            result.ok = true;
            return result;
        }

        // Periodic analytic expansion: exact goal pose via a Dubins shot.
        if (expansions % analytic_expansion_period_ == 0) {
            Path tail;
            if (analytic_expansion({current.x, current.z, current.heading}, tail)) {
                result.path = reconstruct(entry.node);
                result.path.points.insert(
                    result.path.points.end(), tail.points.begin(), tail.points.end());
                result.ok = true;
                return result;
            }
        }

        for (int dir_index = 0; dir_index < direction_count; ++dir_index) {
            const int direction = (dir_index == 0) ? 1 : -1;
            for (int steer_index = 0; steer_index < kSteerCount; ++steer_index) {
                const double kappa = kSteerFractions[steer_index] / min_turn_radius_m_;
                const double signed_s = static_cast<double>(direction) * arc_length_m_;
                const PlanarPose from{current.x, current.z, current.heading};
                double mean_cost = 0.0;
                if (!arc_is_free(from, kappa, signed_s, mean_cost)) {
                    continue;
                }
                const PlanarPose next = propagate_arc(from, kappa, signed_s);
                std::size_t key = 0;
                if (!state_key(next.x, next.z, next.heading, key)) {
                    continue;
                }

                double edge = arc_length_m_;
                if (direction < 0) {
                    edge *= reverse_penalty_;
                }
                edge += steer_change_penalty_
                    * std::abs(kSteerFractions[steer_index]
                               - kSteerFractions[current.steer_index]);
                edge += cost_weight_ * (mean_cost / 254.0) * arc_length_m_;
                const double tentative_g = current.g + edge;
                if (tentative_g >= best_g[key] - 1e-12) {
                    continue;
                }
                best_g[key] = tentative_g;

                SearchNode child;
                child.x = next.x;
                child.z = next.z;
                child.heading = next.heading;
                child.g = tentative_g;
                child.parent = entry.node;
                child.steer_index = steer_index;
                child.direction = direction;
                arena.push_back(child);
                open.push({tentative_g + heuristic(next.x, next.z), tentative_g,
                           push_order++, static_cast<int>(arena.size()) - 1});
            }
        }
    }

    result.reason = "no_path";
    return result;
}

} // namespace agbot::nav
