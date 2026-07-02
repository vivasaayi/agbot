#include "agbot_nav/GlobalPlanner.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <queue>
#include <tuple>
#include <vector>

namespace agbot::nav {

namespace {

constexpr double kSqrt2 = 1.41421356237309504880;

struct OpenEntry {
    double f = 0.0;
    double g = 0.0;
    std::size_t index = 0;

    // Deterministic ordering: min f, then min g, then min cell index.
    bool operator>(const OpenEntry& other) const {
        if (f != other.f) {
            return f > other.f;
        }
        if (g != other.g) {
            return g > other.g;
        }
        return index > other.index;
    }
};

} // namespace

bool segment_is_traversable(
    const Costmap& costmap,
    const Vec3& from,
    const Vec3& to,
    std::uint8_t lethal_threshold,
    std::uint8_t unknown_cost) {
    const double distance = (to - from).horizontal_length();
    const double step = costmap.resolution_m > 1e-6 ? costmap.resolution_m * 0.5 : 0.1;
    const int samples = std::max(1, static_cast<int>(std::ceil(distance / step)));
    for (int i = 0; i <= samples; ++i) {
        const double alpha = static_cast<double>(i) / static_cast<double>(samples);
        const double wx = from.x + (to.x - from.x) * alpha;
        const double wz = from.z + (to.z - from.z) * alpha;
        std::uint8_t cost = costmap.cost_at_world(wx, wz);
        if (cost == OccupancyGrid::kUnknown) {
            cost = unknown_cost;
        }
        if (cost >= lethal_threshold) {
            return false;
        }
    }
    return true;
}

AStarPlanner::AStarPlanner(const agbot::config::ParamTable& params) {
    lethal_threshold_ = static_cast<std::uint8_t>(std::clamp<std::int64_t>(
        agbot::config::integer_or(params, "lethal_threshold", lethal_threshold_), 1, 254));
    heuristic_weight_ =
        agbot::config::double_or(params, "heuristic_weight", heuristic_weight_);
    cost_weight_ = agbot::config::double_or(params, "cost_weight", cost_weight_);
    unknown_cost_ = static_cast<std::uint8_t>(std::clamp<std::int64_t>(
        agbot::config::integer_or(params, "unknown_cost", unknown_cost_), 0, 254));
    smooth_ = agbot::config::bool_or(params, "smooth", smooth_);
    smooth_max_cost_ = static_cast<std::uint8_t>(std::clamp<std::int64_t>(
        agbot::config::integer_or(params, "smooth_max_cost", smooth_max_cost_), 1, 254));
}

PlanResult AStarPlanner::plan(const Costmap& costmap, const Vec3& start, const Vec3& goal) {
    PlanResult result;
    if (costmap.width <= 0 || costmap.height <= 0) {
        result.reason = "empty_costmap";
        return result;
    }

    int start_cx = 0;
    int start_cz = 0;
    int goal_cx = 0;
    int goal_cz = 0;
    if (!costmap.world_to_cell(start.x, start.z, start_cx, start_cz)) {
        result.reason = "start_outside_costmap";
        return result;
    }
    if (!costmap.world_to_cell(goal.x, goal.z, goal_cx, goal_cz)) {
        result.reason = "goal_outside_costmap";
        return result;
    }

    auto effective_cost = [&](int cx, int cz) -> std::uint8_t {
        const std::uint8_t cost = costmap.at(cx, cz);
        return cost == OccupancyGrid::kUnknown ? unknown_cost_ : cost;
    };
    if (effective_cost(goal_cx, goal_cz) >= lethal_threshold_) {
        result.reason = "goal_in_lethal_cell";
        return result;
    }

    const std::size_t cell_count =
        static_cast<std::size_t>(costmap.width) * static_cast<std::size_t>(costmap.height);
    std::vector<double> g_score(cell_count, std::numeric_limits<double>::infinity());
    std::vector<std::size_t> came_from(cell_count, std::numeric_limits<std::size_t>::max());
    std::vector<bool> closed(cell_count, false);

    auto heuristic = [&](int cx, int cz) {
        const double dx = static_cast<double>(cx - goal_cx);
        const double dz = static_cast<double>(cz - goal_cz);
        return heuristic_weight_ * std::sqrt(dx * dx + dz * dz);
    };

    std::priority_queue<OpenEntry, std::vector<OpenEntry>, std::greater<OpenEntry>> open;
    const std::size_t start_index = costmap.index(start_cx, start_cz);
    const std::size_t goal_index = costmap.index(goal_cx, goal_cz);
    g_score[start_index] = 0.0;
    open.push({heuristic(start_cx, start_cz), 0.0, start_index});

    static constexpr int kNeighborDx[8] = {1, -1, 0, 0, 1, 1, -1, -1};
    static constexpr int kNeighborDz[8] = {0, 0, 1, -1, 1, -1, 1, -1};

    bool found = false;
    while (!open.empty()) {
        const OpenEntry current = open.top();
        open.pop();
        if (closed[current.index]) {
            continue;
        }
        closed[current.index] = true;
        if (current.index == goal_index) {
            found = true;
            break;
        }
        const int cx = static_cast<int>(current.index % static_cast<std::size_t>(costmap.width));
        const int cz = static_cast<int>(current.index / static_cast<std::size_t>(costmap.width));
        for (int n = 0; n < 8; ++n) {
            const int nx = cx + kNeighborDx[n];
            const int nz = cz + kNeighborDz[n];
            if (!costmap.in_bounds(nx, nz)) {
                continue;
            }
            const std::uint8_t cell_cost = effective_cost(nx, nz);
            if (cell_cost >= lethal_threshold_) {
                continue;
            }
            const std::size_t neighbor_index = costmap.index(nx, nz);
            if (closed[neighbor_index]) {
                continue;
            }
            const double step_length = n < 4 ? 1.0 : kSqrt2;
            const double traversal = step_length
                * (1.0 + cost_weight_ * static_cast<double>(cell_cost) / 254.0);
            const double tentative_g = g_score[current.index] + traversal;
            if (tentative_g < g_score[neighbor_index]) {
                g_score[neighbor_index] = tentative_g;
                came_from[neighbor_index] = current.index;
                open.push({tentative_g + heuristic(nx, nz), tentative_g, neighbor_index});
            }
        }
    }

    if (!found) {
        result.reason = "no_path";
        return result;
    }

    std::vector<std::size_t> reversed;
    for (std::size_t index = goal_index; index != std::numeric_limits<std::size_t>::max();
         index = came_from[index]) {
        reversed.push_back(index);
        if (index == start_index) {
            break;
        }
    }
    std::reverse(reversed.begin(), reversed.end());

    Path raw;
    raw.points.reserve(reversed.size());
    for (const std::size_t index : reversed) {
        const int cx = static_cast<int>(index % static_cast<std::size_t>(costmap.width));
        const int cz = static_cast<int>(index / static_cast<std::size_t>(costmap.width));
        raw.points.push_back(costmap.cell_to_world(cx, cz));
    }
    // Anchor exact endpoints.
    if (!raw.points.empty()) {
        raw.points.front() = {start.x, 0.0, start.z};
        raw.points.back() = {goal.x, 0.0, goal.z};
    }

    if (smooth_ && raw.points.size() > 2) {
        // String pulling: greedily connect the furthest waypoint reachable by
        // a collision-free straight segment.
        Path smoothed;
        std::size_t anchor = 0;
        smoothed.points.push_back(raw.points.front());
        while (anchor + 1 < raw.points.size()) {
            std::size_t reach = anchor + 1;
            for (std::size_t candidate = raw.points.size() - 1; candidate > anchor + 1;
                 --candidate) {
                if (segment_is_traversable(costmap, raw.points[anchor], raw.points[candidate],
                                           std::min(lethal_threshold_, smooth_max_cost_),
                                           unknown_cost_)) {
                    reach = candidate;
                    break;
                }
            }
            smoothed.points.push_back(raw.points[reach]);
            anchor = reach;
        }
        result.path = std::move(smoothed);
    } else {
        result.path = std::move(raw);
    }

    result.ok = true;
    return result;
}

const GlobalPlannerRegistry& default_global_planner_registry() {
    static const GlobalPlannerRegistry registry = [] {
        GlobalPlannerRegistry built;
        built.register_factory(
            "astar",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<IGlobalPlanner> {
                return std::make_unique<AStarPlanner>(params);
            });
        return built;
    }();
    return registry;
}

} // namespace agbot::nav
