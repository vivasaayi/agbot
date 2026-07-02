#include "agbot_nav/DynamicAgents.hpp"
#include "agbot_nav/LocalPlanner.hpp"
#include "agbot_nav/MppiPlanner.hpp"
#include "agbot_nav/NavigationPipeline.hpp"
#include "agbot_nav/Sensing.hpp"
#include "agbot_nav/Tracking.hpp"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <iostream>
#include <limits>
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
using agbot::nav::AgentKind;
using agbot::nav::AgentPathBehavior;
using agbot::nav::Costmap;
using agbot::nav::Detection;
using agbot::nav::DynamicAgent;
using agbot::nav::NavWorld;
using agbot::nav::OccupancyGrid;
using agbot::nav::Path;
using agbot::nav::TrackedObject;
using agbot::nav::Vec3;
using agbot::vehicles::EntityState;

constexpr double kRobotRadiusM = 0.5;

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

// Same corridor as nav_tests.cpp: side walls at |z| in [4, 7], offset
// obstacle at x in [38, 42] blocking z in [-4, 1].
NavWorld make_corridor_world() {
    NavWorld world;
    world.ground_height_m = 0.0;
    world.scene.status = SceneSynthesisStatus::Ready;
    world.scene.scene_hash = "nav-dynamic-corridor";
    world.scene.objects.push_back(make_box("wall_north", -5.0, 85.0, 4.0, 7.0, 3.0));
    world.scene.objects.push_back(make_box("wall_south", -5.0, 85.0, -7.0, -4.0, 3.0));
    world.scene.objects.push_back(make_box("offset_obstacle", 38.0, 42.0, -4.0, 1.0, 3.0));
    world.scene.object_count = world.scene.objects.size();
    return world;
}

DynamicAgent make_pedestrian(
    std::uint32_t id,
    const std::vector<Vec3>& path,
    double speed_mps,
    AgentPathBehavior behavior) {
    DynamicAgent agent;
    agent.id = id;
    agent.kind = AgentKind::Pedestrian;
    agent.x = path.front().x;
    agent.z = path.front().z;
    agent.radius_m = 0.35;
    agent.height_m = 1.7;
    agent.speed_mps = speed_mps;
    agent.path = path;
    agent.behavior = behavior;
    return agent;
}

double distance_to_polyline(const std::vector<Vec3>& points, double x, double z) {
    if (points.empty()) {
        return 0.0;
    }
    double best = std::numeric_limits<double>::infinity();
    if (points.size() == 1) {
        return std::hypot(points.front().x - x, points.front().z - z);
    }
    for (std::size_t i = 1; i < points.size(); ++i) {
        const Vec3& a = points[i - 1];
        const Vec3& b = points[i];
        const double abx = b.x - a.x;
        const double abz = b.z - a.z;
        const double denom = abx * abx + abz * abz;
        double t = 0.0;
        if (denom > 1e-12) {
            t = ((x - a.x) * abx + (z - a.z) * abz) / denom;
            t = std::clamp(t, 0.0, 1.0);
        }
        best = std::min(best, std::hypot(x - (a.x + abx * t), z - (a.z + abz * t)));
    }
    return best;
}

// ---------------------------------------------------------------------------
// Dynamic agent unit tests
// ---------------------------------------------------------------------------

