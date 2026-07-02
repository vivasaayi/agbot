#include "agbot_nav/Controller.hpp"
#include "agbot_nav/GlobalPlanner.hpp"
#include "agbot_nav/LocalPlanner.hpp"
#include "agbot_nav/Mapping.hpp"
#include "agbot_nav/NavigationPipeline.hpp"
#include "agbot_nav/Perception.hpp"
#include "agbot_nav/Sensing.hpp"
#include "agbot_vehicles/KinematicBicycleModel.hpp"

#include <cmath>
#include <cstdint>
#include <iostream>
#include <sstream>
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

using agbot::flight_sim::SceneObject;
using agbot::flight_sim::SceneSynthesisManifest;
using agbot::flight_sim::SceneSynthesisStatus;
using agbot::nav::Costmap;
using agbot::nav::NavWorld;
using agbot::nav::OccupancyGrid;
using agbot::nav::Path;
using agbot::nav::Vec3;
using agbot::vehicles::EntityState;

SceneObject make_box(
    const std::string& id,
    double min_x,
    double max_x,
    double min_z,
    double max_z,
    double height_m) {
    SceneObject object;
    object.object_id = id;
    object.source_kind = "test";
    object.class_name = "building";
    object.height_m = height_m;
    object.footprint_local_m = {
        {min_x, 0.0, min_z},
        {max_x, 0.0, min_z},
        {max_x, 0.0, max_z},
        {min_x, 0.0, max_z},
    };
    return object;
}

// Corridor along +x: side walls at |z| in [4, 7], one offset obstacle at
// x in [38, 42] blocking z in [-4, 1]. Passable gap: z in (1, 4).
NavWorld make_corridor_world() {
    NavWorld world;
    world.ground_height_m = 0.0;
    world.scene.status = SceneSynthesisStatus::Ready;
    world.scene.scene_hash = "nav-test-corridor";
    world.scene.objects.push_back(make_box("wall_north", -5.0, 85.0, 4.0, 7.0, 3.0));
    world.scene.objects.push_back(make_box("wall_south", -5.0, 85.0, -7.0, -4.0, 3.0));
    world.scene.objects.push_back(make_box("offset_obstacle", 38.0, 42.0, -4.0, 1.0, 3.0));
    world.scene.object_count = world.scene.objects.size();
    return world;
}

// ---------------------------------------------------------------------------
// Stage unit tests
// ---------------------------------------------------------------------------

void test_depth_camera_geometry() {
    NavWorld world;
    world.ground_height_m = 0.0;
    world.scene.status = SceneSynthesisStatus::Ready;
    world.scene.objects.push_back(make_box("slab", 9.0, 11.0, -5.0, 5.0, 3.0));

    agbot::config::ParamTable params;
    params["width"] = 9;
    params["height"] = 9;
    params["horizontal_fov_deg"] = 40.0;
    params["vertical_fov_deg"] = 40.0;
    params["max_range_m"] = 30.0;
    params["mount_height_m"] = 1.0;
    agbot::nav::DepthCameraSensor sensor(params);

    EntityState state; // origin, yaw 0 -> facing +x
    const agbot::nav::SensorFrame frame = sensor.sense(world, state, 0.0);
    expect(frame.status == "ok", "depth camera produces a frame");

    // Center ray is horizontal at y=1 and should hit the slab front face at
    // x=9 -> range 9.
    const std::size_t center =
        static_cast<std::size_t>(4) * static_cast<std::size_t>(frame.width) + 4;
    expect(std::abs(frame.depth_m[center] - 9.0) < 1e-9,
           "center ray range matches wall distance exactly");

    bool has_ground = false;
    bool has_obstacle = false;
    for (std::size_t i = 0; i < frame.cloud.size(); ++i) {
        if (frame.cloud.classes[i] == agbot::nav::kClassObstacle) {
            has_obstacle = true;
            expect(frame.cloud.points[i].x >= 9.0 - 1e-6, "obstacle points lie on the slab");
            if (failures > 0) {
                return;
            }
        } else {
            has_ground = true;
        }
    }
    expect(has_ground && has_obstacle, "frame contains ground and obstacle returns");

    // Determinism: identical sensor re-created from the same params yields an
    // identical first frame.
    agbot::nav::DepthCameraSensor sensor_again(params);
    const agbot::nav::SensorFrame frame_again = sensor_again.sense(world, state, 0.0);
    expect(frame.depth_m == frame_again.depth_m, "depth frames are deterministic");
}

