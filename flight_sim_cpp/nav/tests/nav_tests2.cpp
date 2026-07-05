// Tests for the second navigation batch: Hybrid-A* global planning, MPPI
// local planning and the localization stage (ground_truth / ekf_2d).
// Existing suites in nav_tests.cpp are untouched.

#include "agbot_nav/Dubins2D.hpp"
#include "agbot_nav/GlobalPlanner.hpp"
#include "agbot_nav/HybridAStarPlanner.hpp"
#include "agbot_nav/LocalPlanner.hpp"
#include "agbot_nav/Localization.hpp"
#include "agbot_nav/MppiPlanner.hpp"
#include "agbot_nav/NavigationPipeline.hpp"
#include "agbot_vehicles/VehicleTypes.hpp"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstring>
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
using agbot::flight_sim::SceneSynthesisStatus;
using agbot::nav::Costmap;
using agbot::nav::NavWorld;
using agbot::nav::OccupancyGrid;
using agbot::nav::Path;
using agbot::nav::Pose2D;
using agbot::nav::Vec3;
using agbot::nav::dubins2d::angle_diff;
using agbot::vehicles::EntityState;

constexpr double kPi = 3.14159265358979323846;

// ---------------------------------------------------------------------------
// Shared fixtures
// ---------------------------------------------------------------------------

Costmap make_costmap(double origin_x, double origin_z, int width, int height, double res) {
    Costmap grid;
    grid.origin_x = origin_x;
    grid.origin_z = origin_z;
    grid.resolution_m = res;
    grid.width = width;
    grid.height = height;
    grid.reset(0);
    return grid;
}

void fill_lethal(Costmap& grid, double min_x, double max_x, double min_z, double max_z) {
    for (int cz = 0; cz < grid.height; ++cz) {
        for (int cx = 0; cx < grid.width; ++cx) {
            const Vec3 center = grid.cell_to_world(cx, cz);
            if (center.x >= min_x && center.x <= max_x && center.z >= min_z
                && center.z <= max_z) {
                grid.set(cx, cz, OccupancyGrid::kLethal);
            }
        }
    }
}

// Corridor costmap mirroring the nav_tests world: side walls at |z| in
// [4, 7], offset obstacle at x in [38, 42] blocking z in [-4, 1].
Costmap make_corridor_costmap() {
    Costmap grid = make_costmap(-10.0, -10.0, 400, 80, 0.25);
    fill_lethal(grid, -5.0, 85.0, 4.0, 7.0);
    fill_lethal(grid, -5.0, 85.0, -7.0, -4.0);
    fill_lethal(grid, 38.0, 42.0, -4.0, 1.0);
    return grid;
}

bool path_avoids_lethal(const Costmap& grid, const Path& path, std::uint8_t threshold) {
    for (const Vec3& point : path.points) {
        const std::uint8_t cost = grid.cost_at_world(point.x, point.z);
        if (cost != OccupancyGrid::kUnknown && cost >= threshold) {
            return false;
        }
    }
    return true;
}

// Max curvature estimated from consecutive segment headings, skipping
// segments too short for a stable heading.
double max_path_curvature(const Path& path) {
    double max_curvature = 0.0;
    double prev_heading = 0.0;
    double prev_length = 0.0;
    bool has_prev = false;
    for (std::size_t i = 1; i < path.points.size(); ++i) {
        const double dx = path.points[i].x - path.points[i - 1].x;
        const double dz = path.points[i].z - path.points[i - 1].z;
        const double length = std::sqrt(dx * dx + dz * dz);
        if (length < 1e-6) {
            continue;
        }
        const double heading = std::atan2(dz, dx);
        if (has_prev) {
            const double ds = 0.5 * (length + prev_length);
            max_curvature =
                std::max(max_curvature, std::abs(angle_diff(heading, prev_heading)) / ds);
        }
        prev_heading = heading;
        prev_length = length;
        has_prev = true;
    }
    return max_curvature;
}