void test_agent_waypoint_following() {
    // Constant speed along a straight segment: exact position after dt.
    DynamicAgent walker =
        make_pedestrian(1, {{0.0, 0.0, 0.0}, {10.0, 0.0, 0.0}}, 2.0, AgentPathBehavior::Once);
    walker.next_waypoint = 1; // already at path[0]
    agbot::nav::step_agent(walker, 0.5);
    expect(std::abs(walker.x - 1.0) < 1e-12 && std::abs(walker.z) < 1e-12,
           "agent advances exactly speed*dt along the segment");
    expect(std::abs(walker.vx - 2.0) < 1e-12 && std::abs(walker.vz) < 1e-12,
           "agent velocity points along the segment at the configured speed");

    // Once: stops at the last waypoint with zero velocity.
    agbot::nav::step_agent(walker, 10.0);
    expect(walker.done && walker.x == 10.0 && walker.vx == 0.0 && walker.vz == 0.0,
           "once behavior stops at the final waypoint with zero velocity");

    // PingPong: 2 m forward then bounce back (waypoint arrival mid-step).
    DynamicAgent bouncer =
        make_pedestrian(2, {{0.0, 0.0, 0.0}, {2.0, 0.0, 0.0}}, 1.0, AgentPathBehavior::PingPong);
    bouncer.next_waypoint = 1;
    for (int i = 0; i < 30; ++i) { // 3.0 s in 0.1 s steps
        agbot::nav::step_agent(bouncer, 0.1);
    }
    expect(std::abs(bouncer.x - 1.0) < 1e-9 && bouncer.vx < 0.0,
           "ping_pong reverses at the end and walks back");

    // Loop: closes back onto the first waypoint.
    DynamicAgent looper = make_pedestrian(
        3, {{0.0, 0.0, 0.0}, {4.0, 0.0, 0.0}, {4.0, 0.0, 4.0}}, 1.0, AgentPathBehavior::Loop);
    looper.next_waypoint = 1;
    for (int i = 0; i < 100; ++i) { // 10 s: 4 + 4 forward, then toward (0,0)
        agbot::nav::step_agent(looper, 0.1);
    }
    const double loop_leg = std::sqrt(32.0); // diagonal back to the start
    const double expected_progress = 2.0 / loop_leg;
    expect(std::abs(looper.x - (4.0 - 4.0 * expected_progress)) < 1e-9
               && std::abs(looper.z - (4.0 - 4.0 * expected_progress)) < 1e-9,
           "loop behavior continues toward the first waypoint");

    // Determinism: two identical agents stepped identically stay bit-equal.
    DynamicAgent a = make_pedestrian(
        4, {{0.0, 0.0, 1.0}, {5.0, 0.0, -2.0}, {1.0, 0.0, 3.0}}, 1.3, AgentPathBehavior::PingPong);
    DynamicAgent b = a;
    bool identical = true;
    for (int i = 0; i < 500; ++i) {
        agbot::nav::step_agent(a, 0.02);
        agbot::nav::step_agent(b, 0.02);
        identical = identical && a.x == b.x && a.z == b.z && a.vx == b.vx && a.vz == b.vz
            && a.next_waypoint == b.next_waypoint && a.direction == b.direction;
    }
    expect(identical, "agent stepping is bit-deterministic");
}

void test_sensor_sees_agent_cylinder() {
    NavWorld world;
    world.ground_height_m = 0.0;
    world.scene.status = SceneSynthesisStatus::Ready;
    world.agents.push_back(make_pedestrian(
        7, {{10.0, 0.0, 0.0}, {10.0, 0.0, 1.0}}, 0.0, AgentPathBehavior::Once));

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
    expect(frame.status == "ok", "sensor produces a frame with agents present");

    // Center ray is horizontal at y=1: cylinder front face at x = 10 - 0.35.
    const std::size_t center =
        static_cast<std::size_t>(4) * static_cast<std::size_t>(frame.width) + 4;
    expect(std::abs(frame.depth_m[center] - 9.65) < 1e-9,
           "center ray range hits the cylinder front surface exactly");
    // Columns 4 +/- 1 point 4.6 deg off-axis, outside the 2.0 deg half-angle
    // the 0.35 m cylinder subtends at 10 m: they must miss it.
    const std::size_t side = center + 1;
    expect(frame.depth_m[side] <= 0.0 || frame.depth_m[side] > 15.0,
           "adjacent column misses the narrow cylinder");

    std::size_t pedestrian_points = 0;
    bool ids_correct = true;
    for (std::size_t i = 0; i < frame.cloud.size(); ++i) {
        if (frame.cloud.classes[i] == agbot::nav::kClassPedestrian) {
            ++pedestrian_points;
            ids_correct = ids_correct
                && frame.cloud.object_ids[i] == agbot::nav::kDynamicObjectIdBase + 7;
        } else {
            ids_correct = ids_correct
                && frame.cloud.object_ids[i] < agbot::nav::kDynamicObjectIdBase;
        }
    }
    expect(pedestrian_points >= 2, "several pixels return the pedestrian class");
    expect(ids_correct, "agent returns carry the dynamic object id namespace");

    // Vehicle kind maps to the vehicle class.
    world.agents[0].kind = AgentKind::Vehicle;
    agbot::nav::DepthCameraSensor sensor2(params);
    const agbot::nav::SensorFrame vframe = sensor2.sense(world, state, 0.0);
    bool saw_vehicle = false;
    for (const std::uint32_t cls : vframe.cloud.classes) {
        saw_vehicle = saw_vehicle || cls == agbot::nav::kClassVehicle;
    }
    expect(saw_vehicle, "vehicle agents return the vehicle class");
}