void test_perception_segmentation() {
    agbot::nav::SensorFrame frame;
    frame.cloud.points = {
        {1.0, 0.02, 0.0}, // ground
        {2.0, 0.05, 0.0}, // ground
        {3.0, 1.4, 0.0},  // obstacle
        {4.0, 0.5, 1.0},  // obstacle
    };
    frame.cloud.classes.assign(4, 0);

    agbot::config::ParamTable params;
    params["ground_height_m"] = 0.0;
    params["max_step_m"] = 0.15;
    agbot::nav::HeightThresholdGroundSeg height_seg(params);
    const agbot::nav::PerceptionResult by_height = height_seg.segment(frame);
    expect(by_height.obstacles.size() == 2, "height threshold keeps two obstacle points");
    expect(by_height.labeled.size() == 4, "height threshold labels every point");

    agbot::config::ParamTable grid_params;
    grid_params["cell_size_m"] = 0.5;
    grid_params["step_threshold_m"] = 0.15;
    grid_params["ground_band_m"] = 0.3;
    agbot::nav::GridStepGroundSeg grid_seg(grid_params);
    const agbot::nav::PerceptionResult by_grid = grid_seg.segment(frame);
    expect(by_grid.obstacles.size() == 2, "grid step segmentation finds the raised points");
}

void test_mapper_and_inflation_profile() {
    agbot::config::ParamTable params;
    params["origin_x"] = 0.0;
    params["origin_z"] = 0.0;
    params["width"] = 21;
    params["height"] = 21;
    params["resolution_m"] = 0.25;
    params["hit_increment"] = 254;
    agbot::nav::OccupancyGridMapper mapper(params);

    agbot::nav::PointCloud hits;
    hits.points.push_back({2.625, 1.0, 2.625}); // cell (10, 10) center
    hits.classes.push_back(agbot::nav::kClassObstacle);
    mapper.integrate(hits, {0.0, 0.0, 0.0}, 0.0);
    expect(mapper.grid().at(10, 10) == OccupancyGrid::kLethal,
           "mapper marks the hit cell lethal");

    agbot::config::ParamTable inflate_params;
    inflate_params["inflation_radius_m"] = 1.0;
    inflate_params["cost_scaling"] = 2.0;
    inflate_params["lethal_threshold"] = 200;
    const agbot::nav::InflationLayer inflation(inflate_params);
    const Costmap costmap = inflation.inflate(mapper.grid());

    const auto expected_cost = [](double distance_m) {
        return static_cast<std::uint8_t>(std::round(253.0 * std::exp(-2.0 * distance_m)));
    };
    expect(costmap.at(10, 10) == OccupancyGrid::kLethal, "inflation keeps the lethal center");
    expect(costmap.at(11, 10) == expected_cost(0.25), "inflation cost at 0.25 m matches profile");
    expect(costmap.at(12, 10) == expected_cost(0.50), "inflation cost at 0.50 m matches profile");
    expect(costmap.at(14, 10) == expected_cost(1.00), "inflation cost at 1.00 m matches profile");
    expect(costmap.at(15, 10) == 0, "cells beyond the inflation radius stay free");
    expect(costmap.at(11, 10) > costmap.at(12, 10)
               && costmap.at(12, 10) > costmap.at(14, 10),
           "inflation cost decays monotonically");
}

