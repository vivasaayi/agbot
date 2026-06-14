#include "agbot_flight_sim/HudTelemetry.hpp"

#include <algorithm>
#include <cmath>

namespace agbot::flight_sim {
namespace {

constexpr double kRadiansToDegrees = 180.0 / 3.14159265358979323846;

double normalize_heading_deg(double heading_deg) {
    double normalized = std::fmod(heading_deg, 360.0);
    if (normalized < 0.0) {
        normalized += 360.0;
    }
    return normalized;
}

} // namespace

HudTelemetry hud_telemetry_from_state(
    const DroneState& state,
    ControlMode control_mode,
    double critical_battery_percent) {
    const double battery_percent = std::clamp(state.battery_percent, 0.0, 100.0);
    const bool battery_critical = battery_percent <= critical_battery_percent;
    HudUiState ui_state = HudUiState::Normal;
    if (state.mode == DroneMode::Failsafe || battery_critical) {
        ui_state = HudUiState::Emergency;
    }

    return HudTelemetry{
        normalize_heading_deg(state.yaw_rad * kRadiansToDegrees),
        state.velocity.length(),
        state.position.y,
        battery_percent,
        battery_critical,
        state.armed,
        state.mode,
        control_mode,
        ui_state,
    };
}

const char* to_string(HudUiState state) {
    switch (state) {
        case HudUiState::Normal:
            return "normal";
        case HudUiState::Emergency:
            return "emergency";
    }
    return "unknown";
}

} // namespace agbot::flight_sim