void test_static_world_regression() {
    // A world without agents and a world whose only agent sits far beyond
    // sensor range must produce bit-identical frames (the agent loop must
    // not disturb geometry or the deterministic noise stream).
    const NavWorld bare = make_corridor_world();
    NavWorld with_far_agent = make_corridor_world();
    with_far_agent.agents.push_back(make_pedestrian(
        1, {{300.0, 0.0, 0.0}, {300.0, 0.0, 5.0}}, 1.0, AgentPathBehavior::Loop));

    agbot::config::ParamTable params;
    params["width"] = 32;
    params["height"] = 24;
    params["max_range_m"] = 30.0;
    params["range_noise_a"] = 0.002;
    params["dropout_pct"] = 2.0;
    params["seed"] = 7;
    agbot::nav::DepthCameraSensor sensor_a(params);
    agbot::nav::DepthCameraSensor sensor_b(params);
    EntityState state;
    bool identical = true;
    for (int i = 0; i < 5; ++i) {
        const agbot::nav::SensorFrame fa = sensor_a.sense(bare, state, 0.1 * i);
        const agbot::nav::SensorFrame fb = sensor_b.sense(with_far_agent, state, 0.1 * i);
        identical = identical && fa.depth_m == fb.depth_m && fa.cloud.classes == fb.cloud.classes;
    }
    expect(identical, "empty/out-of-range agents leave sensor frames bit-identical");
}

// ---------------------------------------------------------------------------
// Tracker unit tests
// ---------------------------------------------------------------------------

agbot::config::ParamTable tracker_params() {
    agbot::config::ParamTable params;
    params["gate_m"] = 1.0;
    params["min_hits"] = 2;
    params["max_missed"] = 3;
    params["q_pos"] = 0.3;
    params["q_vel"] = 2.0;
    params["r_pos"] = 0.1;
    return params;
}

Detection detection_at(double x, double z, std::uint32_t class_id = agbot::nav::kClassPedestrian) {
    Detection detection;
    detection.position = {x, 0.0, z};
    detection.radius_m = 0.35;
    detection.class_id = class_id;
    detection.point_count = 5;
    return detection;
}

void test_tracker_crossing_agents() {
    // Agent A walks +x along z = 0; agent B walks -x along z = 0.5. They
    // cross near x = 5 without the tracker swapping ids.
    const double dt = 0.1;
    const double va = 1.5;
    const double vb = -1.5;

    agbot::nav::GreedyNnTracker tracker(tracker_params());
    std::vector<TrackedObject> tracks;
    std::uint32_t id_a = 0;
    std::uint32_t id_b = 0;
    bool velocity_ok_at_15 = false;
    bool premature_track = false;
    for (int frame = 0; frame < 70; ++frame) {
        const double t = frame * dt;
        std::vector<Detection> detections;
        detections.push_back(detection_at(0.0 + va * t, 0.0));
        detections.push_back(detection_at(10.0 + vb * t, 0.5));
        tracks = tracker.update(detections, t);
        if (frame == 0 && !tracks.empty()) {
            premature_track = true; // min_hits = 2: nothing confirmed yet
        }
        if (frame == 2) {
            // Both tracks confirmed; remember which id is which by position.
            for (const TrackedObject& track : tracks) {
                if (track.position.x < 5.0) {
                    id_a = track.id;
                } else {
                    id_b = track.id;
                }
            }
        }
        if (frame == 15) {
            for (const TrackedObject& track : tracks) {
                const double v_ref = track.id == id_a ? va : vb;
                velocity_ok_at_15 = std::abs(track.velocity.x - v_ref) <= 0.15 * std::abs(v_ref)
                    && std::abs(track.velocity.z) <= 0.15 * std::abs(v_ref);
                if (!velocity_ok_at_15) {
                    break;
                }
                velocity_ok_at_15 = true;
            }
        }
    }
    expect(!premature_track, "tracks are not reported before min_hits");
    expect(tracks.size() == 2 && id_a != 0 && id_b != 0, "two confirmed tracks exist");
    expect(velocity_ok_at_15, "velocity estimates converge within 15% after 15 frames");

    // After crossing, the id that started near x=0 must be the one on the
    // right and still moving +x (no id swap).
    bool no_swap = true;
    for (const TrackedObject& track : tracks) {
        if (track.id == id_a) {
            no_swap = no_swap && track.position.x > 7.0 && track.velocity.x > 0.0
                && std::abs(track.position.z) < 0.3;
        } else if (track.id == id_b) {
            no_swap = no_swap && track.position.x < 3.0 && track.velocity.x < 0.0
                && std::abs(track.position.z - 0.5) < 0.3;
        }
    }
    expect(no_swap, "crossing agents keep their track ids (no swap)");

    // Death: max_missed empty frames coast the tracks, one more kills them.
    std::vector<TrackedObject> coasting;
    for (int miss = 0; miss < 3; ++miss) {
        coasting = tracker.update({}, 7.0 + 0.1 * miss);
    }
    expect(coasting.size() == 2 && coasting.front().missed == 3,
           "tracks coast through max_missed empty frames");
    coasting = tracker.update({}, 7.4);
    expect(coasting.empty(), "tracks die after max_missed consecutive misses");

    // Determinism: an identical rerun produces bit-identical track states.
    agbot::nav::GreedyNnTracker first(tracker_params());
    agbot::nav::GreedyNnTracker second(tracker_params());
    bool identical = true;
    for (int frame = 0; frame < 40; ++frame) {
        const double t = frame * 0.1;
        std::vector<Detection> detections;
        detections.push_back(detection_at(1.5 * t, 0.0));
        detections.push_back(detection_at(10.0 - 1.5 * t, 0.5));
        const std::vector<TrackedObject> ta = first.update(detections, t);
        const std::vector<TrackedObject> tb = second.update(detections, t);
        identical = identical && ta.size() == tb.size();
        for (std::size_t i = 0; identical && i < ta.size(); ++i) {
            identical = ta[i].id == tb[i].id && ta[i].position.x == tb[i].position.x
                && ta[i].position.z == tb[i].position.z
                && ta[i].velocity.x == tb[i].velocity.x
                && ta[i].velocity.z == tb[i].velocity.z;
        }
    }
    expect(identical, "tracker updates are bit-deterministic");
}