void test_astar_known_grid() {
    // 10x10 grid, 1 m cells. Lethal wall at cx=5 for cz=0..8; the only gap is
    // (5, 9). Optimal 8-connected path from (0,0) to (9,0) via (5,9) has
    // length 9*sqrt(2) + 9.
    Costmap grid;
    grid.origin_x = 0.0;
    grid.origin_z = 0.0;
    grid.resolution_m = 1.0;
    grid.width = 10;
    grid.height = 10;
    grid.reset(0);
    for (int cz = 0; cz <= 8; ++cz) {
        grid.set(5, cz, OccupancyGrid::kLethal);
    }

    agbot::config::ParamTable params;
    params["smooth"] = false;
    params["cost_weight"] = 0.0;
    agbot::nav::AStarPlanner planner(params);
    const agbot::nav::PlanResult result =
        planner.plan(grid, {0.5, 0.0, 0.5}, {9.5, 0.0, 0.5});
    expect(result.ok, "A* finds a path through the gap");
    const double expected = 9.0 * std::sqrt(2.0) + 9.0;
    expect(std::abs(result.path.length_m() - expected) < 1e-6,
           "A* path length equals the known optimum");
    for (const Vec3& point : result.path.points) {
        expect(grid.cost_at_world(point.x, point.z) < OccupancyGrid::kLethal,
               "A* waypoint avoids lethal cells");
        if (failures > 0) {
            return;
        }
    }

    // Smoothing shortens (or preserves) the path and stays collision free.
    agbot::config::ParamTable smooth_params;
    smooth_params["smooth"] = true;
    smooth_params["cost_weight"] = 0.0;
    agbot::nav::AStarPlanner smoothing_planner(smooth_params);
    const agbot::nav::PlanResult smoothed =
        smoothing_planner.plan(grid, {0.5, 0.0, 0.5}, {9.5, 0.0, 0.5});
    expect(smoothed.ok && smoothed.path.length_m() <= result.path.length_m() + 1e-9,
           "string pulling does not lengthen the path");
    expect(smoothed.path.points.size() < result.path.points.size(),
           "string pulling removes intermediate waypoints");
}

void test_pure_pursuit_steering_direction() {
    Costmap costmap;
    costmap.origin_x = -5.0;
    costmap.origin_z = -5.0;
    costmap.resolution_m = 0.25;
    costmap.width = 80;
    costmap.height = 80;
    costmap.reset(0);

    agbot::vehicles::VehicleLimits limits;
    limits.wheelbase_m = 0.8;
    EntityState state; // origin, yaw 0

    Path left_curve; // curves toward +z
    for (int i = 0; i <= 10; ++i) {
        const double x = 0.5 * static_cast<double>(i);
        left_curve.points.push_back({x, 0.0, 0.08 * x * x});
    }
    agbot::nav::PurePursuitPlanner planner;
    const agbot::nav::LocalPlan left_plan =
        planner.compute(costmap, left_curve, state, limits, left_curve.points.back());
    expect(left_plan.ok && left_plan.steer_cmd > 0.0,
           "pure pursuit steers +z toward a +z-curving path");

    Path right_curve;
    for (int i = 0; i <= 10; ++i) {
        const double x = 0.5 * static_cast<double>(i);
        right_curve.points.push_back({x, 0.0, -0.08 * x * x});
    }
    const agbot::nav::LocalPlan right_plan =
        planner.compute(costmap, right_curve, state, limits, right_curve.points.back());
    expect(right_plan.ok && right_plan.steer_cmd < 0.0,
           "pure pursuit steers -z toward a -z-curving path");
    expect(left_plan.v_cmd > 0.0 && left_plan.v_cmd <= limits.max_speed_mps,
           "pure pursuit commands a bounded positive speed");
}

