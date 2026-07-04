// Tests for the M6 occupancy-based autonomy evidence loop (CityEvidence).
// Footprints are rasterized into an occupancy costmap and a delivery robot is
// routed across it; the nav gate scores length, clearance, and recovery.

#include "agbot_flight_sim/Mission.hpp"
#include "agbot_nav/CityEvidence.hpp"
#include "agbot_nav/NavTypes.hpp"
#include "agbot_render/OffscreenRenderer.hpp"
#include "agbot_worldgen/Feature.hpp"

#include <cmath>
#include <iostream>
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

using agbot::flight_sim::GeoCoordinate;
using agbot::nav::CityOccupancyParams;
using agbot::nav::EvidenceFailure;
using agbot::nav::EvidenceLoopSpec;
using agbot::nav::OccupancyGrid;
using agbot::worldgen::ExtractedFeature;

// A rectangular building given in local meters, converted to geodetic rings via
// the library's exact geo_from_local inverse so footprints round-trip through
// local_from_geo inside the occupancy rasterizer without projection drift.
GeoCoordinate geo(double x, double z, const GeoCoordinate& origin) {
    return agbot::flight_sim::geo_from_local({x, 0.0, z}, origin);
}

ExtractedFeature make_box(double cx, double cz, double half, const GeoCoordinate& origin) {
    ExtractedFeature f;
    f.cls = agbot::worldgen::FeatureClass::Building;
    f.class_name = "building";
    f.exterior = {
        geo(cx - half, cz - half, origin),
        geo(cx + half, cz - half, origin),
        geo(cx + half, cz + half, origin),
        geo(cx - half, cz + half, origin),
    };
    return f;
}

void test_occupancy_marks_footprint_and_leaves_gaps() {
    const GeoCoordinate origin{40.71, -74.0, 0.0};
    std::vector<ExtractedFeature> buildings = {
        make_box(-40.0, 0.0, 15.0, origin),
        make_box(40.0, 0.0, 15.0, origin),
    };
    CityOccupancyParams params;
    params.half_extent_m = 120.0;
    params.resolution_m = 2.0;
    params.inflation_cells = 0;
    const OccupancyGrid grid = agbot::nav::build_city_occupancy(buildings, origin, params);

    // Interior of a box is lethal.
    expect(grid.cost_at_world(-40.0, 0.0) >= OccupancyGrid::kLethal,
           "footprint interior is lethal");
    // The corridor between the two boxes (x=0) is free.
    expect(grid.cost_at_world(0.0, 0.0) < OccupancyGrid::kLethal,
           "street corridor between footprints stays free");
    // Well outside every footprint is free.
    expect(grid.cost_at_world(0.0, 100.0) < OccupancyGrid::kLethal,
           "open ground is free");
}

void test_inflation_grows_lethal_region() {
    const GeoCoordinate origin{40.71, -74.0, 0.0};
    std::vector<ExtractedFeature> buildings = {make_box(0.0, 0.0, 10.0, origin)};
    CityOccupancyParams base;
    base.half_extent_m = 80.0;
    base.resolution_m = 2.0;
    base.inflation_cells = 0;
    CityOccupancyParams inflated = base;
    inflated.inflation_cells = 3;

    std::size_t n0 = 0;
    std::size_t n1 = 0;
    for (auto c : agbot::nav::build_city_occupancy(buildings, origin, base).cells) {
        n0 += c >= OccupancyGrid::kLethal ? 1 : 0;
    }
    for (auto c : agbot::nav::build_city_occupancy(buildings, origin, inflated).cells) {
        n1 += c >= OccupancyGrid::kLethal ? 1 : 0;
    }
    expect(n0 > 0, "un-inflated box marks lethal cells");
    expect(n1 > n0, "inflation grows the lethal region");
}

void test_hole_is_left_free() {
    const GeoCoordinate origin{40.71, -74.0, 0.0};
    ExtractedFeature donut = make_box(0.0, 0.0, 30.0, origin);
    // Inner courtyard hole.
    donut.holes = {{
        geo(-10.0, -10.0, origin),
        geo(10.0, -10.0, origin),
        geo(10.0, 10.0, origin),
        geo(-10.0, 10.0, origin),
    }};
    CityOccupancyParams params;
    params.half_extent_m = 80.0;
    params.resolution_m = 2.0;
    params.inflation_cells = 0;
    const OccupancyGrid grid = agbot::nav::build_city_occupancy({donut}, origin, params);
    expect(grid.cost_at_world(0.0, 0.0) < OccupancyGrid::kLethal,
           "courtyard hole is left free");
    expect(grid.cost_at_world(25.0, 0.0) >= OccupancyGrid::kLethal,
           "donut wall is lethal");
}