void test_detection_clustering() {
    agbot::nav::PointCloud cloud;
    // Static obstacle point, ground point, and two dynamic blobs.
    cloud.points = {
        {1.0, 1.0, 0.0},  // static obstacle
        {2.0, 0.0, 0.0},  // ground
        {5.0, 1.0, 0.0},  {5.2, 1.2, 0.1},  {5.1, 0.6, -0.1}, // pedestrian blob
        {9.0, 1.0, 4.0},  {9.1, 0.9, 4.2},                    // vehicle blob
    };
    cloud.classes = {
        agbot::nav::kClassObstacle,
        agbot::nav::kClassGround,
        agbot::nav::kClassPedestrian, agbot::nav::kClassPedestrian,
        agbot::nav::kClassPedestrian,
        agbot::nav::kClassVehicle, agbot::nav::kClassVehicle,
    };
    cloud.object_ids = {1, 0,
                        agbot::nav::kDynamicObjectIdBase + 3,
                        agbot::nav::kDynamicObjectIdBase + 3,
                        agbot::nav::kDynamicObjectIdBase + 3,
                        agbot::nav::kDynamicObjectIdBase + 4,
                        agbot::nav::kDynamicObjectIdBase + 4};

    const std::vector<Detection> by_id =
        agbot::nav::cluster_dynamic_detections(cloud, true, 1.0);
    expect(by_id.size() == 2, "object-id clustering yields one detection per agent");
    expect(by_id[0].class_id == agbot::nav::kClassPedestrian && by_id[0].point_count == 3
               && std::abs(by_id[0].position.x - 5.1) < 1e-9,
           "pedestrian detection centroid and class are correct");

    const std::vector<Detection> by_distance =
        agbot::nav::cluster_dynamic_detections(cloud, false, 1.0);
    expect(by_distance.size() == 2,
           "distance clustering separates the two blobs without object ids");
    expect(by_distance[1].class_id == agbot::nav::kClassVehicle
               && by_distance[1].point_count == 2,
           "distance clustering keeps the vehicle blob intact");
}

// ---------------------------------------------------------------------------
// Predictive costing unit tests
// ---------------------------------------------------------------------------

Costmap free_costmap() {
    Costmap costmap;
    costmap.origin_x = -5.0;
    costmap.origin_z = -5.0;
    costmap.resolution_m = 0.25;
    costmap.width = 100;
    costmap.height = 80;
    costmap.reset(0);
    return costmap;
}

TrackedObject tracked_pedestrian(double x, double z, double vx, double vz) {
    TrackedObject object;
    object.id = 1;
    object.position = {x, 0.0, z};
    object.velocity = {vx, 0.0, vz};
    object.radius_m = 0.35;
    object.class_name = "pedestrian";
    object.age = 10;
    return object;
}

double min_predicted_separation(
    const agbot::nav::Trajectory& trajectory, const TrackedObject& object) {
    double best = std::numeric_limits<double>::infinity();
    for (const agbot::nav::TrajectoryPoint& point : trajectory.points) {
        const double px = object.position.x + object.velocity.x * point.t;
        const double pz = object.position.z + object.velocity.z * point.t;
        best = std::min(
            best, std::hypot(point.position.x - px, point.position.z - pz));
    }
    return best;
}

