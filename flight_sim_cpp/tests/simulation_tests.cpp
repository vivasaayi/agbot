#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/MissionLoader.hpp"

#include <cassert>
#include <cmath>
#include <iostream>

using agbot::flight_sim::DroneMode;
using agbot::flight_sim::DroneSimulation;
using agbot::flight_sim::MissionLoader;

namespace {

const char* kMissionJson = R"json(
{
  "name": "Unit Test Mission",
  "home": { "x": 0.0, "y": 0.0, "z": 0.0 },
  "cruise_speed_mps": 10.0,
  "acceptance_radius_m": 0.5,
  "waypoints": [
    { "name": "takeoff", "action": "takeoff", "x": 0.0, "y": 10.0, "z": 0.0 },
    { "name": "leg_1", "action": "fly", "x": 20.0, "y": 10.0, "z": 0.0 },
    { "name": "hover", "action": "loiter", "x": 20.0, "y": 10.0, "z": 20.0, "hold_seconds": 1.0 },
    { "name": "land", "action": "land", "x": 0.0, "y": 0.0, "z": 0.0 }
  ]
}
)json";

void test_loads_mission() {
    const auto mission = MissionLoader::load_from_text(kMissionJson);
    assert(mission.name == "Unit Test Mission");
    assert(mission.waypoints.size() == 4);
    assert(std::abs(mission.cruise_speed_mps - 10.0) < 1e-9);
    assert(std::abs(mission.acceptance_radius_m - 0.5) < 1e-9);
}

void test_mission_completes() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    DroneSimulation simulation(std::move(mission));

    constexpr double dt_s = 1.0 / 60.0;
    for (int i = 0; i < 60 * 30 && !simulation.is_complete(); ++i) {
        simulation.step(dt_s);
    }

    assert(simulation.state().mode == DroneMode::Completed);
    assert(simulation.state().target_waypoint_index == 4);
    assert(simulation.state().battery_percent > 90.0);
}

void test_failsafe_low_battery() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    agbot::flight_sim::SimulationConfig config;
    config.min_battery_percent = 99.99;
    DroneSimulation simulation(std::move(mission), config);
    simulation.step(1.0);
    assert(simulation.state().mode == DroneMode::Failsafe);
}

} // namespace

int main() {
    test_loads_mission();
    test_mission_completes();
    test_failsafe_low_battery();
    std::cout << "agbot_flight_sim_tests passed\n";
    return 0;
}