double path_end_heading(const Path& path) {
    for (std::size_t i = path.points.size(); i-- > 1;) {
        const double dx = path.points[i].x - path.points[i - 1].x;
        const double dz = path.points[i].z - path.points[i - 1].z;
        if (std::sqrt(dx * dx + dz * dz) > 1e-6) {
            return std::atan2(dz, dx);
        }
    }
    return 0.0;
}

std::uint64_t hash_doubles(const std::vector<double>& values) {
    std::uint64_t hash = 0xcbf29ce484222325ULL;
    for (const double value : values) {
        std::uint64_t bits = 0;
        std::memcpy(&bits, &value, sizeof(bits));
        for (int b = 0; b < 8; ++b) {
            hash ^= (bits >> (8 * b)) & 0xffULL;
            hash *= 0x100000001b3ULL;
        }
    }
    return hash;
}

std::uint64_t hash_local_plan(const agbot::nav::LocalPlan& plan) {
    std::vector<double> values{plan.v_cmd, plan.steer_cmd};
    for (const agbot::nav::TrajectoryPoint& point : plan.trajectory.points) {
        values.push_back(point.position.x);
        values.push_back(point.position.z);
        values.push_back(point.yaw);
        values.push_back(point.v);
    }
    return hash_doubles(values);
}

// ---------------------------------------------------------------------------
// Hybrid-A*
// ---------------------------------------------------------------------------

agbot::config::ParamTable hybrid_params(double radius_m) {
    agbot::config::ParamTable params;
    params["min_turn_radius_m"] = radius_m;
    params["arc_length_m"] = 1.0;
    params["n_headings"] = 24;
    params["lethal_threshold"] = 200;
    params["goal_xy_tolerance_m"] = 1.0;
    params["goal_yaw_tolerance_rad"] = 0.35;
    return params;
}

void test_hybrid_astar_min_turn_radius() {
    const Costmap grid = make_costmap(0.0, 0.0, 160, 160, 0.25); // 40 x 40 m, free
    const double radius = 3.0;
    agbot::nav::HybridAStarPlanner planner(hybrid_params(radius));

    // Goal requires a quarter-turn to the left.
    const agbot::nav::PlanResult result =
        planner.plan_poses(grid, {5.0, 5.0, 0.0}, {25.0, 22.0, kPi / 2.0});
    expect(result.ok, "hybrid A* plans a turning maneuver in free space");
    if (!result.ok) {
        return;
    }
    const double max_curvature = max_path_curvature(result.path);
    std::cout << "  [hybrid_astar] max_curvature=" << max_curvature
              << " limit=" << 1.0 / radius << " points=" << result.path.points.size() << "\n";
    expect(max_curvature <= 1.05 / radius + 1e-9,
           "path curvature never exceeds 1/min_turn_radius");
    expect(std::abs(angle_diff(path_end_heading(result.path), kPi / 2.0)) < 0.35,
           "goal heading is honored within tolerance");
}

void test_hybrid_astar_corridor() {
    const Costmap grid = make_corridor_costmap();
    const double radius = 2.0;
    agbot::nav::HybridAStarPlanner planner(hybrid_params(radius));
    const agbot::nav::PlanResult result =
        planner.plan_poses(grid, {0.0, 0.0, 0.0}, {60.0, 0.0, 0.0});
    expect(result.ok, "hybrid A* threads the corridor and offset obstacle");
    if (!result.ok) {
        std::cout << "  reason: " << result.reason << "\n";
        return;
    }
    expect(path_avoids_lethal(grid, result.path, 200),
           "hybrid A* corridor path avoids lethal cells");
    const double max_curvature = max_path_curvature(result.path);
    std::cout << "  [hybrid_astar corridor] length_m=" << result.path.length_m()
              << " max_curvature=" << max_curvature << " limit=" << 1.0 / radius << "\n";
    expect(max_curvature <= 1.05 / radius + 1e-9,
           "corridor path respects the kinematic curvature bound");
    expect(std::abs(angle_diff(path_end_heading(result.path), 0.0)) < 0.35,
           "corridor goal heading within tolerance");
    const Vec3& last = result.path.points.back();
    expect((Vec3{60.0, 0.0, 0.0} - last).horizontal_length() < 1.1,
           "corridor path terminates at the goal");
}