void test_dwa_avoids_blocked_straight_path() {
    Costmap costmap;
    costmap.origin_x = -5.0;
    costmap.origin_z = -5.0;
    costmap.resolution_m = 0.25;
    costmap.width = 80;
    costmap.height = 80;
    costmap.reset(0);
    // Lethal wall at x in [2, 2.5] covering z in [-5, 1]; opening at z > 1.
    for (int cz = 0; cz < costmap.height; ++cz) {
        for (int cx = 0; cx < costmap.width; ++cx) {
            const Vec3 center = costmap.cell_to_world(cx, cz);
            if (center.x >= 2.0 && center.x <= 2.5 && center.z <= 1.0) {
                costmap.set(cx, cz, OccupancyGrid::kLethal);
            }
        }
    }

    Path straight; // deliberately drives through the wall
    for (int i = 0; i <= 16; ++i) {
        straight.points.push_back({0.5 * static_cast<double>(i), 0.0, 0.0});
    }
    agbot::vehicles::VehicleLimits limits;
    limits.wheelbase_m = 0.8;
    limits.max_steer_rad = 0.6;
    EntityState state;
    state.velocity = {1.5, 0.0, 0.0};

    expect(!agbot::nav::segment_is_traversable(costmap, {0.0, 0.0, 0.0}, {4.0, 0.0, 0.0}, 200, 0),
           "straight line ahead is actually blocked");

    agbot::config::ParamTable params;
    params["cruise_speed_mps"] = 3.0;
    params["horizon_s"] = 1.6;
    agbot::nav::DwaPlanner planner(params);
    const agbot::nav::LocalPlan plan =
        planner.compute(costmap, straight, state, limits, {8.0, 0.0, 0.0});
    expect(plan.ok, "DWA finds a collision-free rollout despite the blocked path");
    for (const agbot::nav::TrajectoryPoint& point : plan.trajectory.points) {
        const std::uint8_t cost = costmap.cost_at_world(point.position.x, point.position.z);
        expect(cost == OccupancyGrid::kUnknown || cost < 200,
               "DWA rollout stays clear of lethal cells");
        if (failures > 0) {
            return;
        }
    }
}

void test_stanley_converges_from_offset() {
    Path straight;
    straight.points.push_back({-5.0, 0.0, 0.0});
    straight.points.push_back({80.0, 0.0, 0.0});

    agbot::config::ParamTable vehicle_params;
    vehicle_params["wheelbase_m"] = 0.8;
    vehicle_params["max_steer_rad"] = 0.6;
    vehicle_params["max_steer_rate_radps"] = 4.0;
    vehicle_params["max_speed_mps"] = 5.0;
    agbot::vehicles::KinematicBicycleModel vehicle(vehicle_params);

    agbot::config::ParamTable controller_params;
    controller_params["k_e"] = 1.2;
    controller_params["k_soft"] = 1.0;
    agbot::nav::PidStanleyController controller(controller_params);

    EntityState state;
    state.position = {0.0, 0.0, 2.0}; // 2 m lateral offset
    state.velocity = {3.0, 0.0, 0.0};

    bool converged = false;
    double converged_at_x = 0.0;
    while (state.position.x < 60.0) {
        const agbot::vehicles::Actuation actuation =
            controller.control(state, straight, 3.0, vehicle.limits(), 0.02);
        state = vehicle.step(state, actuation, 0.02);
        if (!converged && std::abs(state.position.z) < 0.2) {
            converged = true;
            converged_at_x = state.position.x;
        }
        if (converged) {
            expect(std::abs(state.position.z) < 0.35, "stanley does not diverge after converging");
            if (failures > 0) {
                return;
            }
        }
    }
    expect(converged, "stanley converges lateral error below 0.2 m");
    expect(converged_at_x < 40.0, "stanley converges within 40 m of travel");
}

// ---------------------------------------------------------------------------
// Integration scenario
// ---------------------------------------------------------------------------

std::string pipeline_config_text(const std::string& local_algorithm, double inflation_radius_m) {
    std::ostringstream toml;
    toml << R"toml(
[pipeline]
sensor_period_s = 0.1
map_period_s = 0.1
plan_period_s = 1.0
local_period_s = 0.05
goal_tolerance_m = 1.0

[vehicle]
algorithm = "kinematic_bicycle"
max_speed_mps = 3.0
max_accel_mps2 = 2.5
max_brake_mps2 = 4.0
max_steer_rad = 0.6
max_steer_rate_radps = 2.5
wheelbase_m = 0.8

[sensing]
algorithm = "depth_camera"
width = 32
height = 24
horizontal_fov_deg = 90.0
vertical_fov_deg = 60.0
max_range_m = 30.0
mount_height_m = 0.5
seed = 7

[perception]
algorithm = "height_threshold"
ground_height_m = 0.0
max_step_m = 0.15

[mapping]
algorithm = "occupancy_grid"
origin_x = -10.0
origin_z = -10.0
width = 400
height = 80
resolution_m = 0.25
hit_increment = 128
cost_scaling = 3.0
lethal_threshold = 200
)toml";
    toml << "inflation_radius_m = " << inflation_radius_m << "\n";
    toml << R"toml(
