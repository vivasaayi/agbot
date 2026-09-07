#pragma once

#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_flight_sim/Mission.hpp"
#include "agbot_flight_sim/SafetyRules.hpp"

#include <cstddef>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace agbot::vehicles {
class MultirotorModel;
}

namespace agbot::flight_sim {

enum class PlantModel {
    Simple,
    Multirotor,
};

enum class DroneMode {
    Idle,
    Takeoff,
    Flying,
    Hovering,
    Loiter,
    Landing,
    Completed,
    Failsafe,
};

enum class ControlMode {
    Autopilot,
    Manual,
    Replay,
};

struct ManualControlInput {
    double throttle = 0.0; // -1 descent, +1 climb
    double yaw = 0.0;      // -1 left, +1 right
    double pitch = 0.0;    // -1 backward, +1 forward
    double roll = 0.0;     // -1 left, +1 right
    bool takeoff = false;
    bool land = false;
    bool arm = false;
};

struct DroneState {
    Vec3 position;
    Vec3 velocity;
    double yaw_rad = 0.0;
    double pitch_rad = 0.0;
    double roll_rad = 0.0;
    double battery_percent = 100.0;
    double mission_time_s = 0.0;
    std::size_t target_waypoint_index = 0;
    double hold_elapsed_s = 0.0;
    DroneMode mode = DroneMode::Idle;
    ControlMode control_mode = ControlMode::Autopilot;
    bool armed = false;
};

enum class SimulationEventType {
    Position,
    Sensor,
    Battery,
    Status,
    Emergency,
};

struct SimulationEvent {
    SimulationEventType type = SimulationEventType::Status;
    double time_s = 0.0;
    DroneMode mode = DroneMode::Idle;
    Vec3 position;
    Vec3 velocity;
    double battery_percent = 100.0;
    std::size_t target_waypoint_index = 0;
    std::optional<SafetyViolationCode> safety_code;
    std::string message;
};

struct SimulationConfig {
    PlantModel plant_model = PlantModel::Simple;
    double min_battery_percent = 12.0;
    double idle_battery_drain_percent_per_s = 0.001;
    double flight_battery_drain_percent_per_s = 0.012;
    double max_step_s = 0.05;
    double max_horizontal_speed_mps = 18.0;
    double max_vertical_speed_mps = 6.0;
    double max_acceleration_mps2 = 12.0;
    double yaw_rate_radps = 1.4;
    double manual_takeoff_altitude_m = 20.0;
    double max_landing_slope_deg = 15.0;
    SafetyEnvelope safety;
    std::optional<TerrainMesh> terrain;
};

class DroneSimulation {
public:
    explicit DroneSimulation(Mission mission, SimulationConfig config = {});
    ~DroneSimulation();
    DroneSimulation(DroneSimulation&&) noexcept;
    DroneSimulation& operator=(DroneSimulation&&) noexcept;
    DroneSimulation(const DroneSimulation&) = delete;
    DroneSimulation& operator=(const DroneSimulation&) = delete;

    void reset();
    void step(double dt_s);
    void replace_mission(Mission mission);
    void set_control_mode(ControlMode mode);
    void set_manual_input(ManualControlInput input);
    void set_guidance_state(std::optional<DroneState> state);
    void clear_guidance_state();
    void inject_battery_drop(double percent);
    void set_actuator_response_factor(double factor);
    void set_wind(Vec3 wind_mps);
    void set_terrain(std::optional<TerrainMesh> terrain);
    void request_emergency_abort();
    void arm();
    void disarm();

    [[nodiscard]] const Mission& mission() const;
    [[nodiscard]] Mission& mutable_mission();
    [[nodiscard]] const DroneState& state() const;
    [[nodiscard]] ControlMode control_mode() const;
    [[nodiscard]] Vec3 wind() const;
    [[nodiscard]] std::optional<double> ground_elevation_m(
        Vec3 local_position) const;
    [[nodiscard]] std::optional<double> altitude_agl_m() const;
    [[nodiscard]] std::optional<double> altitude_agl_m(
        Vec3 local_position) const;
    [[nodiscard]] double world_elevation_m(Vec3 local_position) const;
    [[nodiscard]] Vec3 resolved_waypoint_position(
        const Waypoint& waypoint) const;
    [[nodiscard]] const std::vector<SimulationEvent>& events() const;
    [[nodiscard]] std::vector<SimulationEvent> drain_events();
    void clear_events();
    [[nodiscard]] const std::optional<SafetyViolation>& last_safety_violation() const;
    [[nodiscard]] bool is_complete() const;
    [[nodiscard]] double progress() const;

private:
    void step_fixed(double dt_s);
    void step_autopilot(double dt_s);
    void step_manual(double dt_s);
    bool move_towards_velocity(
        Vec3 desired_velocity,
        double dt_s,
        bool allow_ground_contact = false);
    bool fail_if_safety_violated();
    bool fail_for_terrain(
        SafetyViolationCode code,
        std::string message);
    bool fail_if_landing_site_unsafe(Vec3 local_position);
    void refresh_home_ground_elevation();
    [[nodiscard]] std::optional<double> ground_local_y(
        Vec3 local_position) const;
    [[nodiscard]] std::optional<Vec3> try_resolved_waypoint_position(
        const Waypoint& waypoint) const;
    void advance_waypoint();
    void emit_event(
        SimulationEventType type,
        std::string message = {},
        std::optional<SafetyViolationCode> safety_code = std::nullopt);
    void emit_normal_event_frame();
    [[nodiscard]] const Waypoint* target_waypoint() const;

    Mission mission_;
    SimulationConfig config_;
    DroneState state_;
    ManualControlInput manual_input_;
    Vec3 wind_mps_;
    std::optional<DroneState> guidance_state_;
    double actuator_response_factor_ = 1.0;
    std::unique_ptr<agbot::vehicles::MultirotorModel> multirotor_model_;
    bool emergency_abort_requested_ = false;
    std::optional<SafetyViolation> last_safety_violation_;
    std::vector<SimulationEvent> event_log_;
    double home_ground_elevation_m_ = 0.0;
};

[[nodiscard]] const char* to_string(DroneMode mode);
[[nodiscard]] const char* to_string(ControlMode mode);
[[nodiscard]] const char* to_string(SimulationEventType type);
[[nodiscard]] const char* to_string(PlantModel model);
[[nodiscard]] PlantModel plant_model_from_string(std::string_view value);

} // namespace agbot::flight_sim