void test_evidence_loop_routes_and_recovers() {
    const GeoCoordinate origin{40.71, -74.0, 0.0};
    // A wall of boxes with a gap the planner must thread.
    std::vector<ExtractedFeature> buildings;
    for (double z = -200.0; z <= 200.0; z += 40.0) {
        if (std::abs(z) < 30.0) {
            continue; // leave a doorway near the center
        }
        buildings.push_back(make_box(0.0, z, 18.0, origin));
    }
    EvidenceLoopSpec spec;
    spec.start = {-300.0, 0.0, 0.0};
    spec.goal = {300.0, 0.0, 0.0};
    spec.occupancy.half_extent_m = 400.0;
    spec.occupancy.resolution_m = 4.0;
    spec.occupancy.inflation_cells = 1;

    const auto r = agbot::nav::run_evidence_loop(buildings, origin, spec);
    expect(r.ok, "evidence loop finds a goal-reaching plan");
    expect(r.failure == EvidenceFailure::None, "no failure class on success");
    expect(r.collision_free, "final plan is collision-free");
    expect(r.length_m >= r.euclidean_m, "plan is at least as long as straight line");
    expect(r.lethal_cells > 0, "occupancy has lethal cells");
    expect(r.recovery_count == 1 && r.replan_count == 1, "recovery probe replans once");
    expect(r.min_clearance_m >= 0.0, "clearance is defined");
}

void test_blocked_goal_is_reason_coded() {
    const GeoCoordinate origin{40.71, -74.0, 0.0};
    std::vector<ExtractedFeature> buildings = {make_box(300.0, 0.0, 40.0, origin)};
    EvidenceLoopSpec spec;
    spec.start = {-300.0, 0.0, 0.0};
    spec.goal = {300.0, 0.0, 0.0}; // inside the box
    spec.occupancy.half_extent_m = 400.0;
    spec.occupancy.resolution_m = 4.0;
    spec.snap_radius_cells = 0; // no goal tolerance: keep the endpoint blocked
    const auto r = agbot::nav::run_evidence_loop(buildings, origin, spec);
    expect(!r.ok, "loop fails when goal is inside a building");
    expect(r.failure == EvidenceFailure::GoalBlocked, "goal-blocked is reason-coded");
}

void test_snap_recovers_blocked_goal() {
    const GeoCoordinate origin{40.71, -74.0, 0.0};
    // A small building straddling the goal; a free street lies just beside it.
    std::vector<ExtractedFeature> buildings = {make_box(280.0, 0.0, 20.0, origin)};
    EvidenceLoopSpec spec;
    spec.start = {-300.0, 0.0, 0.0};
    spec.goal = {280.0, 0.0, 0.0}; // inside the box
    spec.occupancy.half_extent_m = 400.0;
    spec.occupancy.resolution_m = 4.0;
    spec.snap_radius_cells = 30; // goal tolerance snaps to the nearby street
    const auto r = agbot::nav::run_evidence_loop(buildings, origin, spec);
    expect(r.ok, "goal tolerance snaps a blocked goal to a reachable free cell");
    expect(r.failure == EvidenceFailure::None, "snapped goal has no failure class");
}

void test_determinism() {
    const GeoCoordinate origin{40.71, -74.0, 0.0};
    std::vector<ExtractedFeature> buildings = {
        make_box(-60.0, 20.0, 20.0, origin),
        make_box(70.0, -30.0, 25.0, origin),
    };
    EvidenceLoopSpec spec;
    spec.occupancy.half_extent_m = 300.0;
    spec.occupancy.resolution_m = 4.0;
    const auto a = agbot::nav::run_evidence_loop(buildings, origin, spec);
    const auto b = agbot::nav::run_evidence_loop(buildings, origin, spec);
    expect(a.ok == b.ok && a.length_m == b.length_m && a.lethal_cells == b.lethal_cells &&
               a.min_clearance_m == b.min_clearance_m,
           "evidence loop is deterministic across runs");
}

