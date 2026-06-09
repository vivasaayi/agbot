#pragma once

#include "agbot_flight_sim/Mission.hpp"

#include <cstddef>

namespace agbot::flight_sim {

enum class DroneMode {
    Idle,
    Takeoff,
    Flying,
    Loiter,
    Landing,
    Completed,
    Failsafe,
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
};

struct SimulationConfig {
    double min_battery_percent = 12.0;
    double idle_battery_drain_percent_per_s = 0.001;
    double flight_battery_drain_percent_per_s = 0.012;
    double max_step_s = 0.05;
};

class DroneSimulation {
public:
    explicit DroneSimulation(Mission mission, SimulationConfig config = {});

    void reset();
    void step(double dt_s);

    [[nodiscard]] const Mission& mission() const;
    [[nodiscard]] const DroneState& state() const;
    [[nodiscard]] bool is_complete() const;
    [[nodiscard]] double progress() const;

private:
    void step_fixed(double dt_s);
    void advance_waypoint();
    [[nodiscard]] const Waypoint* target_waypoint() const;

    Mission mission_;
    SimulationConfig config_;
    DroneState state_;
};

[[nodiscard]] const char* to_string(DroneMode mode);

} // namespace agbot::flight_sim