[global]
algorithm = "astar"
lethal_threshold = 200
heuristic_weight = 1.0
cost_weight = 4.0
unknown_cost = 0
smooth = true
)toml";
    toml << "\n[local]\nalgorithm = \"" << local_algorithm << "\"\n";
    toml << R"toml(
lookahead_m = 2.5
cruise_speed_mps = 2.5
curvature_gain = 1.5
goal_slow_gain = 0.8
min_speed_mps = 0.4
horizon_s = 1.5
rollout_dt_s = 0.1
lethal_threshold = 130
w_obstacle = 4.0

[control]
algorithm = "pid_stanley"
kp = 1.5
ki = 0.3
integral_limit = 2.0
k_e = 1.2
k_soft = 1.0
)toml";
    return toml.str();
}

struct ScenarioRun {
    bool built = false;
    bool reached = false;
    double time_to_goal_s = 0.0;
    double traveled_m = 0.0;
    double final_path_length_m = 0.0;
    double min_clearance_m = 1e9;
    std::size_t lethal_traversals = 0;
    std::uint64_t param_hash = 0;
    EntityState final_state;
    std::vector<agbot::nav::NavTelemetry> telemetry;
};

ScenarioRun run_corridor_scenario(const std::string& config_text, double max_time_s = 90.0) {
    ScenarioRun run;
    const agbot::nav::NavigationPipelineConfig config =
        agbot::nav::parse_pipeline_config(config_text);
    agbot::nav::NavigationPipeline pipeline(config);
    if (!pipeline.ok()) {
        std::cout << "  pipeline error: " << pipeline.error() << "\n";
        return run;
    }
    run.built = true;
    run.param_hash = pipeline.param_hash();

    const NavWorld world = make_corridor_world();
    const Vec3 goal{80.0, 0.0, 0.0};
    EntityState state; // (0, 0, 0), yaw 0
    const double dt = 0.02;

    Vec3 previous = state.position;
    double elapsed = 0.0;
    while (elapsed < max_time_s) {
        pipeline.tick(world, state, goal, dt);
        elapsed += dt;
        run.traveled_m += (state.position - previous).horizontal_length();
        previous = state.position;

        const agbot::nav::NavTelemetry& sample = pipeline.telemetry().back();
        if (sample.robot_cell_cost != OccupancyGrid::kUnknown
            && sample.robot_cell_cost >= OccupancyGrid::kLethal) {
            ++run.lethal_traversals;
        }
        if (sample.min_obstacle_distance_m > 0.0) {
            run.min_clearance_m = std::min(run.min_clearance_m, sample.min_obstacle_distance_m);
        }
        if (pipeline.goal_reached()) {
            run.reached = true;
            run.time_to_goal_s = elapsed;
            break;
        }
    }
    run.final_state = state;
    run.final_path_length_m = pipeline.global_path().length_m();
    run.telemetry = pipeline.telemetry();
    return run;
}

void report_scenario(const std::string& label, const ScenarioRun& run) {
    std::cout << "  [" << label << "] reached=" << (run.reached ? "yes" : "no")
              << " time_to_goal_s=" << run.time_to_goal_s
              << " traveled_m=" << run.traveled_m
              << " global_path_m=" << run.final_path_length_m
              << " min_clearance_m=" << run.min_clearance_m
              << " lethal_traversals=" << run.lethal_traversals << "\n";
}

void test_integration_pure_pursuit() {
    const ScenarioRun run = run_corridor_scenario(pipeline_config_text("pure_pursuit", 1.0));
    report_scenario("pure_pursuit", run);
    expect(run.built, "pipeline builds from TOML config");
    expect(run.reached, "pure pursuit run reaches the goal");
    expect(run.time_to_goal_s < 90.0, "goal reached within the time budget");
    expect(run.lethal_traversals == 0, "robot never occupies a lethal costmap cell");
    expect(run.traveled_m < 160.0, "traveled distance stays under 2x straight line");
    expect(run.final_path_length_m > 0.0 && run.final_path_length_m < 160.0,
           "global path length is sane");
    expect(run.min_clearance_m > 0.3, "sensed clearance never collapses to a collision");
}