void test_mppi_predictive_avoidance() {
    const Costmap costmap = free_costmap();
    Path straight;
    for (int i = 0; i <= 24; ++i) {
        straight.points.push_back({0.5 * i, 0.0, 0.0});
    }
    agbot::vehicles::VehicleLimits limits;
    limits.wheelbase_m = 0.8;
    limits.max_speed_mps = 3.0;
    EntityState state;
    state.velocity = {2.0, 0.0, 0.0};
    const Vec3 goal{12.0, 0.0, 0.0};

    agbot::config::ParamTable params;
    params["time_steps"] = 25;
    params["dt"] = 0.08;
    params["num_samples"] = 512;
    params["cruise_speed_mps"] = 2.0;
    params["seed"] = 3;
    params["w_dynamic"] = 4.0;

    // Pedestrian crossing the path at x = 3 exactly when the robot arrives.
    const TrackedObject crossing = tracked_pedestrian(3.0, -2.5, 0.0, 1.7);

    agbot::nav::MppiPlanner baseline(params);
    const agbot::nav::LocalPlan plan_without =
        baseline.compute(costmap, straight, state, limits, goal);
    agbot::nav::MppiPlanner reacting(params);
    const agbot::nav::LocalPlan plan_with =
        reacting.compute(costmap, straight, state, limits, goal, {crossing});

    expect(plan_without.ok && plan_with.ok, "MPPI plans succeed with and without tracks");
    const double sep_without = min_predicted_separation(plan_without.trajectory, crossing);
    const double sep_with = min_predicted_separation(plan_with.trajectory, crossing);
    std::cout << "  [mppi_unit] predicted separation without=" << sep_without
              << " with=" << sep_with << " (hard margin "
              << (0.5 + crossing.radius_m + 0.3) << ")\n";
    expect(sep_with > sep_without,
           "predictive critic pushes the rollout away from the crossing");
    expect(sep_with > 0.5 + crossing.radius_m + 0.3,
           "chosen trajectory clears the hard dynamic margin");

    // Hard penalty: a pedestrian parked directly on the robot within the
    // margin makes every rollout lethal (reason-coded) or forces a plan that
    // still clears the margin.
    const TrackedObject blocking = tracked_pedestrian(1.2, 0.0, 0.0, 0.0);
    agbot::nav::MppiPlanner blocked(params);
    const agbot::nav::LocalPlan plan_blocked =
        blocked.compute(costmap, straight, state, limits, goal, {blocking});
    expect(!plan_blocked.ok ? plan_blocked.reason == "all_rollouts_lethal"
                            : min_predicted_separation(plan_blocked.trajectory, blocking)
                                > 0.5 + blocking.radius_m + 0.3 - 1e-9,
           "inside-margin proximity triggers the hard penalty");
}

void test_dwa_and_pure_pursuit_dynamic_response() {
    const Costmap costmap = free_costmap();
    Path straight;
    for (int i = 0; i <= 24; ++i) {
        straight.points.push_back({0.5 * i, 0.0, 0.0});
    }
    agbot::vehicles::VehicleLimits limits;
    limits.wheelbase_m = 0.8;
    EntityState state;
    state.velocity = {1.5, 0.0, 0.0};
    const Vec3 goal{12.0, 0.0, 0.0};

    // DWA: stationary pedestrian dead ahead. Either every rollout violates
    // the hard margin (reason-coded) or the chosen rollout clears it.
    const TrackedObject blocking = tracked_pedestrian(2.0, 0.0, 0.0, 0.0);
    agbot::nav::DwaPlanner dwa;
    const agbot::nav::LocalPlan dwa_plan =
        dwa.compute(costmap, straight, state, limits, goal, {blocking});
    if (dwa_plan.ok) {
        expect(min_predicted_separation(dwa_plan.trajectory, blocking)
                   > 0.5 + blocking.radius_m + 0.3 - 1e-9,
               "DWA rollout clears the dynamic hard margin");
    } else {
        expect(dwa_plan.reason == "all_rollouts_lethal",
               "DWA reports the reason-coded dynamic rejection");
    }
    const agbot::nav::LocalPlan dwa_free = dwa.compute(costmap, straight, state, limits, goal);
    expect(dwa_free.ok, "DWA without tracked objects is unaffected");

    // Pure pursuit: the yield guard scales speed down for a predicted
    // crossing and leaves the empty-track case untouched.
    agbot::nav::PurePursuitPlanner pursuit;
    const agbot::nav::LocalPlan pp_free =
        pursuit.compute(costmap, straight, state, limits, goal);
    const TrackedObject crossing = tracked_pedestrian(3.0, -1.5, 0.0, 1.0);
    const agbot::nav::LocalPlan pp_yield =
        pursuit.compute(costmap, straight, state, limits, goal, {crossing});
    std::cout << "  [pure_pursuit_unit] v_free=" << pp_free.v_cmd
              << " v_yield=" << pp_yield.v_cmd << "\n";
    expect(pp_free.ok && pp_yield.ok, "pure pursuit plans succeed");
    expect(pp_yield.v_cmd < 0.5 * pp_free.v_cmd,
           "yield guard cuts speed for a predicted crossing");
    const TrackedObject imminent = tracked_pedestrian(1.0, 0.0, 0.0, 0.0);
    const agbot::nav::LocalPlan pp_stop =
        pursuit.compute(costmap, straight, state, limits, goal, {imminent});
    expect(pp_stop.v_cmd == 0.0, "yield guard commands a full stop at predicted contact");
}