void test_hybrid_astar_reverse_dead_end() {
    // Dead-end pocket 2 m wide: front wall ahead of the robot, side walls too
    // close to turn around with R = 2 m. The goal sits directly behind.
    Costmap grid = make_costmap(0.0, 0.0, 80, 80, 0.25); // 20 x 20 m
    fill_lethal(grid, 10.0, 11.0, 0.0, 20.0);            // front wall
    fill_lethal(grid, 2.0, 11.0, 0.0, 9.0);              // south wall
    fill_lethal(grid, 2.0, 11.0, 11.0, 20.0);            // north wall

    const Pose2D start{8.0, 10.0, 0.0};
    const Pose2D goal{5.0, 10.0, 0.0};

    agbot::config::ParamTable forward_only = hybrid_params(2.0);
    forward_only["allow_reverse"] = false;
    forward_only["goal_xy_tolerance_m"] = 0.5;
    agbot::nav::HybridAStarPlanner forward_planner(forward_only);
    const agbot::nav::PlanResult forward_result = forward_planner.plan_poses(grid, start, goal);
    expect(!forward_result.ok,
           "forward-only planner cannot reach a goal directly behind in a dead-end");

    agbot::config::ParamTable with_reverse = hybrid_params(2.0);
    with_reverse["allow_reverse"] = true;
    with_reverse["goal_xy_tolerance_m"] = 0.5;
    agbot::nav::HybridAStarPlanner reverse_planner(with_reverse);
    const agbot::nav::PlanResult reverse_result = reverse_planner.plan_poses(grid, start, goal);
    expect(reverse_result.ok, "allow_reverse=true finds the backing maneuver");
    if (reverse_result.ok) {
        expect(path_avoids_lethal(grid, reverse_result.path, 200),
               "reverse maneuver stays collision free");
        const Vec3& last = reverse_result.path.points.back();
        expect((Vec3{goal.x, 0.0, goal.z} - last).horizontal_length() < 0.6,
               "reverse maneuver ends at the goal");
    }
}

void test_hybrid_astar_determinism() {
    const Costmap grid = make_corridor_costmap();
    agbot::nav::HybridAStarPlanner first(hybrid_params(2.0));
    agbot::nav::HybridAStarPlanner second(hybrid_params(2.0));
    const agbot::nav::PlanResult a = first.plan_poses(grid, {0.0, 0.0, 0.0}, {60.0, 0.0, 0.0});
    const agbot::nav::PlanResult b = second.plan_poses(grid, {0.0, 0.0, 0.0}, {60.0, 0.0, 0.0});
    expect(a.ok && b.ok, "both hybrid A* runs succeed");
    bool identical = a.path.points.size() == b.path.points.size();
    if (identical) {
        for (std::size_t i = 0; i < a.path.points.size(); ++i) {
            if (a.path.points[i].x != b.path.points[i].x
                || a.path.points[i].z != b.path.points[i].z) {
                identical = false;
                break;
            }
        }
    }
    expect(identical, "hybrid A* is bit-identical across runs");
}

// ---------------------------------------------------------------------------
// MPPI
// ---------------------------------------------------------------------------

agbot::config::ParamTable mppi_params(std::int64_t seed, double lambda) {
    agbot::config::ParamTable params;
    params["time_steps"] = 30;
    params["dt"] = 0.05;
    params["num_samples"] = 512;
    params["lambda"] = lambda;
    params["sigma_accel"] = 1.0;
    params["sigma_steer_rate"] = 1.5;
    params["cruise_speed_mps"] = 2.5;
    params["lethal_threshold"] = 130;
    params["w_obstacle"] = 4.0;
    params["w_path"] = 1.5;
    params["w_goal"] = 0.8;
    params["w_speed"] = 0.3;
    params["w_smooth"] = 0.05;
    params["goal_slow_gain"] = 0.8;
    params["seed"] = seed;
    return params;
}

