#include "agbot_flight_sim/DroneSimulation.hpp"

#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <utility>

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

Vec3 clamp_vector_delta(Vec3 delta, double max_length) {
    const double length = delta.length();
    if (length <= max_length || length <= 1e-9) {
        return delta;
    }
    return delta.normalized() * max_length;
}

double clamp_axis(double value) {
    return std::clamp(value, -1.0, 1.0);
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
    state_.control_mode = ControlMode::Autopilot;
    manual_input_ = {};
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

void DroneSimulation::replace_mission(Mission mission) {
    if (mission.waypoints.empty()) {
        throw std::runtime_error("Replacement mission must contain at least one waypoint");
    }
    mission_ = std::move(mission);
    reset();
}

void DroneSimulation::set_control_mode(ControlMode mode) {
    state_.control_mode = mode;
    if (mode == ControlMode::Manual && state_.mode == DroneMode::Completed) {
        state_.mode = DroneMode::Idle;
    }
}

void DroneSimulation::set_manual_input(ManualControlInput input) {
    input.throttle = clamp_axis(input.throttle);
    input.yaw = clamp_axis(input.yaw);
    input.pitch = clamp_axis(input.pitch);
    input.roll = clamp_axis(input.roll);
    manual_input_ = input;
}

void DroneSimulation::set_wind(Vec3 wind_mps) {
    wind_mps_ = wind_mps;
}

void DroneSimulation::arm() {
    state_.armed = true;
    if (state_.mode == DroneMode::Idle) {
        state_.mode = DroneMode::Hovering;
    }
}

void DroneSimulation::disarm() {
    state_.armed = false;
    state_.velocity = {};
    if (state_.position.y <= 0.05) {
        state_.position.y = 0.0;
        state_.mode = DroneMode::Idle;
    }
}

const Mission& DroneSimulation::mission() const {
    return mission_;
}

Mission& DroneSimulation::mutable_mission() {
    return mission_;
}

const DroneState& DroneSimulation::state() const {
    return state_;
}

ControlMode DroneSimulation::control_mode() const {
    return state_.control_mode;
}

Vec3 DroneSimulation::wind() const {
    return wind_mps_;
}

bool DroneSimulation::is_complete() const {
    if (state_.control_mode == ControlMode::Manual) {
        return state_.mode == DroneMode::Failsafe;
    }
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
    if (state_.mode == DroneMode::Failsafe || state_.control_mode == ControlMode::Replay) {
        return;
    }

    state_.mission_time_s += dt_s;

    if (state_.battery_percent <= config_.min_battery_percent) {
        state_.mode = DroneMode::Failsafe;
        state_.velocity = {};
        return;
    }

    if (state_.control_mode == ControlMode::Manual) {
        step_manual(dt_s);
    } else {
        step_autopilot(dt_s);
    }
}

void DroneSimulation::step_autopilot(double dt_s) {
    if (state_.mode == DroneMode::Completed) {
        return;
    }

    const Waypoint* waypoint = target_waypoint();
    if (waypoint == nullptr) {
        state_.mode = DroneMode::Completed;
        state_.velocity = {};
        return;
    }

    if (!state_.armed) {
        state_.armed = true;
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

    const double speed = std::min(waypoint->speed_mps.value_or(mission_.cruise_speed_mps), config_.max_horizontal_speed_mps);
    const Vec3 desired_velocity = to_target.normalized() * std::max(0.1, speed);
    move_towards_velocity(desired_velocity, dt_s);
    state_.mode = mode_for_waypoint(*waypoint);

    const double remaining_after_move = (waypoint->position - state_.position).length();
    if (remaining_after_move <= acceptance) {
        state_.position = waypoint->position;
        state_.velocity = {};
    }
}

void DroneSimulation::step_manual(double dt_s) {
    if (manual_input_.arm) {
        arm();
    }

    if (!state_.armed) {
        state_.mode = DroneMode::Idle;
        move_towards_velocity({}, dt_s);
        state_.battery_percent -= config_.idle_battery_drain_percent_per_s * dt_s;
        return;
    }

    state_.yaw_rad += manual_input_.yaw * config_.yaw_rate_radps * dt_s;

    const Vec3 forward(std::sin(state_.yaw_rad), 0.0, std::cos(state_.yaw_rad));
    const Vec3 right(std::cos(state_.yaw_rad), 0.0, -std::sin(state_.yaw_rad));

    double vertical_axis = manual_input_.throttle;
    if (manual_input_.takeoff && state_.position.y < config_.manual_takeoff_altitude_m) {
        vertical_axis = 0.75;
        state_.mode = DroneMode::Takeoff;
    } else if (manual_input_.land) {
        vertical_axis = -0.55;
        state_.mode = DroneMode::Landing;
    } else if (std::abs(vertical_axis) < 0.02 && state_.position.y > 0.05) {
        state_.mode = DroneMode::Hovering;
    } else {
        state_.mode = DroneMode::Flying;
    }

    const Vec3 horizontal = (forward * manual_input_.pitch + right * manual_input_.roll) * config_.max_horizontal_speed_mps;
    Vec3 desired_velocity(
        horizontal.x,
        vertical_axis * config_.max_vertical_speed_mps,
        horizontal.z
    );

    move_towards_velocity(desired_velocity, dt_s);

    if (state_.position.y <= 0.0) {
        state_.position.y = 0.0;
        if (manual_input_.land || desired_velocity.y < 0.0) {
            state_.velocity = {};
            state_.mode = DroneMode::Idle;
            state_.armed = false;
        }
    }
}

void DroneSimulation::move_towards_velocity(Vec3 desired_velocity, double dt_s) {
    const Vec3 delta = desired_velocity - state_.velocity;
    state_.velocity += clamp_vector_delta(delta, config_.max_acceleration_mps2 * dt_s);

    Vec3 ground_velocity = state_.velocity;
    if (state_.position.y > 0.05 || desired_velocity.y > 0.0) {
        ground_velocity += wind_mps_;
    }

    state_.position += ground_velocity * dt_s;

    if (state_.position.y < 0.0) {
        state_.position.y = 0.0;
    }

    if (state_.velocity.horizontal_length() > 0.001) {
        state_.yaw_rad = std::atan2(state_.velocity.x, state_.velocity.z);
    }
    state_.pitch_rad = std::atan2(state_.velocity.y, std::max(0.001, state_.velocity.horizontal_length()));
    state_.roll_rad = std::clamp(state_.velocity.horizontal_length() / std::max(1.0, config_.max_horizontal_speed_mps), 0.0, 1.0)
        * std::sin(state_.mission_time_s * 2.0 * kPi * 0.35) * 0.08;

    const double movement_factor = std::clamp(state_.velocity.length() / std::max(0.1, mission_.cruise_speed_mps), 0.0, 2.0);
    state_.battery_percent -= (config_.flight_battery_drain_percent_per_s * std::max(0.25, movement_factor)) * dt_s;
    state_.battery_percent = std::max(0.0, state_.battery_percent);
}

void DroneSimulation::advance_waypoint() {
    ++state_.target_waypoint_index;
    state_.hold_elapsed_s = 0.0;

    if (state_.target_waypoint_index >= mission_.waypoints.size()) {
        state_.mode = DroneMode::Completed;
        state_.armed = false;
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
        case DroneMode::Hovering:
            return "hovering";
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

const char* to_string(ControlMode mode) {
    switch (mode) {
        case ControlMode::Autopilot:
            return "autopilot";
        case ControlMode::Manual:
            return "manual";
        case ControlMode::Replay:
            return "replay";
    }
    return "unknown";
}

} // namespace agbot::flight_sim
