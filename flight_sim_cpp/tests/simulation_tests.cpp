#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/MissionLoader.hpp"
#include "agbot_flight_sim/TelemetryRecorder.hpp"
#include "agbot_flight_sim/TelemetryReplay.hpp"

#include <cassert>
#include <cmath>
#include <filesystem>
#include <iostream>

using agbot::flight_sim::DroneMode;
using agbot::flight_sim::DroneSimulation;
using agbot::flight_sim::MissionLoader;
using agbot::flight_sim::ManualControlInput;
using agbot::flight_sim::ControlMode;
using agbot::flight_sim::TelemetryRecorder;
using agbot::flight_sim::TelemetryReplay;

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
    for (int i = 0; i < 60 * 45 && !simulation.is_complete(); ++i) {
        simulation.step(dt_s);
    }

    assert(simulation.state().mode == DroneMode::Completed);
    assert(simulation.state().target_waypoint_index == 4);
    assert(simulation.state().battery_percent > 90.0);
}

void test_manual_controls_move_drone() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    DroneSimulation simulation(std::move(mission));
    simulation.set_control_mode(ControlMode::Manual);
    simulation.arm();

    ManualControlInput input;
    input.arm = true;
    input.takeoff = true;
    input.pitch = 1.0;
    simulation.set_manual_input(input);

    for (int i = 0; i < 60 * 3; ++i) {
        simulation.step(1.0 / 60.0);
    }

    assert(simulation.state().position.y > 5.0);
    assert(simulation.state().position.z > 5.0);
    assert(simulation.state().mode != DroneMode::Completed);
}

void test_mission_round_trip() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    const std::string json = agbot::flight_sim::mission_to_json(mission);
    const auto reloaded = MissionLoader::load_from_text(json);
    assert(reloaded.name == mission.name);
    assert(reloaded.waypoints.size() == mission.waypoints.size());
}

void test_failsafe_low_battery() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    agbot::flight_sim::SimulationConfig config;
    config.min_battery_percent = 100.0;
    DroneSimulation simulation(std::move(mission), config);
    simulation.step(1.0);
    assert(simulation.state().mode == DroneMode::Failsafe);
}

void test_telemetry_recorder_close_is_idempotent() {
    auto mission = MissionLoader::load_from_text(kMissionJson);
    DroneSimulation simulation(std::move(mission));
    const auto output = std::filesystem::temp_directory_path() / "agbot_flight_sim_recorder_test.jsonl";

    TelemetryRecorder recorder(output);
    recorder.write_sample(simulation.state());
    assert(recorder.is_open());
    recorder.close();
    assert(!recorder.is_open());
    recorder.close();

    const auto replay = TelemetryReplay::load_jsonl(output);
    assert(!replay.empty());
    std::filesystem::remove(output);
}

} // namespace

int main() {
    test_loads_mission();
    test_mission_completes();
    test_manual_controls_move_drone();
    test_mission_round_trip();
    test_failsafe_low_battery();
    test_telemetry_recorder_close_is_idempotent();
    std::cout << "agbot_flight_sim_tests passed\n";
    return 0;
}