void test_mppi_avoids_blocked_straight_path() {
    Costmap costmap = make_costmap(-5.0, -5.0, 80, 80, 0.25);
    fill_lethal(costmap, 2.0, 2.5, -5.0, 1.0); // opening at z > 1

    Path straight; // deliberately drives through the wall
    for (int i = 0; i <= 16; ++i) {
        straight.points.push_back({0.5 * static_cast<double>(i), 0.0, 0.0});
    }
    agbot::vehicles::VehicleLimits limits;
    limits.wheelbase_m = 0.8;
    limits.max_steer_rad = 0.6;
    EntityState state;
    state.velocity = {1.5, 0.0, 0.0};

    // Long horizon and a strong goal critic so lateral escape beats braking
    // in front of the wall (the straight global path is deliberately wrong,
    // so the path critic is de-weighted).
    agbot::config::ParamTable params = mppi_params(9, 0.3);
    params["lethal_threshold"] = 200;
    params["time_steps"] = 50;
    params["sigma_steer_rate"] = 2.5;
    params["w_path"] = 0.3;
    params["w_goal"] = 2.0;
    agbot::nav::MppiPlanner planner(params);
    agbot::nav::LocalPlan plan;
    for (int iteration = 0; iteration < 4; ++iteration) {
        plan = planner.compute(costmap, straight, state, limits, {8.0, 0.0, 0.0});
    }
    expect(plan.ok, "MPPI produces a plan despite the blocked straight path");
    double max_cost = 0.0;
    for (const agbot::nav::TrajectoryPoint& point : plan.trajectory.points) {
        const std::uint8_t cost = costmap.cost_at_world(point.position.x, point.position.z);
        if (cost != OccupancyGrid::kUnknown) {
            max_cost = std::max(max_cost, static_cast<double>(cost));
        }
    }
    const Vec3 final_position = plan.trajectory.points.back().position;
    std::cout << "  [mppi obstacle] steer_cmd=" << plan.steer_cmd
              << " max_traj_cost=" << max_cost << " final=(" << final_position.x << ", "
              << final_position.z << ")\n";
    expect(max_cost < 200.0, "MPPI trajectory stays below the lethal threshold");
    expect(plan.steer_cmd > 0.0, "MPPI steers toward the +z opening");
    expect(final_position.x > 2.5, "MPPI trajectory makes it past the wall");
}

void test_mppi_determinism_and_lambda_sensitivity() {
    const Costmap costmap = make_costmap(-5.0, -5.0, 80, 80, 0.25);
    Path straight;
    for (int i = 0; i <= 16; ++i) {
        straight.points.push_back({0.5 * static_cast<double>(i), 0.0, 0.0});
    }
    agbot::vehicles::VehicleLimits limits;
    limits.wheelbase_m = 0.8;
    EntityState state;
    state.velocity = {1.0, 0.0, 0.0};
    const Vec3 goal{8.0, 0.0, 0.0};

    agbot::nav::MppiPlanner first(mppi_params(42, 0.3));
    agbot::nav::MppiPlanner second(mppi_params(42, 0.3));
    std::uint64_t hash_a = 0;
    std::uint64_t hash_b = 0;
    for (int call = 0; call < 3; ++call) {
        hash_a = hash_local_plan(first.compute(costmap, straight, state, limits, goal));
        hash_b = hash_local_plan(second.compute(costmap, straight, state, limits, goal));
    }
    expect(hash_a == hash_b, "identical seeds give bit-identical MPPI output");

    agbot::nav::MppiPlanner hot(mppi_params(42, 2.0));
    std::uint64_t hash_hot = 0;
    for (int call = 0; call < 3; ++call) {
        hash_hot = hash_local_plan(hot.compute(costmap, straight, state, limits, goal));
    }
    std::cout << "  [mppi determinism] hash(lambda=0.3)=" << hash_a
              << " hash(lambda=2.0)=" << hash_hot << "\n";
    expect(hash_a != hash_hot, "changing lambda changes the MPPI solution");
}