void test_integration_determinism() {
    const std::string config = pipeline_config_text("pure_pursuit", 1.0);
    const ScenarioRun first = run_corridor_scenario(config);
    const ScenarioRun second = run_corridor_scenario(config);
    expect(first.reached && second.reached, "both identical runs reach the goal");
    expect(first.final_state.position.x == second.final_state.position.x
               && first.final_state.position.z == second.final_state.position.z
               && first.final_state.yaw_rad == second.final_state.yaw_rad,
           "identical runs produce bit-identical final pose");
    expect(first.telemetry.size() == second.telemetry.size(),
           "identical runs produce identical tick counts");
}

void test_integration_hot_swap_local_planner() {
    const ScenarioRun dwa_run = run_corridor_scenario(pipeline_config_text("dwa", 1.0));
    report_scenario("dwa", dwa_run);
    expect(dwa_run.built, "dwa pipeline builds from the same config shape");
    expect(dwa_run.reached, "dwa local planner also reaches the goal");
    expect(dwa_run.lethal_traversals == 0, "dwa run never occupies a lethal cell");
    expect(dwa_run.traveled_m < 160.0, "dwa traveled distance stays sane");
}

void test_integration_param_change_changes_hash_and_route() {
    const std::string narrow = pipeline_config_text("pure_pursuit", 1.0);
    const std::string wide = pipeline_config_text("pure_pursuit", 2.0);
    const ScenarioRun narrow_run = run_corridor_scenario(narrow);
    const ScenarioRun wide_run = run_corridor_scenario(wide);
    report_scenario("inflation_2.0", wide_run);
    expect(narrow_run.param_hash != wide_run.param_hash,
           "changing inflation radius changes the recorded param_hash");
    expect(wide_run.reached, "wider inflation still reaches the goal");
    expect(wide_run.lethal_traversals == 0, "wider inflation run stays lethal-free");

    bool route_differs = false;
    const std::size_t shared =
        std::min(narrow_run.telemetry.size(), wide_run.telemetry.size());
    for (std::size_t i = 0; i < shared; ++i) {
        const double dx = narrow_run.telemetry[i].pose.x - wide_run.telemetry[i].pose.x;
        const double dz = narrow_run.telemetry[i].pose.z - wide_run.telemetry[i].pose.z;
        if (std::sqrt(dx * dx + dz * dz) > 0.05) {
            route_differs = true;
            break;
        }
    }
    expect(route_differs, "inflation change produces a different-but-valid route");
}

void test_registries_expose_algorithms() {
    expect(agbot::nav::default_sensor_registry().contains("depth_camera"),
           "sensor registry exposes depth_camera");
    expect(agbot::nav::default_perception_registry().contains("height_threshold")
               && agbot::nav::default_perception_registry().contains("grid_step"),
           "perception registry exposes both segmenters");
    expect(agbot::nav::default_mapper_registry().contains("occupancy_grid"),
           "mapper registry exposes occupancy_grid");
    expect(agbot::nav::default_global_planner_registry().contains("astar"),
           "global planner registry exposes astar");
    expect(agbot::nav::default_local_planner_registry().contains("pure_pursuit")
               && agbot::nav::default_local_planner_registry().contains("dwa"),
           "local planner registry exposes pure_pursuit and dwa");
    expect(agbot::nav::default_controller_registry().contains("pid_stanley"),
           "controller registry exposes pid_stanley");
}

} // namespace

int main() {
    test_registries_expose_algorithms();
    test_depth_camera_geometry();
    test_perception_segmentation();
    test_mapper_and_inflation_profile();
    test_astar_known_grid();
    test_pure_pursuit_steering_direction();
    test_dwa_avoids_blocked_straight_path();
    test_stanley_converges_from_offset();
    test_integration_pure_pursuit();
    test_integration_determinism();
    test_integration_hot_swap_local_planner();
    test_integration_param_change_changes_hash_and_route();

    if (failures > 0) {
        std::cout << failures << " failure(s)\n";
        return 1;
    }
    std::cout << "all nav tests passed\n";
    return 0;
}
