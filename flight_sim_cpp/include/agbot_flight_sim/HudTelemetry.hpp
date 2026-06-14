#pragma once

#include "agbot_flight_sim/DroneSimulation.hpp"

namespace agbot::flight_sim {

enum class HudUiState {
    Normal,
    Emergency,
};

struct HudTelemetry {
    double compass_heading_deg = 0.0;
    double speed_mps = 0.0;
    double altitude_m = 0.0;
    double battery_percent = 100.0;
    bool battery_critical = false;
    bool armed = false;
    DroneMode flight_mode = DroneMode::Idle;
    ControlMode control_mode = ControlMode::Autopilot;
    HudUiState ui_state = HudUiState::Normal;
};

[[nodiscard]] HudTelemetry hud_telemetry_from_state(
    const DroneState& state,
    ControlMode control_mode,
    double critical_battery_percent = 25.0);

[[nodiscard]] const char* to_string(HudUiState state);

} // namespace agbot::flight_sim