// ---------------------------------------------------------------------------
// Pipeline integration (corridor scenario, mirroring nav_tests.cpp)
// ---------------------------------------------------------------------------

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

NavWorld make_corridor_world() {
    NavWorld world;
    world.ground_height_m = 0.0;
    world.scene.status = SceneSynthesisStatus::Ready;
    world.scene.scene_hash = "nav-test2-corridor";
    world.scene.objects.push_back(make_box("wall_north", -5.0, 85.0, 4.0, 7.0, 3.0));
    world.scene.objects.push_back(make_box("wall_south", -5.0, 85.0, -7.0, -4.0, 3.0));
    world.scene.objects.push_back(make_box("offset_obstacle", 38.0, 42.0, -4.0, 1.0, 3.0));
    world.scene.object_count = world.scene.objects.size();
    return world;
}

std::string pipeline_config_text(
    const std::string& local_block,
    const std::string& localization_block) {
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
inflation_radius_m = 1.0

[global]
algorithm = "astar"
lethal_threshold = 200
heuristic_weight = 1.0
cost_weight = 4.0
unknown_cost = 0
smooth = true

[control]
algorithm = "pid_stanley"
kp = 1.5
ki = 0.3
integral_limit = 2.0
k_e = 1.2
k_soft = 1.0
)toml";
    toml << "\n[local]\n" << local_block << "\n";
    if (!localization_block.empty()) {
        toml << "\n[localization]\n" << localization_block << "\n";
    }
    return toml.str();
}

const std::string kMppiLocalBlock = R"toml(algorithm = "mppi"
time_steps = 30
dt = 0.05
num_samples = 512
lambda = 0.3
sigma_accel = 1.0
sigma_steer_rate = 1.5
cruise_speed_mps = 2.5
lethal_threshold = 130
w_obstacle = 4.0
w_path = 1.5
w_goal = 0.8
w_speed = 0.3
w_smooth = 0.05
goal_slow_gain = 0.8
seed = 42)toml";

const std::string kDwaLocalBlock = R"toml(algorithm = "dwa"
cruise_speed_mps = 2.5
horizon_s = 1.5
rollout_dt_s = 0.1
lethal_threshold = 130
w_obstacle = 4.0
goal_slow_gain = 0.8
min_speed_mps = 0.4)toml";

const std::string kPurePursuitLocalBlock = R"toml(algorithm = "pure_pursuit"
lookahead_m = 2.5
cruise_speed_mps = 2.5
curvature_gain = 1.5
goal_slow_gain = 0.8
min_speed_mps = 0.4
horizon_s = 1.5
rollout_dt_s = 0.1)toml";

struct ScenarioRun {
    bool built = false;
    bool reached = false;
    double time_to_goal_s = 0.0;
    double traveled_m = 0.0;
    double min_clearance_m = 1e9;
    std::size_t lethal_traversals = 0;
    EntityState final_state;
    double final_estimate_error_m = 0.0;
};