void test_sensor_occupancy_backprojects_obstacles() {
    // Nadir camera 100 m up, looking straight down; depth is eye-space metres
    // along forward, so world height = 100 - depth.
    agbot::render::OffscreenCamera cam;
    cam.eye = {0.0f, 100.0f, 0.0f};
    cam.target = {0.0f, 0.0f, 0.0f};
    cam.up = {0.0f, 0.0f, -1.0f};

    agbot::render::SensorFrame frame;
    frame.width = 40;
    frame.height = 40;
    frame.depth.assign(1600, 100.0f);   // ground everywhere (world y = 0)
    frame.semantic.assign(1600, 1);
    // Central 16x16 patch is a rooftop at world y = 30 m (depth 70).
    for (int y = 12; y < 28; ++y) {
        for (int x = 12; x < 28; ++x) {
            frame.depth[static_cast<std::size_t>(y) * 40 + x] = 70.0f;
        }
    }
    agbot::nav::SensorOccupancyParams p;
    p.grid.half_extent_m = 50.0;
    // Cells wider than the ~2 m synthetic depth-sample spacing so the central
    // cell is covered (a dense real frame does not need this).
    p.grid.resolution_m = 4.0;
    p.grid.inflation_cells = 0;
    p.min_obstacle_height_m = 3.0f;
    const agbot::nav::OccupancyGrid grid =
        agbot::nav::occupancy_from_sensor_frame(frame, cam, p);

    expect(grid.cost_at_world(0.0, 0.0) >= OccupancyGrid::kLethal,
           "rooftop hit back-projects to a lethal cell near the image center");
    expect(grid.cost_at_world(45.0, 45.0) < OccupancyGrid::kLethal,
           "far-field ground stays free");
    std::size_t lethal = 0;
    for (auto c : grid.cells) lethal += c >= OccupancyGrid::kLethal ? 1 : 0;
    expect(lethal > 0, "sensor occupancy has lethal cells from the rooftop patch");
}

void test_occupancy_consistency_metric() {
    OccupancyGrid footprint;
    footprint.width = 10;
    footprint.height = 10;
    footprint.resolution_m = 1.0;
    footprint.reset(0);
    OccupancyGrid sensor = footprint;
    // Footprint: 3x3 block at cols/rows 2..4.
    for (int cz = 2; cz <= 4; ++cz) {
        for (int cx = 2; cx <= 4; ++cx) {
            footprint.set(cx, cz, OccupancyGrid::kLethal);
        }
    }
    // Sensor: 2x2 block inside the footprint (all real) + one stray hit at (8,8).
    for (int cz = 2; cz <= 3; ++cz) {
        for (int cx = 2; cx <= 3; ++cx) {
            sensor.set(cx, cz, OccupancyGrid::kLethal);
        }
    }
    sensor.set(8, 8, OccupancyGrid::kLethal);

    const auto c = agbot::nav::occupancy_consistency(sensor, footprint, 0);
    expect(c.sensor_lethal == 5 && c.footprint_lethal == 9, "consistency counts lethal cells");
    expect(c.agree_lethal == 4, "4 sensor cells agree with the footprint");
    expect(std::abs(c.precision - 0.8) < 1e-9, "precision = agree / sensor_lethal = 0.8");
    expect(std::abs(c.recall - 4.0 / 9.0) < 1e-9, "recall = seen footprint / footprint_lethal");
}

void test_evidence_loop_reports_planner_effort() {
    const GeoCoordinate origin{40.71, -74.0, 0.0};
    std::vector<ExtractedFeature> buildings = {make_box(0.0, 0.0, 20.0, origin)};
    EvidenceLoopSpec spec;
    spec.start = {-250.0, 0.0, -250.0};
    spec.goal = {250.0, 0.0, 250.0};
    spec.occupancy.half_extent_m = 300.0;
    spec.occupancy.resolution_m = 4.0;
    const auto r = agbot::nav::run_evidence_loop(buildings, origin, spec);
    expect(r.ok, "evidence loop plans around a single block");
    expect(r.time_to_first_plan > 0, "time-to-first-plan (A* expansions) is reported");
    expect(r.time_in_recovery > 0, "time-in-recovery (replan expansions) is reported");
}

} // namespace

int main() {
    test_occupancy_marks_footprint_and_leaves_gaps();
    test_inflation_grows_lethal_region();
    test_hole_is_left_free();
    test_evidence_loop_routes_and_recovers();
    test_blocked_goal_is_reason_coded();
    test_snap_recovers_blocked_goal();
    test_sensor_occupancy_backprojects_obstacles();
    test_occupancy_consistency_metric();
    test_evidence_loop_reports_planner_effort();
    test_determinism();

    if (failures > 0) {
        std::cout << failures << " failure(s)\n";
        return 1;
    }
    std::cout << "all city-evidence tests passed\n";
    return 0;
}
