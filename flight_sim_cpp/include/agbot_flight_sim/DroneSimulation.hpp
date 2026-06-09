#pragma once

#include "agbot_flight_sim/Mission.hpp"

#include <cstddef>

namespace agbot::flight_sim {

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

struct SimulationConfig {
    double min_battery_percent = 12.0;
    double idle_battery_drain_percent_per_s = 0.001;
    double flight_battery_drain_percent_per_s = 0.012;
    double max_step_s = 0.05;
    double max_horizontal_speed_mps = 18.0;
    double max_vertical_speed_mps = 6.0;
    double max_acceleration_mps2 = 12.0;
    double yaw_rate_radps = 1.4;
    double manual_takeoff_altitude_m = 20.0;
};

class DroneSimulation {
public:
    explicit DroneSimulation(Mission mission, SimulationConfig config = {});

    void reset();
    void step(double dt_s);
    void replace_mission(Mission mission);
    void set_control_mode(ControlMode mode);
    void set_manual_input(ManualControlInput input);
    void set_wind(Vec3 wind_mps);
    void arm();
    void disarm();

    [[nodiscard]] const Mission& mission() const;
    [[nodiscard]] Mission& mutable_mission();
    [[nodiscard]] const DroneState& state() const;
    [[nodiscard]] ControlMode control_mode() const;
    [[nodiscard]] Vec3 wind() const;
    [[nodiscard]] bool is_complete() const;
    [[nodiscard]] double progress() const;

private:
    void step_fixed(double dt_s);
    void step_autopilot(double dt_s);
    void step_manual(double dt_s);
    void move_towards_velocity(Vec3 desired_velocity, double dt_s);
    void advance_waypoint();
    [[nodiscard]] const Waypoint* target_waypoint() const;

    Mission mission_;
    SimulationConfig config_;
    DroneState state_;
    ManualControlInput manual_input_;
    Vec3 wind_mps_;
};

[[nodiscard]] const char* to_string(DroneMode mode);
[[nodiscard]] const char* to_string(ControlMode mode);

} // namespace agbot::flight_sim