// ---------------------------------------------------------------------------
// Integration A: corridor + crossing pedestrian
// ---------------------------------------------------------------------------

std::string corridor_config_text(bool tracking_enabled) {
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
decay = 48
inflation_radius_m = 1.0
cost_scaling = 3.0
lethal_threshold = 200

[global]
algorithm = "astar"
lethal_threshold = 200
heuristic_weight = 1.0
cost_weight = 4.0
unknown_cost = 0
smooth = true

[local]
algorithm = "mppi"
time_steps = 25
dt = 0.08
num_samples = 512
lambda = 0.4
sigma_accel = 1.0
sigma_steer_rate = 1.2
cruise_speed_mps = 2.5
lethal_threshold = 130
w_obstacle = 4.0
w_path = 1.2
w_goal = 0.6
w_speed = 0.4
w_smooth = 0.05
min_speed_mps = 0.0
goal_slow_gain = 0.8
seed = 5
w_dynamic = 4.0
dynamic_margin_m = 0.3
dynamic_sigma_m = 1.2
max_prediction_s = 2.5
robot_radius_m = 0.5

[control]
algorithm = "pid_stanley"
kp = 1.5
ki = 0.3
integral_limit = 2.0
k_e = 1.2
k_soft = 1.0
)toml";
    if (tracking_enabled) {
        toml << R"toml(
[tracking]
algorithm = "greedy_nn"
use_object_ids = true
gate_m = 2.5
min_hits = 2
max_missed = 8
q_pos = 0.3
q_vel = 2.0
r_pos = 0.15
)toml";
    }
    return toml.str();
}

struct DynamicRun {
    bool built = false;
    bool reached = false;
    double time_to_goal_s = 0.0;
    double min_agent_separation_m = 1e9; // surface-to-surface, robot vs agents
    std::size_t collision_ticks = 0;
    std::size_t max_tracked = 0;
    double min_speed_in_window_mps = 1e9;
    double max_cross_track_unobstructed_m = 0.0;
    double max_lateral_dev_in_window_m = 0.0;
    EntityState final_state;
    std::size_t tick_count = 0;
};

DynamicRun run_corridor_crossing(const std::string& config_text, double max_time_s = 90.0) {
    DynamicRun run;
    const agbot::nav::NavigationPipelineConfig config =
        agbot::nav::parse_pipeline_config(config_text);
    agbot::nav::NavigationPipeline pipeline(config);
    if (!pipeline.ok()) {
        std::cout << "  pipeline error: " << pipeline.error() << "\n";
        return run;
    }
    run.built = true;

    NavWorld world = make_corridor_world();
    // Pedestrian crossing the corridor at x = 20, timed to be in the robot
    // lane around the robot's unimpeded arrival (~8 s at 2.5 m/s cruise).
    world.agents.push_back(make_pedestrian(
        1, {{20.0, 0.0, 3.8}, {20.0, 0.0, -3.8}}, 0.45, AgentPathBehavior::Once));

    const Vec3 goal{80.0, 0.0, 0.0};
    EntityState state;
    const double dt = 0.02;
    double elapsed = 0.0;
    while (elapsed < max_time_s) {
        agbot::nav::step_agents(world.agents, dt);
        pipeline.tick(world, state, goal, dt);
        elapsed += dt;
        ++run.tick_count;

        for (const DynamicAgent& agent : world.agents) {
            const double separation =
                std::hypot(state.position.x - agent.x, state.position.z - agent.z)
                - (kRobotRadiusM + agent.radius_m);
            run.min_agent_separation_m = std::min(run.min_agent_separation_m, separation);
            if (separation <= 0.0) {
                ++run.collision_ticks;
            }
        }
        run.max_tracked = std::max(run.max_tracked, pipeline.tracked_objects().size());
        if (std::abs(state.position.x - 20.0) < 8.0) {
            run.min_speed_in_window_mps = std::min(
                run.min_speed_in_window_mps, state.velocity.horizontal_length());
        }
        if (pipeline.goal_reached()) {
            run.reached = true;
            run.time_to_goal_s = elapsed;
            break;
        }
    }
    run.final_state = state;
    return run;
}

