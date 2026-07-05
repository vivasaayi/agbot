#pragma once

#include "agbot_flight_sim/Vec3.hpp"

#include <cstdint>
#include <vector>

namespace agbot::nav {

// Sensor object-id namespace for dynamic agents: a depth-camera return from
// agent `id` carries object id kDynamicObjectIdBase + id, disjoint from the
// static scene-object ids (1 + object index) and ground (0).
inline constexpr std::uint32_t kDynamicObjectIdBase = 1u << 20;

enum class AgentKind {
    Pedestrian,
    Vehicle,
};

// How the agent traverses its waypoint path once the last waypoint is hit.
enum class AgentPathBehavior {
    Loop,     // continue from the first waypoint
    Once,     // stop at the last waypoint (velocity drops to zero)
    PingPong, // reverse direction and walk the path backwards
};

// One moving agent in the NavWorld: a vertical cylinder of radius_m/height_m
// following `path` at constant speed_mps. Waypoint progression state
// (next_waypoint, direction, done) lives in the struct so stepping is a pure
// deterministic function of the previous state and dt.
struct DynamicAgent {
    std::uint32_t id = 0;
    AgentKind kind = AgentKind::Pedestrian;
    double x = 0.0;  // world XZ position (meters)
    double z = 0.0;
    double vx = 0.0; // current velocity (m/s), maintained by step_agents
    double vz = 0.0;
    double radius_m = 0.35;
    double height_m = 1.7;
    double speed_mps = 1.2;
    std::vector<agbot::flight_sim::Vec3> path; // XZ waypoints (y ignored)
    AgentPathBehavior behavior = AgentPathBehavior::Loop;

    // Progression state.
    std::size_t next_waypoint = 0;
    int direction = 1; // +1 forward, -1 backward (PingPong)
    bool done = false;
};

[[nodiscard]] const char* to_string(AgentKind kind);

// Semantic class id (kClassPedestrian / kClassVehicle) for an agent kind.
[[nodiscard]] std::uint32_t agent_class_id(AgentKind kind);

// Advance one agent by dt_s: constant-speed waypoint following with exact
// arrival handling (the remaining travel budget carries across waypoints
// within the same step). Deterministic; no RNG.
void step_agent(DynamicAgent& agent, double dt_s);

// Advance every agent by dt_s.
void step_agents(std::vector<DynamicAgent>& agents, double dt_s);

} // namespace agbot::nav