// Optional canyon window [canyon_start_s, canyon_end_s) drives the pipeline's
// gps quality down to canyon_quality mid-run.
ScenarioRun run_corridor_scenario(
    const std::string& config_text,
    double max_time_s = 90.0,
    double canyon_start_s = -1.0,
    double canyon_end_s = -1.0,
    double canyon_quality = 1.0) {
    ScenarioRun run;
    const agbot::nav::NavigationPipelineConfig config =
        agbot::nav::parse_pipeline_config(config_text);
    agbot::nav::NavigationPipeline pipeline(config);
    if (!pipeline.ok()) {
        std::cout << "  pipeline error: " << pipeline.error() << "\n";
        return run;
    }
    run.built = true;

    const NavWorld world = make_corridor_world();
    const Vec3 goal{80.0, 0.0, 0.0};
    EntityState state;
    const double dt = 0.02;

    Vec3 previous = state.position;
    double elapsed = 0.0;
    while (elapsed < max_time_s) {
        const bool in_canyon = elapsed >= canyon_start_s && elapsed < canyon_end_s;
        pipeline.set_gps_quality(in_canyon ? canyon_quality : 1.0);
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
    const Pose2D estimate = pipeline.estimated_pose();
    const double ex = estimate.x - state.position.x;
    const double ez = estimate.z - state.position.z;
    run.final_estimate_error_m = std::sqrt(ex * ex + ez * ez);
    return run;
}

void report_scenario(const std::string& label, const ScenarioRun& run) {
    std::cout << "  [" << label << "] reached=" << (run.reached ? "yes" : "no")
              << " time_to_goal_s=" << run.time_to_goal_s
              << " traveled_m=" << run.traveled_m
              << " min_clearance_m=" << run.min_clearance_m
              << " lethal_traversals=" << run.lethal_traversals << "\n";
}

void test_integration_mppi_corridor() {
    const ScenarioRun mppi_run =
        run_corridor_scenario(pipeline_config_text(kMppiLocalBlock, ""));
    report_scenario("mppi", mppi_run);
    const ScenarioRun dwa_run =
        run_corridor_scenario(pipeline_config_text(kDwaLocalBlock, ""));
    report_scenario("dwa (reference)", dwa_run);

    expect(mppi_run.built, "mppi pipeline builds from TOML config");
    expect(mppi_run.reached, "mppi run reaches the corridor goal");
    expect(mppi_run.lethal_traversals == 0, "mppi run never occupies a lethal cell");
    expect(mppi_run.traveled_m < 160.0, "mppi traveled distance stays under 2x straight line");
    expect(mppi_run.min_clearance_m > 0.3, "mppi sensed clearance never collapses");
}

void test_integration_mppi_determinism() {
    const std::string config = pipeline_config_text(kMppiLocalBlock, "");
    const ScenarioRun first = run_corridor_scenario(config);
    const ScenarioRun second = run_corridor_scenario(config);
    expect(first.reached && second.reached, "both mppi runs reach the goal");
    expect(first.final_state.position.x == second.final_state.position.x
               && first.final_state.position.z == second.final_state.position.z
               && first.final_state.yaw_rad == second.final_state.yaw_rad,
           "mppi corridor runs are bit-identical");
}

// ---------------------------------------------------------------------------
// EKF localization
// ---------------------------------------------------------------------------

struct EkfRunResult {
    double ekf_rmse_m = 0.0;
    double gps_rmse_m = 0.0;
    double max_canyon_error_m = 0.0;
    double max_canyon_gps_error_m = 0.0;
    double final_error_m = 0.0;
    double variance_before_canyon = 0.0;
    double variance_end_canyon = 0.0;
    double variance_after_recovery = 0.0;
};

// Straight-line drive with synthetic GPS/compass/odometry; optional canyon
// window degrades gps quality to canyon_quality for t in [25, 35).
EkfRunResult run_ekf_drive(double duration_s, bool canyon, double canyon_quality) {
    const double dt = 0.05;
    const double gps_period = 0.2;
    const double gps_sigma = 1.2;
    const double compass_sigma = 0.05;
    const double odom_v_sigma = 0.05;
    const double odom_w_sigma = 0.01;
    const std::uint64_t seed = 1234;

    agbot::config::ParamTable params;
    params["sigma_odom_v"] = odom_v_sigma;
    params["sigma_odom_yaw_rate"] = odom_w_sigma;
    agbot::nav::Ekf2dLocalizer ekf(params);

    EntityState truth;
    ekf.observe_truth({0.0, 0.0, 0.0}, 2.0);

    EkfRunResult result;
    double ekf_sq_sum = 0.0;
    std::size_t ekf_samples = 0;
    double gps_sq_sum = 0.0;
    std::size_t gps_samples = 0;
    double gps_elapsed = 1e9;
    std::uint64_t counter = 0;
    double time_s = 0.0;

    while (time_s < duration_s) {
        const EntityState next = agbot::vehicles::bicycle_propagate(truth, 2.0, 0.0, 0.8, dt);
        const double dx = next.position.x - truth.position.x;
        const double dz = next.position.z - truth.position.z;
        const double forward = dx * std::cos(truth.yaw_rad) + dz * std::sin(truth.yaw_rad);
        const double dyaw = next.yaw_rad - truth.yaw_rad;
        truth = next;
        time_s += dt;
        ++counter;

        const double v_odom =
            forward / dt + odom_v_sigma * agbot::nav::noise::gaussian(seed, 1, counter);
        const double w_odom =
            dyaw / dt + odom_w_sigma * agbot::nav::noise::gaussian(seed, 2, counter);
        ekf.predict(v_odom, w_odom, dt);

        const bool in_canyon = canyon && time_s >= 25.0 && time_s < 35.0;
        const double quality = in_canyon ? canyon_quality : 1.0;
        gps_elapsed += dt;
        if (gps_elapsed + 1e-9 >= gps_period) {
            gps_elapsed = 0.0;
            const double sigma_eff = gps_sigma / quality;
            const double gx = truth.position.x
                + sigma_eff * agbot::nav::noise::gaussian(seed, 3, counter);
            const double gz = truth.position.z
                + sigma_eff * agbot::nav::noise::gaussian(seed, 4, counter);
            ekf.correct_gps(gx, gz, gps_sigma, quality);
            const double gerr = std::sqrt((gx - truth.position.x) * (gx - truth.position.x)
                                          + (gz - truth.position.z) * (gz - truth.position.z));
            gps_sq_sum += gerr * gerr;
            ++gps_samples;
            if (in_canyon) {
                result.max_canyon_gps_error_m = std::max(result.max_canyon_gps_error_m, gerr);
            }
        }
        const double compass = truth.yaw_rad
            + compass_sigma * agbot::nav::noise::gaussian(seed, 5, counter);
        ekf.correct_heading(compass, compass_sigma);

        const Pose2D estimate = ekf.pose();
        const double ex = estimate.x - truth.position.x;
        const double ez = estimate.z - truth.position.z;
        const double err = std::sqrt(ex * ex + ez * ez);
        ekf_sq_sum += err * err;
        ++ekf_samples;
        if (in_canyon) {
            result.max_canyon_error_m = std::max(result.max_canyon_error_m, err);
        }
        result.final_error_m = err;

        if (std::abs(time_s - 24.9) < dt * 0.5) {
            result.variance_before_canyon = ekf.position_variance();
        }
        if (std::abs(time_s - 34.9) < dt * 0.5) {
            result.variance_end_canyon = ekf.position_variance();
        }
    }
    result.variance_after_recovery = ekf.position_variance();
    result.ekf_rmse_m =
        ekf_samples > 0 ? std::sqrt(ekf_sq_sum / static_cast<double>(ekf_samples)) : 0.0;
    result.gps_rmse_m =
        gps_samples > 0 ? std::sqrt(gps_sq_sum / static_cast<double>(gps_samples)) : 0.0;
    return result;
}

void test_ekf_beats_raw_gps() {
    const EkfRunResult result = run_ekf_drive(40.0, false, 1.0);
    std::cout << "  [ekf straight] ekf_rmse_m=" << result.ekf_rmse_m
              << " gps_rmse_m=" << result.gps_rmse_m << "\n";
    expect(result.ekf_rmse_m < result.gps_rmse_m,
           "EKF position RMSE beats raw GPS RMSE on a straight drive");
    expect(result.ekf_rmse_m < 1.0, "EKF RMSE stays sub-meter with nominal GPS");
}

void test_ekf_urban_canyon() {
    const EkfRunResult result = run_ekf_drive(50.0, true, 0.05);
    std::cout << "  [ekf canyon] max_canyon_error_m=" << result.max_canyon_error_m
              << " max_canyon_gps_error_m=" << result.max_canyon_gps_error_m
              << " final_error_m=" << result.final_error_m << "\n";
    std::cout << "  [ekf canyon] var_before=" << result.variance_before_canyon
              << " var_end_canyon=" << result.variance_end_canyon
              << " var_after=" << result.variance_after_recovery << "\n";
    expect(result.max_canyon_gps_error_m > 5.0,
           "raw GPS error is huge during the canyon window");
    expect(result.max_canyon_error_m < 3.0,
           "EKF error stays bounded (< 3 m) through the canyon on odometry");
    expect(result.final_error_m < 1.0, "EKF recovers after the canyon window");
    expect(result.variance_end_canyon > result.variance_before_canyon,
           "covariance grows during the GPS outage");
    expect(result.variance_after_recovery < result.variance_end_canyon,
           "covariance shrinks again after GPS recovery");
}

void test_pipeline_with_ekf_localization() {
    const std::string localization_block = R"toml(algorithm = "ekf_2d"
sigma_odom_v = 0.05
sigma_odom_yaw_rate = 0.01
gps_sigma_m = 0.6
gps_period_s = 0.2
compass_sigma_rad = 0.03
compass_period_s = 0.1
odom_v_sigma = 0.05
odom_yaw_rate_sigma = 0.01
noise_seed = 11)toml";
    // A 6-second urban-canyon window mid-run exercises the pipeline's
    // set_gps_quality hook end-to-end; the EKF must bridge it on odometry.
    const ScenarioRun run = run_corridor_scenario(
        pipeline_config_text(kPurePursuitLocalBlock, localization_block),
        90.0, 12.0, 18.0, 0.05);
    report_scenario("pure_pursuit+ekf_2d (6 s canyon)", run);
    std::cout << "  [ekf pipeline] final_estimate_error_m=" << run.final_estimate_error_m
              << "\n";
    expect(run.built, "pipeline builds with the ekf_2d localization stage");
    expect(run.reached, "noisy-localization run still reaches the corridor goal");
    expect(run.lethal_traversals == 0, "ekf run never occupies a lethal cell");
    const Vec3 goal{80.0, 0.0, 0.0};
    expect((goal - run.final_state.position).horizontal_length() < 2.5,
           "true final position is near the goal despite estimation error");
    expect(run.final_estimate_error_m < 1.5, "final EKF estimate error stays small");
}