void test_integration_corridor_crossing() {
    const DynamicRun tracked = run_corridor_crossing(corridor_config_text(true));
    const DynamicRun blind = run_corridor_crossing(corridor_config_text(false));
    std::cout << "  [corridor tracked] reached=" << (tracked.reached ? "yes" : "no")
              << " time_s=" << tracked.time_to_goal_s
              << " min_sep_m=" << tracked.min_agent_separation_m
              << " collisions=" << tracked.collision_ticks
              << " max_tracked=" << tracked.max_tracked
              << " min_speed_window=" << tracked.min_speed_in_window_mps << "\n";
    std::cout << "  [corridor blind]   reached=" << (blind.reached ? "yes" : "no")
              << " time_s=" << blind.time_to_goal_s
              << " min_sep_m=" << blind.min_agent_separation_m
              << " collisions=" << blind.collision_ticks << "\n";

    expect(tracked.built && blind.built, "both corridor pipelines build");
    expect(tracked.reached, "tracked run reaches the goal despite the crossing");
    expect(tracked.collision_ticks == 0, "tracked run is collision-free");
    expect(tracked.min_agent_separation_m > 0.0,
           "tracked run keeps positive surface separation to the pedestrian");
    expect(tracked.max_tracked >= 1, "tracker confirmed the crossing pedestrian");
    expect(tracked.min_agent_separation_m > blind.min_agent_separation_m,
           "prediction measurably increases the closest-approach margin");
}

void test_integration_corridor_determinism() {
    const std::string config = corridor_config_text(true);
    const DynamicRun first = run_corridor_crossing(config);
    const DynamicRun second = run_corridor_crossing(config);
    expect(first.reached && second.reached, "both corridor reruns reach the goal");
    expect(first.final_state.position.x == second.final_state.position.x
               && first.final_state.position.z == second.final_state.position.z
               && first.final_state.yaw_rad == second.final_state.yaw_rad
               && first.tick_count == second.tick_count
               && first.min_agent_separation_m == second.min_agent_separation_m,
           "corridor crossing runs are bit-identical");
}

// ---------------------------------------------------------------------------
// Integration B: delivery run on the worldgen street fixture
// ---------------------------------------------------------------------------

std::string delivery_config_text() {
    std::ostringstream toml;
    toml << R"toml(
[pipeline]
sensor_period_s = 0.1
map_period_s = 0.1
plan_period_s = 3.0
local_period_s = 0.1
goal_tolerance_m = 1.5

[vehicle]
algorithm = "kinematic_bicycle"
max_speed_mps = 4.0
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
width = 490
height = 180
resolution_m = 0.5
hit_increment = 128
decay = 48
inflation_radius_m = 1.0
cost_scaling = 3.0
lethal_threshold = 200

[global]
algorithm = "road_graph"
cost_mode = "time"
max_snap_m = 50.0
)toml";
    toml << "roads_path = \"" << FLIGHT_SIM_ROOT_DIR
         << "/worldgen/tests/fixtures/roads_fixture.json\"\n";
    toml << R"toml(
aoi_min_lat = -0.01
aoi_min_lon = -0.01
aoi_max_lat = 0.01
aoi_max_lon = 0.01

[local]
algorithm = "mppi"
time_steps = 25
dt = 0.08
num_samples = 256
lambda = 0.4
sigma_accel = 1.0
sigma_steer_rate = 1.2
cruise_speed_mps = 3.0
lethal_threshold = 130
w_obstacle = 4.0
w_path = 1.6
w_goal = 0.6
w_speed = 0.4
w_smooth = 0.05
min_speed_mps = 0.0
goal_slow_gain = 0.8
seed = 5
w_dynamic = 4.0
dynamic_margin_m = 0.3
dynamic_sigma_m = 1.2
max_prediction_s = 2.5
robot_radius_m = 0.5

[tracking]
algorithm = "greedy_nn"
use_object_ids = true
gate_m = 2.5
min_hits = 2
max_missed = 8
q_pos = 0.3
q_vel = 2.0
r_pos = 0.15

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

