#include "agbot_nav/GlobalPlanner.hpp"
#include "agbot_nav/RoadGraphPlanner.hpp"

#include "agbot_worldgen/RoadNetwork.hpp"

#include <algorithm>
#include <cmath>
#include <filesystem>
#include <iostream>
#include <memory>
#include <string>
#include <vector>

namespace {

int failures = 0;

void expect(bool condition, const std::string& label) {
    if (condition) {
        std::cout << "PASS " << label << "\n";
    } else {
        std::cout << "FAIL " << label << "\n";
        ++failures;
    }
}

bool near(double actual, double expected, double tolerance) {
    return std::abs(actual - expected) <= tolerance;
}

using agbot::nav::Costmap;
using agbot::nav::Path;
using agbot::nav::PlanResult;
using agbot::nav::RoadGraphPlanner;
using agbot::nav::Vec3;
using agbot::worldgen::ExtractedFeature;
using agbot::worldgen::RoadNetwork;

const agbot::flight_sim::GeoCoordinate kOrigin{0.0, 0.0, 0.0};

ExtractedFeature make_road(
    const std::string& way_id,
    const std::string& highway,
    const std::string& oneway,
    const std::vector<std::pair<double, double>>& points_xz) {
    ExtractedFeature feature;
    feature.cls = agbot::worldgen::FeatureClass::Road;
    feature.class_name = "road";
    feature.source_id = "way:" + way_id;
    feature.attributes["geometry_type"] = "polyline";
    feature.attributes["highway"] = highway;
    feature.attributes["oneway"] = oneway;
    feature.attributes["way_id"] = way_id;
    for (const auto& [x, z] : points_xz) {
        feature.exterior.push_back(agbot::flight_sim::geo_from_local({x, 0.0, z}, kOrigin));
    }
    return feature;
}

// 400 x 200 m street grid around the origin:
//   A Street  (residential, two-way): (0,0)-(200,0)-(400,0)
//   B Street  (residential, two-way): (0,200)-(200,200)-(400,200)
//   First Ave (primary, two-way):     (0,0)-(0,200)        fast: 14 m/s
//   Second Ave(residential, ONEWAY):  (200,0)->(200,200)   northbound only
//   Third Ave (residential, two-way): (400,0)-(400,200)
//   Shortcut  (service, two-way):     (0,0)-(200,200)      short but 5 m/s
std::shared_ptr<const RoadNetwork> fixture_network() {
    std::vector<ExtractedFeature> features;
    features.push_back(make_road("101", "residential", "no", {{0, 0}, {200, 0}, {400, 0}}));
    features.push_back(
        make_road("102", "residential", "no", {{0, 200}, {200, 200}, {400, 200}}));
    features.push_back(make_road("103", "primary", "no", {{0, 0}, {0, 200}}));
    features.push_back(make_road("104", "residential", "yes", {{200, 0}, {200, 200}}));
    features.push_back(make_road("105", "residential", "no", {{400, 0}, {400, 200}}));
    features.push_back(make_road("107", "service", "no", {{0, 0}, {200, 200}}));
    return std::make_shared<const RoadNetwork>(
        RoadNetwork::build(features, kOrigin, agbot::worldgen::RoadNetworkParams{}));
}

RoadGraphPlanner make_planner(
    std::shared_ptr<const RoadNetwork> network,
    const agbot::config::ParamTable& params = {}) {
    RoadGraphPlanner planner(params);
    planner.set_network(std::move(network));
    return planner;
}

double max_corridor_deviation_m(const RoadNetwork& network, const Path& path) {
    double worst = 0.0;
    for (const Vec3& point : path.points) {
        const auto projection = network.nearest_edge_point(point.x, point.z);
        worst = std::max(worst, projection.distance_m);
    }
    return worst;
}

bool passes_near(const Path& path, double x, double z, double tolerance) {
    return std::any_of(path.points.begin(), path.points.end(), [&](const Vec3& point) {
        return std::hypot(point.x - x, point.z - z) <= tolerance;
    });
}

void test_registry() {
    const auto& registry = agbot::nav::default_global_planner_registry();
    expect(registry.contains("road_graph"), "registry contains road_graph");
    const auto planner = registry.create("road_graph", {});
    expect(planner != nullptr && planner->name() == "road_graph", "registry creates road_graph");
    if (planner) {
        const PlanResult result = planner->plan(Costmap{}, {0, 0, 0}, {10, 0, 10});
        expect(!result.ok && result.reason == "road_network_missing",
               "planning without a network is reason-coded");
    }
}

void test_routes_on_streets() {
    const auto network = fixture_network();
    RoadGraphPlanner planner = make_planner(network);
    const PlanResult result = planner.plan(Costmap{}, {10, 0, 3}, {390, 0, 197});
    expect(result.ok, "grid route succeeds");
    if (!result.ok) {
        return;
    }
    // Streets force a manhattan route (~580 m); a straight line would be
    // ~430 m and would cut across the blocks.
    expect(result.path.length_m() > 500.0 && result.path.length_m() < 640.0,
           "route length is manhattan, not euclidean");
    expect(max_corridor_deviation_m(*network, result.path) <= 5.0,
           "every path point stays in the street corridor");
    expect(near(result.path.points.front().x, 10.0, 1e-6) &&
               near(result.path.points.front().z, 3.0, 1e-6),
           "path starts at the requested start");
    expect(near(result.path.points.back().x, 390.0, 1e-6) &&
               near(result.path.points.back().z, 197.0, 1e-6),
           "path ends at the requested goal");
}

void test_oneway_respected() {
    const auto network = fixture_network();
    RoadGraphPlanner planner = make_planner(network);

    const PlanResult forward = planner.plan(Costmap{}, {200, 0, 10}, {200, 0, 190});
    expect(forward.ok, "northbound route on the oneway succeeds");
    if (forward.ok) {
        expect(near(forward.path.length_m(), 180.0, 5.0),
               "northbound route rides Second Avenue directly");
    }

    const PlanResult backward = planner.plan(Costmap{}, {200, 0, 190}, {200, 0, 10});
    expect(backward.ok, "southbound route around the oneway succeeds");
    if (backward.ok && forward.ok) {
        expect(backward.path.length_m() > forward.path.length_m() + 100.0,
               "southbound route detours around the oneway");
        expect(max_corridor_deviation_m(*network, backward.path) <= 5.0,
               "southbound detour stays on streets");
    }
}

void test_cost_modes_pick_different_routes() {
    const auto network = fixture_network();

    agbot::config::ParamTable time_params;
    time_params["cost_mode"] = "time";
    RoadGraphPlanner time_planner = make_planner(network, time_params);
    const PlanResult by_time = time_planner.plan(Costmap{}, {0, 0, 0}, {200, 0, 200});

    agbot::config::ParamTable distance_params;
    distance_params["cost_mode"] = "distance";
    RoadGraphPlanner distance_planner = make_planner(network, distance_params);
    const PlanResult by_distance = distance_planner.plan(Costmap{}, {0, 0, 0}, {200, 0, 200});

    expect(by_time.ok && by_distance.ok, "both cost modes find a route");
    if (!by_time.ok || !by_distance.ok) {
        return;
    }
    // Distance mode takes the 283 m service shortcut; time mode prefers the
    // 400 m primary + residential detour (28.6 s vs 56.6 s).
    expect(near(by_distance.path.length_m(), 200.0 * std::sqrt(2.0), 5.0),
           "distance mode takes the short slow diagonal");
    expect(near(by_time.path.length_m(), 400.0, 5.0), "time mode takes the longer fast route");
    expect(passes_near(by_time.path, 0.0, 200.0, 2.0),
           "time mode routes through the primary avenue junction");
}

void test_snap_limits() {
    const auto network = fixture_network();
    RoadGraphPlanner planner = make_planner(network);

    const PlanResult far_goal = planner.plan(Costmap{}, {10, 0, 3}, {5000, 0, 5000});
    expect(!far_goal.ok && far_goal.reason == "goal_snap_exceeds_max_snap_m",
           "goal beyond max_snap_m is reason-coded");

    const PlanResult far_start = planner.plan(Costmap{}, {5000, 0, 5000}, {10, 0, 3});
    expect(!far_start.ok && far_start.reason == "start_snap_exceeds_max_snap_m",
           "start beyond max_snap_m is reason-coded");

    agbot::config::ParamTable wide;
    wide["max_snap_m"] = 7000.0;
    RoadGraphPlanner generous = make_planner(network, wide);
    expect(generous.plan(Costmap{}, {5000, 0, 5000}, {10, 0, 3}).ok,
           "raising max_snap_m accepts the same start");
}

void test_determinism() {
    const auto network = fixture_network();
    RoadGraphPlanner planner = make_planner(network);
    const PlanResult first = planner.plan(Costmap{}, {10, 0, 3}, {390, 0, 197});
    const PlanResult second = planner.plan(Costmap{}, {10, 0, 3}, {390, 0, 197});
    expect(first.ok && second.ok && first.path.points.size() == second.path.points.size(),
           "replans agree on waypoint count");
    bool identical = first.path.points.size() == second.path.points.size();
    for (std::size_t i = 0; identical && i < first.path.points.size(); ++i) {
        identical = first.path.points[i].x == second.path.points[i].x &&
            first.path.points[i].z == second.path.points[i].z;
    }
    expect(identical, "replans are bit-identical");
}

void test_manhattan_route() {
    const std::string roads_path =
        std::string(FLIGHT_SIM_ROOT_DIR) + "/data/worldgen/manhattan_roads.json";
    if (!std::filesystem::exists(roads_path)) {
        std::cout << "SKIP manhattan route (data/worldgen/manhattan_roads.json absent; run "
                  << "worldgen/tools/fetch_osm_roads.sh)\n";
        return;
    }
    // Exercise the self-loading path: the planner imports the Overpass dump
    // and builds the graph itself.
    agbot::config::ParamTable params;
    params["roads_path"] = roads_path;
    params["aoi_min_lat"] = 40.700;
    params["aoi_min_lon"] = -74.020;
    params["aoi_max_lat"] = 40.740;
    params["aoi_max_lon"] = -73.980;
    params["max_snap_m"] = 100.0;
    const auto planner = agbot::nav::default_global_planner_registry().create("road_graph", params);
    expect(planner != nullptr, "road_graph self-loads manhattan roads");

    // Two far-apart street-side points in local meters around the AOI center.
    const Vec3 start{-1200.0, 0.0, -800.0};
    const Vec3 goal{1200.0, 0.0, 800.0};
    const PlanResult result = planner->plan(Costmap{}, start, goal);
    expect(result.ok, "manhattan route succeeds");
    if (result.ok) {
        const double euclid = std::hypot(goal.x - start.x, goal.z - start.z);
        std::cout << "INFO manhattan route: " << result.path.points.size() << " points, "
                  << result.path.length_m() << " m vs euclid " << euclid << " m\n";
        expect(result.path.length_m() >= euclid, "street route is no shorter than euclidean");
        expect(result.path.length_m() <= 2.5 * euclid, "street route length is plausible");
    } else {
        std::cout << "INFO manhattan route failed: " << result.reason << "\n";
    }
}

} // namespace

int main() {
    test_registry();
    test_routes_on_streets();
    test_oneway_respected();
    test_cost_modes_pick_different_routes();
    test_snap_limits();
    test_determinism();
    test_manhattan_route();

    if (failures != 0) {
        std::cout << failures << " failure(s)\n";
        return 1;
    }
    std::cout << "all road planner tests passed\n";
    return 0;
}