void test_default_localization_matches_legacy_behavior() {
    // No [localization] section vs explicit ground_truth: identical runs.
    const ScenarioRun implicit =
        run_corridor_scenario(pipeline_config_text(kPurePursuitLocalBlock, ""));
    const ScenarioRun explicit_gt = run_corridor_scenario(
        pipeline_config_text(kPurePursuitLocalBlock, "algorithm = \"ground_truth\""));
    expect(implicit.reached && explicit_gt.reached,
           "default and explicit ground_truth runs both reach the goal");
    expect(implicit.final_state.position.x == explicit_gt.final_state.position.x
               && implicit.final_state.position.z == explicit_gt.final_state.position.z,
           "ground_truth localization leaves the trajectory bit-identical");
}

void test_registries_expose_new_algorithms() {
    expect(agbot::nav::default_global_planner_registry().contains("hybrid_astar"),
           "global planner registry exposes hybrid_astar");
    expect(agbot::nav::default_local_planner_registry().contains("mppi"),
           "local planner registry exposes mppi");
    expect(agbot::nav::default_localizer_registry().contains("ground_truth")
               && agbot::nav::default_localizer_registry().contains("ekf_2d"),
           "localizer registry exposes ground_truth and ekf_2d");
}

} // namespace

int main() {
    test_registries_expose_new_algorithms();
    test_hybrid_astar_min_turn_radius();
    test_hybrid_astar_corridor();
    test_hybrid_astar_reverse_dead_end();
    test_hybrid_astar_determinism();
    test_mppi_avoids_blocked_straight_path();
    test_mppi_determinism_and_lambda_sensitivity();
    test_integration_mppi_corridor();
    test_integration_mppi_determinism();
    test_ekf_beats_raw_gps();
    test_ekf_urban_canyon();
    test_pipeline_with_ekf_localization();
    test_default_localization_matches_legacy_behavior();

    if (failures > 0) {
        std::cout << failures << " failure(s)\n";
        return 1;
    }
    std::cout << "all nav batch-2 tests passed\n";
    return 0;
}
