#include "agbot_nav/DynamicAgents.hpp"

#include "agbot_nav/NavTypes.hpp"

#include <cmath>

namespace agbot::nav {

namespace {

constexpr double kArrivalEps = 1e-9;

// Advance the waypoint cursor after arriving at path[next_waypoint]. Returns
// false when the agent is finished (Once behavior exhausted or degenerate
// path).
bool advance_waypoint(DynamicAgent& agent) {
    const std::size_t count = agent.path.size();
    if (count <= 1) {
        return false;
    }
    if (agent.direction >= 0) {
        if (agent.next_waypoint + 1 < count) {
            ++agent.next_waypoint;
            return true;
        }
        switch (agent.behavior) {
        case AgentPathBehavior::Loop:
            agent.next_waypoint = 0;
            return true;
        case AgentPathBehavior::PingPong:
            agent.direction = -1;
            agent.next_waypoint = count - 2;
            return true;
        case AgentPathBehavior::Once:
            return false;
        }
        return false;
    }
    if (agent.next_waypoint > 0) {
        --agent.next_waypoint;
        return true;
    }
    // Backward traversal only exists for PingPong; bounce forward again.
    agent.direction = 1;
    agent.next_waypoint = 1;
    return true;
}

} // namespace

const char* to_string(AgentKind kind) {
    switch (kind) {
    case AgentKind::Pedestrian:
        return "pedestrian";
    case AgentKind::Vehicle:
        return "vehicle";
    }
    return "pedestrian";
}

std::uint32_t agent_class_id(AgentKind kind) {
    return kind == AgentKind::Vehicle ? kClassVehicle : kClassPedestrian;
}

void step_agent(DynamicAgent& agent, double dt_s) {
    if (dt_s <= 0.0) {
        return;
    }
    if (agent.done || agent.path.empty() || agent.speed_mps <= 0.0) {
        agent.vx = 0.0;
        agent.vz = 0.0;
        return;
    }
    if (agent.next_waypoint >= agent.path.size()) {
        agent.next_waypoint = agent.path.size() - 1;
    }

    double remaining = agent.speed_mps * dt_s;
    // Bounded waypoint advances per step guard against zero-length segment
    // cycles; travel distance still dominates the loop in normal paths.
    std::size_t advances = 2 * agent.path.size() + 2;
    while (remaining > kArrivalEps && !agent.done && advances > 0) {
        const Vec3& target = agent.path[agent.next_waypoint];
        const double dx = target.x - agent.x;
        const double dz = target.z - agent.z;
        const double distance = std::sqrt(dx * dx + dz * dz);
        if (distance <= remaining) {
            agent.x = target.x;
            agent.z = target.z;
            remaining -= distance;
            if (!advance_waypoint(agent)) {
                agent.done = true;
            }
            --advances;
        } else {
            agent.x += dx / distance * remaining;
            agent.z += dz / distance * remaining;
            remaining = 0.0;
        }
    }

    // Velocity toward the current target (evidence for tests/telemetry and
    // ground truth for tracker validation).
    if (agent.done) {
        agent.vx = 0.0;
        agent.vz = 0.0;
        return;
    }
    const Vec3& target = agent.path[agent.next_waypoint];
    const double dx = target.x - agent.x;
    const double dz = target.z - agent.z;
    const double distance = std::sqrt(dx * dx + dz * dz);
    if (distance <= kArrivalEps) {
        agent.vx = 0.0;
        agent.vz = 0.0;
        return;
    }
    agent.vx = dx / distance * agent.speed_mps;
    agent.vz = dz / distance * agent.speed_mps;
}

void step_agents(std::vector<DynamicAgent>& agents, double dt_s) {
    for (DynamicAgent& agent : agents) {
        step_agent(agent, dt_s);
    }
}

} // namespace agbot::nav