DynamicRun run_delivery(double max_time_s = 180.0) {
    DynamicRun run;
    const agbot::nav::NavigationPipelineConfig config =
        agbot::nav::parse_pipeline_config(delivery_config_text());
    agbot::nav::NavigationPipeline pipeline(config);
    if (!pipeline.ok()) {
        std::cout << "  pipeline error: " << pipeline.error() << "\n";
        return run;
    }
    run.built = true;

    // Street fixture: A Street runs along z = 0 (x in [0, 445]); Second
    // Avenue (oneway northbound) runs along x = 222.64 (z in [0, 222.64]).
    NavWorld world;
    world.ground_height_m = 0.0;
    world.scene.status = SceneSynthesisStatus::Ready;
    world.scene.scene_hash = "delivery-run";
    // Flanking buildings that never intrude on the streets.
    world.scene.objects.push_back(make_box("shop_north", 60.0, 90.0, 6.0, 16.0, 6.0));
    world.scene.objects.push_back(make_box("shop_south", 130.0, 160.0, -16.0, -6.0, 6.0));
    world.scene.object_count = world.scene.objects.size();
    // Pedestrian crossing A Street at x = 100, timed against the robot's
    // unimpeded arrival there (~30 s): the pedestrian reaches the lane
    // center z = 0 at t = 36 / 1.2 = 30 s and forces a yield.
    world.agents.push_back(make_pedestrian(
        1, {{100.0, 0.0, 36.0}, {100.0, 0.0, -8.0}}, 1.2, AgentPathBehavior::Once));

    const Vec3 goal{222.64, 0.0, 60.0};
    EntityState state;
    state.position = {10.0, 0.0, 0.0};
    const double dt = 0.02;
    double elapsed = 0.0;
    while (elapsed < max_time_s) {
        agbot::nav::step_agents(world.agents, dt);
        pipeline.tick(world, state, goal, dt);
        elapsed += dt;
        ++run.tick_count;

        for (const DynamicAgent& agent : world.agents) {
            const double separation =
                std::hypot(state.position.x - agent.x, state.position.z - agent.z)
                - (kRobotRadiusM + agent.radius_m);
            run.min_agent_separation_m = std::min(run.min_agent_separation_m, separation);
            if (separation <= 0.0) {
                ++run.collision_ticks;
            }
        }
        run.max_tracked = std::max(run.max_tracked, pipeline.tracked_objects().size());

        const double cross_track = distance_to_polyline(
            pipeline.global_path().points, state.position.x, state.position.z);
        const bool in_encounter_window = std::abs(state.position.x - 100.0) < 12.0
            && std::abs(state.position.z) < 6.0;
        if (in_encounter_window) {
            run.min_speed_in_window_mps = std::min(
                run.min_speed_in_window_mps, state.velocity.horizontal_length());
            run.max_lateral_dev_in_window_m =
                std::max(run.max_lateral_dev_in_window_m, cross_track);
        } else if (!pipeline.global_path().points.empty()) {
            run.max_cross_track_unobstructed_m =
                std::max(run.max_cross_track_unobstructed_m, cross_track);
        }
        if (pipeline.goal_reached()) {
            run.reached = true;
            run.time_to_goal_s = elapsed;
            break;
        }
    }
    run.final_state = state;
    return run;
}

void test_integration_delivery_run() {
    const DynamicRun run = run_delivery();
    std::cout << "  [delivery] reached=" << (run.reached ? "yes" : "no")
              << " time_s=" << run.time_to_goal_s
              << " min_sep_m=" << run.min_agent_separation_m
              << " collisions=" << run.collision_ticks
              << " max_tracked=" << run.max_tracked
              << " min_speed_window=" << run.min_speed_in_window_mps
              << " max_lat_dev_window=" << run.max_lateral_dev_in_window_m
              << " max_cross_track=" << run.max_cross_track_unobstructed_m << "\n";
    expect(run.built, "delivery pipeline builds with road_graph + tracking");
    expect(run.reached, "delivery run completes the street route");
    expect(run.collision_ticks == 0, "delivery run is collision-free");
    expect(run.min_agent_separation_m > 0.0,
           "delivery run keeps positive separation to the pedestrian");
    expect(run.max_tracked >= 1, "delivery run tracked the crossing pedestrian");
    expect(run.max_cross_track_unobstructed_m < 8.0,
           "cross-track error stays under 8 m while unobstructed");
    const bool yielded = run.min_speed_in_window_mps < 0.3 * 3.0
        || run.max_lateral_dev_in_window_m > 1.0;
    expect(yielded, "telemetry shows a yield (speed dip below 30% cruise or lateral event)");
}

void test_integration_delivery_determinism() {
    const DynamicRun first = run_delivery();
    const DynamicRun second = run_delivery();
    expect(first.reached && second.reached, "both delivery reruns reach the goal");
    expect(first.final_state.position.x == second.final_state.position.x
               && first.final_state.position.z == second.final_state.position.z
               && first.final_state.yaw_rad == second.final_state.yaw_rad
               && first.tick_count == second.tick_count
               && first.min_agent_separation_m == second.min_agent_separation_m,
           "delivery runs are bit-identical");
}

void test_tracker_registry() {
    expect(agbot::nav::default_tracker_registry().contains("greedy_nn"),
           "tracker registry exposes greedy_nn");
    const auto tracker = agbot::nav::default_tracker_registry().create("greedy_nn", {});
    expect(tracker != nullptr && tracker->name() == "greedy_nn",
           "tracker registry constructs greedy_nn");
}

} // namespace

int main() {
    test_agent_waypoint_following();
    test_sensor_sees_agent_cylinder();
    test_static_world_regression();
    test_tracker_registry();
    test_detection_clustering();
    test_tracker_crossing_agents();
    test_mppi_predictive_avoidance();
    test_dwa_and_pure_pursuit_dynamic_response();
    test_integration_corridor_crossing();
    test_integration_corridor_determinism();
    test_integration_delivery_run();
    test_integration_delivery_determinism();

    if (failures > 0) {
        std::cout << failures << " failure(s)\n";
        return 1;
    }
    std::cout << "all dynamic obstacle tests passed\n";
    return 0;
}
