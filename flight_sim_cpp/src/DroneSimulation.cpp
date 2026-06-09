#include "agbot_flight_sim/DroneSimulation.hpp"

#include <algorithm>
#include <cmath>
#include <stdexcept>

namespace agbot::flight_sim {
namespace {

constexpr double kPi = 3.14159265358979323846;

DroneMode mode_for_waypoint(const Waypoint& waypoint) {
    switch (waypoint.action) {
        case WaypointAction::Takeoff:
            return DroneMode::Takeoff;
        case WaypointAction::Loiter:
            return DroneMode::Loiter;
        case WaypointAction::Land:
            return DroneMode::Landing;
        case WaypointAction::ReturnHome:
        case WaypointAction::FlyThrough:
            return DroneMode::Flying;
    }
    return DroneMode::Flying;
}

double clamp_step(double dt_s, double max_step_s) {
    if (dt_s <= 0.0) {
        return 0.0;
    }
    return std::min(dt_s, max_step_s);
}

} // namespace

DroneSimulation::DroneSimulation(Mission mission, SimulationConfig config)
    : mission_(std::move(mission)), config_(config) {
    if (mission_.waypoints.empty()) {
        throw std::runtime_error("DroneSimulation requires a mission with waypoints");
    }
    reset();
}

void DroneSimulation::reset() {
    state_ = {};
    state_.position = mission_.home;
    state_.mode = DroneMode::Idle;
}

void DroneSimulation::step(double dt_s) {
    while (dt_s > 0.0) {
        const double step_s = clamp_step(dt_s, config_.max_step_s);
        if (step_s <= 0.0) {
            break;
        }
        step_fixed(step_s);
        dt_s -= step_s;
    }
}

const Mission& DroneSimulation::mission() const {
    return mission_;
}

const DroneState& DroneSimulation::state() const {
    return state_;
}

bool DroneSimulation::is_complete() const {
    return state_.mode == DroneMode::Completed || state_.mode == DroneMode::Failsafe;
}

double DroneSimulation::progress() const {
    if (mission_.waypoints.empty()) {
        return 1.0;
    }
    return std::clamp(
        static_cast<double>(state_.target_waypoint_index) / static_cast<double>(mission_.waypoints.size()),
        0.0,
        1.0
    );
}

void DroneSimulation::step_fixed(double dt_s) {
    if (is_complete()) {
        return;
    }

    state_.mission_time_s += dt_s;

    if (state_.battery_percent <= config_.min_battery_percent) {
        state_.mode = DroneMode::Failsafe;
        state_.velocity = {};
        return;
    }

    const Waypoint* waypoint = target_waypoint();
    if (waypoint == nullptr) {
        state_.mode = DroneMode::Completed;
        state_.velocity = {};
        return;
    }

    if (state_.mode == DroneMode::Idle) {
        state_.mode = mode_for_waypoint(*waypoint);
    }

    const Vec3 to_target = waypoint->position - state_.position;
    const double distance = to_target.length();
    const double acceptance = std::max(0.1, mission_.acceptance_radius_m);

    if (distance <= acceptance) {
        state_.position = waypoint->position;
        state_.velocity = {};

        if (waypoint->hold_seconds > 0.0 && state_.hold_elapsed_s < waypoint->hold_seconds) {
            state_.mode = DroneMode::Loiter;
            state_.hold_elapsed_s += dt_s;
        } else {
            advance_waypoint();
        }

        state_.battery_percent -= config_.idle_battery_drain_percent_per_s * dt_s;
        state_.battery_percent = std::max(0.0, state_.battery_percent);
        return;
    }

    const double speed = waypoint->speed_mps.value_or(mission_.cruise_speed_mps);
    const double travel_distance = std::min(distance, std::max(0.1, speed) * dt_s);
    const Vec3 direction = to_target.normalized();
    const Vec3 previous = state_.position;

    state_.position += direction * travel_distance;
    state_.velocity = (state_.position - previous) / dt_s;
    state_.mode = mode_for_waypoint(*waypoint);

    if (state_.velocity.horizontal_length() > 0.001) {
        state_.yaw_rad = std::atan2(state_.velocity.x, state_.velocity.z);
    }
    state_.pitch_rad = std::atan2(state_.velocity.y, std::max(0.001, state_.velocity.horizontal_length()));
    state_.roll_rad = std::sin(state_.mission_time_s * 2.0 * kPi * 0.35) * 0.035;

    const double movement_factor = std::clamp(state_.velocity.length() / std::max(0.1, mission_.cruise_speed_mps), 0.0, 2.0);
    state_.battery_percent -= (config_.flight_battery_drain_percent_per_s * movement_factor) * dt_s;
    state_.battery_percent = std::max(0.0, state_.battery_percent);
}

void DroneSimulation::advance_waypoint() {
    ++state_.target_waypoint_index;
    state_.hold_elapsed_s = 0.0;

    if (state_.target_waypoint_index >= mission_.waypoints.size()) {
        state_.mode = DroneMode::Completed;
        return;
    }

    if (const Waypoint* waypoint = target_waypoint()) {
        state_.mode = mode_for_waypoint(*waypoint);
    }
}

const Waypoint* DroneSimulation::target_waypoint() const {
    if (state_.target_waypoint_index >= mission_.waypoints.size()) {
        return nullptr;
    }
    return &mission_.waypoints[state_.target_waypoint_index];
}

const char* to_string(DroneMode mode) {
    switch (mode) {
        case DroneMode::Idle:
            return "idle";
        case DroneMode::Takeoff:
            return "takeoff";
        case DroneMode::Flying:
            return "flying";
        case DroneMode::Loiter:
            return "loiter";
        case DroneMode::Landing:
            return "landing";
        case DroneMode::Completed:
            return "completed";
        case DroneMode::Failsafe:
            return "failsafe";
    }
    return "unknown";
}

} // namespace agbot::flight_sim
