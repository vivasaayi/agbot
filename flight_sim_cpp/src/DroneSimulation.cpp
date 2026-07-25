#include "agbot_flight_sim/DroneSimulation.hpp"

#include "agbot_vehicles/MultirotorModel.hpp"

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
    if (config_.plant_model == PlantModel::Multirotor) {
        multirotor_model_ = std::make_unique<agbot::vehicles::MultirotorModel>();
    }
    refresh_home_ground_elevation();
    reset();
}

DroneSimulation::~DroneSimulation() = default;
DroneSimulation::DroneSimulation(DroneSimulation&&) noexcept = default;
DroneSimulation& DroneSimulation::operator=(DroneSimulation&&) noexcept = default;

void DroneSimulation::reset() {
    state_ = {};
    state_.position = mission_.home;
    state_.mode = DroneMode::Idle;
    state_.control_mode = ControlMode::Autopilot;
    manual_input_ = {};
    guidance_state_.reset();
    actuator_response_factor_ = 1.0;
    if (multirotor_model_) {
        multirotor_model_->clear_velocity_setpoint();
        multirotor_model_->set_response_factor(1.0);
    }
    emergency_abort_requested_ = false;
    last_safety_violation_.reset();
    event_log_.clear();
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
    // Terrain meshes are expressed in the previous mission's local frame.
    // A replacement mission must explicitly load/re-align its own surface.
    config_.terrain.reset();
    refresh_home_ground_elevation();
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

void DroneSimulation::set_guidance_state(std::optional<DroneState> state) {
    guidance_state_ = std::move(state);
}

void DroneSimulation::clear_guidance_state() {
    guidance_state_.reset();
}

void DroneSimulation::inject_battery_drop(double percent) {
    state_.battery_percent = std::max(0.0, state_.battery_percent - std::max(0.0, percent));
}

void DroneSimulation::set_actuator_response_factor(double factor) {
    actuator_response_factor_ = std::clamp(factor, 0.0, 1.0);
}

void DroneSimulation::set_wind(Vec3 wind_mps) {
    wind_mps_ = wind_mps;
}

void DroneSimulation::set_terrain(std::optional<TerrainMesh> terrain) {
    config_.terrain = std::move(terrain);
    refresh_home_ground_elevation();
}

void DroneSimulation::request_emergency_abort() {
    emergency_abort_requested_ = true;
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
    const auto ground_y = ground_local_y(state_.position);
    if (ground_y.has_value() &&
        state_.position.y <= *ground_y + 0.05) {
        state_.position.y = *ground_y;
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

std::optional<double> DroneSimulation::ground_elevation_m(
    Vec3 local_position) const {
    if (!config_.terrain.has_value()) {
        return 0.0;
    }
    return terrain_height_at(
        *config_.terrain, local_position.x, local_position.z);
}

std::optional<double> DroneSimulation::ground_local_y(
    Vec3 local_position) const {
    const auto elevation = ground_elevation_m(local_position);
    if (!elevation.has_value()) {
        return std::nullopt;
    }
    return *elevation - home_ground_elevation_m_;
}

std::optional<double> DroneSimulation::altitude_agl_m() const {
    return altitude_agl_m(state_.position);
}

std::optional<double> DroneSimulation::altitude_agl_m(
    Vec3 local_position) const {
    const auto ground_y = ground_local_y(local_position);
    if (!ground_y.has_value()) {
        return std::nullopt;
    }
    return local_position.y - *ground_y;
}

double DroneSimulation::world_elevation_m(Vec3 local_position) const {
    return local_position.y + home_ground_elevation_m_;
}

std::optional<Vec3> DroneSimulation::try_resolved_waypoint_position(
    const Waypoint& waypoint) const {
    Vec3 target = waypoint.position;
    const auto ground_y = ground_local_y(waypoint.position);
    if (!ground_y.has_value()) {
        return std::nullopt;
    }

    if (waypoint.action == WaypointAction::Land) {
        target.y = *ground_y;
        return target;
    }

    switch (mission_.altitude_reference) {
        case AltitudeReference::AboveGroundLevel:
            target.y = *ground_y + waypoint.position.y;
            break;
        case AltitudeReference::RelativeHome:
            target.y = waypoint.position.y;
            break;
        case AltitudeReference::MeanSeaLevel:
            target.y =
                waypoint.position.y - home_ground_elevation_m_;
            break;
    }
    return target;
}

Vec3 DroneSimulation::resolved_waypoint_position(
    const Waypoint& waypoint) const {
    const auto target = try_resolved_waypoint_position(waypoint);
    if (!target.has_value()) {
        throw std::runtime_error(
            "Waypoint is outside the configured terrain surface");
    }
    return *target;
}

const std::vector<SimulationEvent>& DroneSimulation::events() const {
    return event_log_;
}

std::vector<SimulationEvent> DroneSimulation::drain_events() {
    std::vector<SimulationEvent> drained = std::move(event_log_);
    event_log_.clear();
    return drained;
}

void DroneSimulation::clear_events() {
    event_log_.clear();
}

const std::optional<SafetyViolation>& DroneSimulation::last_safety_violation() const {
    return last_safety_violation_;
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

    if (fail_if_safety_violated()) {
        return;
    }

    state_.mission_time_s += dt_s;

    if (state_.control_mode == ControlMode::Manual) {
        step_manual(dt_s);
    } else {
        step_autopilot(dt_s);
    }

    if (state_.mode == DroneMode::Failsafe) {
        return;
    }
    if (fail_if_safety_violated()) {
        return;
    }

    emit_normal_event_frame();
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

    if (waypoint->action == WaypointAction::Land &&
        fail_if_landing_site_unsafe(waypoint->position)) {
        return;
    }

    const auto target_position =
        try_resolved_waypoint_position(*waypoint);
    if (!target_position.has_value()) {
        fail_for_terrain(
            SafetyViolationCode::TerrainUnavailable,
            "Waypoint is outside the configured terrain surface");
        return;
    }
    const DroneState& guidance_state = guidance_state_.has_value() ? *guidance_state_ : state_;
    const Vec3 to_target = *target_position - guidance_state.position;
    const double distance = to_target.length();
    const double acceptance = std::max(0.1, mission_.acceptance_radius_m);

    if (distance <= acceptance) {
        state_.position = *target_position;
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
    if (move_towards_velocity(
            desired_velocity,
            dt_s,
            waypoint->action == WaypointAction::Land)) {
        return;
    }
    state_.mode = mode_for_waypoint(*waypoint);

    const double remaining_after_move =
        (*target_position - state_.position).length();
    if (remaining_after_move <= acceptance) {
        state_.position = *target_position;
        state_.velocity = {};
    }
}

void DroneSimulation::step_manual(double dt_s) {
    if (manual_input_.arm) {
        arm();
    }

    if (!state_.armed) {
        state_.mode = DroneMode::Idle;
        if (move_towards_velocity({}, dt_s, true)) {
            return;
        }
        state_.battery_percent -= config_.idle_battery_drain_percent_per_s * dt_s;
        return;
    }

    state_.yaw_rad += manual_input_.yaw * config_.yaw_rate_radps * dt_s;

    const Vec3 forward(std::sin(state_.yaw_rad), 0.0, std::cos(state_.yaw_rad));
    const Vec3 right(std::cos(state_.yaw_rad), 0.0, -std::sin(state_.yaw_rad));

    const auto current_agl = altitude_agl_m();
    if (!current_agl.has_value()) {
        fail_for_terrain(
            SafetyViolationCode::TerrainUnavailable,
            "Aircraft is outside the configured terrain surface");
        return;
    }

    double vertical_axis = manual_input_.throttle;
    if (manual_input_.takeoff &&
        *current_agl < config_.manual_takeoff_altitude_m) {
        vertical_axis = 0.75;
        state_.mode = DroneMode::Takeoff;
    } else if (manual_input_.land) {
        if (fail_if_landing_site_unsafe(state_.position)) {
            return;
        }
        vertical_axis = -0.55;
        state_.mode = DroneMode::Landing;
    } else if (std::abs(vertical_axis) < 0.02 &&
               *current_agl > 0.05) {
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

    if (move_towards_velocity(
            desired_velocity,
            dt_s,
            manual_input_.land || desired_velocity.y < 0.0)) {
        return;
    }

    const auto agl_after_move = altitude_agl_m();
    const auto ground_y = ground_local_y(state_.position);
    if (agl_after_move.has_value() && ground_y.has_value() &&
        *agl_after_move <= 0.0) {
        state_.position.y = *ground_y;
        if (manual_input_.land || desired_velocity.y < 0.0) {
            state_.velocity = {};
            state_.mode = DroneMode::Idle;
            state_.armed = false;
        }
    }
}

bool DroneSimulation::move_towards_velocity(
    Vec3 desired_velocity,
    double dt_s,
    bool allow_ground_contact) {
    const auto initial_agl = altitude_agl_m();
    if (!initial_agl.has_value()) {
        return fail_for_terrain(
            SafetyViolationCode::TerrainUnavailable,
            "Aircraft is outside the configured terrain surface");
    }

    if (multirotor_model_) {
        agbot::vehicles::EntityState plant_state;
        plant_state.position = state_.position;
        plant_state.velocity = state_.velocity;
        plant_state.yaw_rad = state_.yaw_rad;
        plant_state.pitch_rad = state_.pitch_rad;
        plant_state.roll_rad = state_.roll_rad;
        plant_state.time_s = state_.mission_time_s;

        multirotor_model_->set_velocity_setpoint(desired_velocity);
        multirotor_model_->set_response_factor(actuator_response_factor_);
        const auto next = multirotor_model_->step(plant_state, {}, dt_s);
        state_.position = next.position;
        state_.velocity = next.velocity;
        state_.yaw_rad = next.yaw_rad;
        state_.pitch_rad = next.pitch_rad;
        state_.roll_rad = next.roll_rad;
    } else {
        const Vec3 delta = desired_velocity - state_.velocity;
        state_.velocity += clamp_vector_delta(
            delta,
            config_.max_acceleration_mps2 * actuator_response_factor_ * dt_s);
        state_.position += state_.velocity * dt_s;
    }

    if (*initial_agl > 0.05 || desired_velocity.y > 0.0) {
        state_.position += wind_mps_ * dt_s;
    }

    const auto ground_y = ground_local_y(state_.position);
    if (!ground_y.has_value()) {
        return fail_for_terrain(
            SafetyViolationCode::TerrainUnavailable,
            "Aircraft left the configured terrain surface");
    }
    if (state_.position.y < *ground_y) {
        state_.position.y = *ground_y;
        state_.velocity.y = 0.0;
        if (!allow_ground_contact) {
            return fail_for_terrain(
                SafetyViolationCode::TerrainCollision,
                "Aircraft intersected the terrain surface");
        }
        if (fail_if_landing_site_unsafe(state_.position)) {
            return true;
        }
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
    return false;
}

bool DroneSimulation::fail_for_terrain(
    SafetyViolationCode code,
    std::string message) {
    last_safety_violation_ = SafetyViolation {
        code,
        to_string(code),
        state_.target_waypoint_index,
        std::move(message),
    };
    state_.mode = DroneMode::Failsafe;
    state_.velocity = {};
    state_.armed = false;
    emit_event(
        SimulationEventType::Emergency,
        last_safety_violation_->message,
        code);
    return true;
}

bool DroneSimulation::fail_if_landing_site_unsafe(
    Vec3 local_position) {
    if (!config_.terrain.has_value()) {
        return false;
    }
    const auto slope = terrain_slope_degrees_at(
        *config_.terrain, local_position.x, local_position.z);
    if (!slope.has_value()) {
        return fail_for_terrain(
            SafetyViolationCode::TerrainUnavailable,
            "Landing zone is outside the configured terrain surface");
    }
    if (*slope > config_.max_landing_slope_deg) {
        return fail_for_terrain(
            SafetyViolationCode::UnsafeLandingSlope,
            "Landing zone exceeds the configured slope limit");
    }
    return false;
}

void DroneSimulation::refresh_home_ground_elevation() {
    if (!config_.terrain.has_value()) {
        home_ground_elevation_m_ = 0.0;
        return;
    }
    const auto elevation = terrain_height_at(
        *config_.terrain, mission_.home.x, mission_.home.z);
    if (!elevation.has_value()) {
        throw std::invalid_argument(
            "Configured terrain does not cover the mission home");
    }
    home_ground_elevation_m_ = *elevation;
}

bool DroneSimulation::fail_if_safety_violated() {
    SafetyEnvelope envelope = config_.safety;
    envelope.min_battery_percent = config_.min_battery_percent;
    const auto agl = altitude_agl_m();
    if (!agl.has_value()) {
        return fail_for_terrain(
            SafetyViolationCode::TerrainUnavailable,
            "Aircraft is outside the configured terrain surface");
    }
    Vec3 safety_position = state_.position;
    safety_position.y = *agl;
    const SafetySample sample {
        safety_position,
        state_.battery_percent,
        state_.target_waypoint_index,
        emergency_abort_requested_,
    };

    if (auto violation = evaluate_safety(sample, envelope)) {
        last_safety_violation_ = *violation;
        state_.mode = DroneMode::Failsafe;
        state_.velocity = {};
        state_.armed = false;
        emit_event(SimulationEventType::Emergency, violation->message, violation->code);
        return true;
    }
    return false;
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

void DroneSimulation::emit_event(
    SimulationEventType type,
    std::string message,
    std::optional<SafetyViolationCode> safety_code) {
    event_log_.push_back({
        type,
        state_.mission_time_s,
        state_.mode,
        state_.position,
        state_.velocity,
        state_.battery_percent,
        state_.target_waypoint_index,
        safety_code,
        std::move(message),
    });
}

void DroneSimulation::emit_normal_event_frame() {
    emit_event(SimulationEventType::Position, "position sample broadcast");
    emit_event(SimulationEventType::Sensor, "sensor sample broadcast");
    emit_event(SimulationEventType::Battery, "battery sample broadcast");
    emit_event(SimulationEventType::Status, "status sample broadcast");
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

const char* to_string(PlantModel model) {
    switch (model) {
        case PlantModel::Simple:
            return "simple";
        case PlantModel::Multirotor:
            return "multirotor";
    }
    return "unknown";
}

PlantModel plant_model_from_string(std::string_view value) {
    if (value == "simple") {
        return PlantModel::Simple;
    }
    if (value == "multirotor") {
        return PlantModel::Multirotor;
    }
    throw std::invalid_argument("unknown plant model: " + std::string(value));
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

const char* to_string(SimulationEventType type) {
    switch (type) {
        case SimulationEventType::Position:
            return "position";
        case SimulationEventType::Sensor:
            return "sensor";
        case SimulationEventType::Battery:
            return "battery";
        case SimulationEventType::Status:
            return "status";
        case SimulationEventType::Emergency:
            return "emergency";
    }
    return "unknown";
}

} // namespace agbot::flight_sim
