#include "agbot_nav/RoadGraphPlanner.hpp"

#include "agbot_worldgen/extractors/RoadImport.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <queue>
#include <vector>

namespace agbot::nav {

namespace {

using agbot::worldgen::EdgeProjection;
using agbot::worldgen::RoadEdge;
using agbot::worldgen::RoadNetwork;

constexpr std::uint32_t kNoNode = 0xffffffffu;
constexpr std::uint32_t kNoEdge = 0xffffffffu;

struct OpenEntry {
    double f = 0.0;
    double g = 0.0;
    std::uint32_t node = 0;

    // Deterministic ordering: min f, then min g, then min node id.
    bool operator>(const OpenEntry& other) const {
        if (f != other.f) {
            return f > other.f;
        }
        if (g != other.g) {
            return g > other.g;
        }
        return node > other.node;
    }
};

// Sub-polyline of `polyline` covering arc lengths [s0, s1] from its start;
// projected endpoints are interpolated.
std::vector<Vec3> sub_polyline(const std::vector<Vec3>& polyline, double s0, double s1) {
    std::vector<Vec3> result;
    if (polyline.size() < 2 || s1 <= s0) {
        return result;
    }
    const auto point_at = [&polyline](double s) -> Vec3 {
        double offset = 0.0;
        for (std::size_t i = 1; i < polyline.size(); ++i) {
            const Vec3& a = polyline[i - 1];
            const Vec3& b = polyline[i];
            const double segment = std::hypot(b.x - a.x, b.z - a.z);
            if (offset + segment >= s || i + 1 == polyline.size()) {
                const double t = segment > 0.0 ? std::clamp((s - offset) / segment, 0.0, 1.0) : 0.0;
                return {a.x + t * (b.x - a.x), 0.0, a.z + t * (b.z - a.z)};
            }
            offset += segment;
        }
        return polyline.back();
    };
    result.push_back(point_at(s0));
    double offset = 0.0;
    for (std::size_t i = 1; i < polyline.size(); ++i) {
        const Vec3& a = polyline[i - 1];
        const Vec3& b = polyline[i];
        const double segment = std::hypot(b.x - a.x, b.z - a.z);
        const double vertex_s = offset + segment;
        if (vertex_s > s0 && vertex_s < s1) {
            result.push_back(b);
        }
        offset = vertex_s;
    }
    result.push_back(point_at(s1));
    return result;
}

void append_points(std::vector<Vec3>& path, const std::vector<Vec3>& points) {
    for (const Vec3& point : points) {
        if (!path.empty()) {
            const Vec3& last = path.back();
            if (std::hypot(point.x - last.x, point.z - last.z) < 1e-6) {
                continue;
            }
        }
        path.push_back(point);
    }
}

} // namespace

RoadGraphPlanner::RoadGraphPlanner(const agbot::config::ParamTable& params) {
    namespace cfg = agbot::config;
    cost_mode_ = cfg::string_or(params, "cost_mode", cost_mode_);
    max_snap_m_ = cfg::double_or(params, "max_snap_m", max_snap_m_);
    heuristic_weight_ = cfg::double_or(params, "heuristic_weight", heuristic_weight_);
    use_costmap_ = cfg::bool_or(params, "use_costmap", use_costmap_);
    lethal_threshold_ = static_cast<std::uint8_t>(std::clamp<std::int64_t>(
        cfg::integer_or(params, "lethal_threshold", lethal_threshold_), 1, 254));

    // Optional self-load: run the road_import extractor over roads_path and
    // build the network around the AOI center. Failures are reason-coded and
    // reported by plan().
    const std::string roads_path = cfg::string_or(params, "roads_path", "");
    if (roads_path.empty()) {
        return;
    }
    agbot::flight_sim::GeoBounds aoi;
    aoi.min_latitude = cfg::double_or(params, "aoi_min_lat", 0.0);
    aoi.min_longitude = cfg::double_or(params, "aoi_min_lon", 0.0);
    aoi.max_latitude = cfg::double_or(params, "aoi_max_lat", 0.0);
    aoi.max_longitude = cfg::double_or(params, "aoi_max_lon", 0.0);
    if (aoi.min_latitude >= aoi.max_latitude || aoi.min_longitude >= aoi.max_longitude) {
        load_error_ = "road_data_load_failed:invalid_aoi";
        return;
    }
    agbot::config::ParamTable import_params = params;
    import_params["path"] = roads_path;
    const agbot::worldgen::RoadImportExtractor extractor;
    const agbot::worldgen::ExtractionResult imported =
        extractor.extract({aoi, import_params});
    if (!imported.ok) {
        load_error_ = "road_data_load_failed:" + imported.error_code;
        return;
    }
    network_ = std::make_shared<const RoadNetwork>(RoadNetwork::build(
        imported.features, aoi.center(), agbot::worldgen::road_network_params_from(params)));
}

PlanResult RoadGraphPlanner::plan(const Costmap& costmap, const Vec3& start, const Vec3& goal) {
    PlanResult result;
    if (!load_error_.empty()) {
        result.reason = load_error_;
        return result;
    }
    if (!network_) {
        result.reason = "road_network_missing";
        return result;
    }
    const RoadNetwork& network = *network_;
    if (network.edges().empty()) {
        result.reason = "road_network_empty";
        return result;
    }

    const EdgeProjection start_snap = network.nearest_edge_point(start.x, start.z);
    if (!start_snap.ok || start_snap.distance_m > max_snap_m_) {
        result.reason = "start_snap_exceeds_max_snap_m";
        return result;
    }
    const EdgeProjection goal_snap = network.nearest_edge_point(goal.x, goal.z);
    if (!goal_snap.ok || goal_snap.distance_m > max_snap_m_) {
        result.reason = "goal_snap_exceeds_max_snap_m";
        return result;
    }

    const bool time_mode = cost_mode_ != "distance";
    const auto meters_cost = [time_mode](const RoadEdge& edge, double meters) {
        return time_mode ? meters / edge.speed_mps : meters;
    };
    const auto edge_cost = [time_mode](const RoadEdge& edge) {
        return time_mode ? edge.travel_time_s : edge.length_m;
    };
    const auto edge_usable = [this, &costmap](const RoadEdge& edge) {
        if (!use_costmap_ || costmap.width <= 0 || costmap.height <= 0) {
            return true;
        }
        for (std::size_t i = 1; i < edge.polyline.size(); ++i) {
            if (!segment_is_traversable(costmap, edge.polyline[i - 1], edge.polyline[i],
                                        lethal_threshold_, 0)) {
                return false;
            }
        }
        return true;
    };

    double max_speed = 0.1;
    for (const RoadEdge& edge : network.edges()) {
        max_speed = std::max(max_speed, edge.speed_mps);
    }
    const auto heuristic = [&](std::uint32_t node_id) {
        const agbot::worldgen::RoadNode& node = network.nodes()[node_id];
        const double euclid = std::hypot(node.x - goal_snap.point.x, node.z - goal_snap.point.z);
        return heuristic_weight_ * (time_mode ? euclid / max_speed : euclid);
    };

    const RoadEdge& start_edge = network.edges()[start_snap.edge_id];
    const RoadEdge& goal_edge = network.edges()[goal_snap.edge_id];
    const RoadEdge* start_reverse = network.reverse_edge(start_edge.id);
    const RoadEdge* goal_reverse = network.reverse_edge(goal_edge.id);

    // Seeds: entering the graph from the start projection. Forward along the
    // snapped edge reaches its `to` node; when a reverse companion exists the
    // street is two-way and the `from` node is reachable as well.
    struct Seed {
        std::uint32_t node;
        double cost;
        std::uint32_t via_edge; // edge whose partial polyline enters the graph
        double s0;
        double s1;
    };
    std::vector<Seed> seeds;
    if (edge_usable(start_edge)) {
        seeds.push_back({start_edge.to,
                         meters_cost(start_edge, start_edge.length_m - start_snap.s_along_m),
                         start_edge.id, start_snap.s_along_m, start_edge.length_m});
    }
    if (start_reverse != nullptr && edge_usable(*start_reverse)) {
        seeds.push_back({start_reverse->to,
                         meters_cost(*start_reverse, start_snap.s_along_m),
                         start_reverse->id, start_edge.length_m - start_snap.s_along_m,
                         start_reverse->length_m});
    }

    // Goal targets: leaving the graph toward the goal projection.
    struct Target {
        std::uint32_t node;
        double cost;
        std::uint32_t via_edge;
        double s0;
        double s1;
    };
    std::vector<Target> targets;
    if (edge_usable(goal_edge)) {
        targets.push_back({goal_edge.from, meters_cost(goal_edge, goal_snap.s_along_m),
                           goal_edge.id, 0.0, goal_snap.s_along_m});
    }
    if (goal_reverse != nullptr && edge_usable(*goal_reverse)) {
        targets.push_back({goal_reverse->from,
                           meters_cost(*goal_reverse, goal_edge.length_m - goal_snap.s_along_m),
                           goal_reverse->id, 0.0, goal_edge.length_m - goal_snap.s_along_m});
    }

    // Direct candidate: start and goal on the same directed edge (or its
    // reverse companion), reachable without touching a junction.
    double direct_cost = std::numeric_limits<double>::infinity();
    std::vector<Vec3> direct_points;
    const auto consider_direct = [&](const RoadEdge& edge, double s0, double s1) {
        if (s1 < s0 || !edge_usable(edge)) {
            return;
        }
        const double cost = meters_cost(edge, s1 - s0);
        if (cost < direct_cost) {
            direct_cost = cost;
            direct_points = sub_polyline(edge.polyline, s0, s1);
        }
    };
    if (goal_snap.edge_id == start_edge.id) {
        consider_direct(start_edge, start_snap.s_along_m, goal_snap.s_along_m);
    }
    if (start_reverse != nullptr && goal_snap.edge_id == start_edge.id) {
        // Two-way street: traveling against the snapped direction is legal.
        consider_direct(*start_reverse, start_edge.length_m - start_snap.s_along_m,
                        start_edge.length_m - goal_snap.s_along_m);
    }

    // A* over graph nodes (closed-on-pop, deterministic tie-breaks).
    const std::size_t node_count = network.nodes().size();
    std::vector<double> g_score(node_count, std::numeric_limits<double>::infinity());
    std::vector<std::uint32_t> came_via_edge(node_count, kNoEdge);
    std::vector<std::uint32_t> seed_index(node_count, kNoNode);
    std::vector<bool> closed(node_count, false);
    std::priority_queue<OpenEntry, std::vector<OpenEntry>, std::greater<OpenEntry>> open;

    for (std::uint32_t i = 0; i < seeds.size(); ++i) {
        const Seed& seed = seeds[i];
        if (seed.cost < g_score[seed.node]) {
            g_score[seed.node] = seed.cost;
            seed_index[seed.node] = i;
            came_via_edge[seed.node] = kNoEdge;
            open.push({seed.cost + heuristic(seed.node), seed.cost, seed.node});
        }
    }

    while (!open.empty()) {
        const OpenEntry current = open.top();
        open.pop();
        if (closed[current.node]) {
            continue;
        }
        closed[current.node] = true;
        for (const std::uint32_t edge_id : network.out_edges(current.node)) {
            const RoadEdge& edge = network.edges()[edge_id];
            if (closed[edge.to] || !edge_usable(edge)) {
                continue;
            }
            const double tentative = g_score[current.node] + edge_cost(edge);
            if (tentative < g_score[edge.to]) {
                g_score[edge.to] = tentative;
                came_via_edge[edge.to] = edge_id;
                seed_index[edge.to] = kNoNode;
                open.push({tentative + heuristic(edge.to), tentative, edge.to});
            }
        }
    }

    // Best graph route among the goal targets (deterministic: first-listed
    // target wins ties).
    double best_cost = std::numeric_limits<double>::infinity();
    const Target* best_target = nullptr;
    for (const Target& target : targets) {
        if (g_score[target.node] == std::numeric_limits<double>::infinity()) {
            continue;
        }
        const double total = g_score[target.node] + target.cost;
        if (total < best_cost) {
            best_cost = total;
            best_target = &target;
        }
    }

    if (direct_cost <= best_cost && !direct_points.empty()) {
        result.path.points.push_back({start.x, 0.0, start.z});
        append_points(result.path.points, direct_points);
        append_points(result.path.points, {{goal.x, 0.0, goal.z}});
        result.ok = true;
        return result;
    }
    if (best_target == nullptr) {
        result.reason = "no_route";
        return result;
    }

    // Walk predecessors back to a seed node, collecting full edge polylines.
    std::vector<std::uint32_t> route_edges;
    std::uint32_t node = best_target->node;
    while (came_via_edge[node] != kNoEdge) {
        const std::uint32_t edge_id = came_via_edge[node];
        route_edges.push_back(edge_id);
        node = network.edges()[edge_id].from;
    }
    std::reverse(route_edges.begin(), route_edges.end());
    const Seed& entry_seed = seeds[seed_index[node]];

    result.path.points.push_back({start.x, 0.0, start.z});
    const RoadEdge& entry_edge = network.edges()[entry_seed.via_edge];
    append_points(result.path.points,
                  sub_polyline(entry_edge.polyline, entry_seed.s0, entry_seed.s1));
    for (const std::uint32_t edge_id : route_edges) {
        append_points(result.path.points, network.edges()[edge_id].polyline);
    }
    const RoadEdge& exit_edge = network.edges()[best_target->via_edge];
    append_points(result.path.points,
                  sub_polyline(exit_edge.polyline, best_target->s0, best_target->s1));
    append_points(result.path.points, {{goal.x, 0.0, goal.z}});
    result.ok = true;
    return result;
}

} // namespace agbot::nav
