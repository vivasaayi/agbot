#pragma once

#include "agbot_flight_sim/DroneSimulation.hpp"

#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace agbot::flight_sim {

enum class FaultClass {
    WindGust,
    GpsDrift,
    ImuNoise,
    SensorDropout,
    CommLoss,
    LowBattery,
    StaleTerrain,
    BadTile,
    ActuatorLag,
};

struct FaultSpec {
    FaultClass fault_class = FaultClass::GpsDrift;
    std::optional<std::uint64_t> seed;
    std::uint64_t start_step = 0;
    std::optional<std::uint64_t> end_step;
    double magnitude = 0.0;
    std::string target;
};

struct FaultInjectionPlan {
    std::vector<FaultSpec> faults;

    [[nodiscard]] bool empty() const;
    [[nodiscard]] std::string to_json() const;
};

struct FaultEvent {
    FaultClass fault_class = FaultClass::GpsDrift;
    std::uint64_t seed = 0;
    std::uint64_t step = 0;
    std::string target;
    std::string action;
};

[[nodiscard]] const char* to_string(FaultClass fault_class);
[[nodiscard]] FaultClass fault_class_from_string(std::string_view value);
[[nodiscard]] FaultSpec parse_fault_spec(std::string_view text);
void validate_fault_plan(const FaultInjectionPlan& plan);
[[nodiscard]] bool is_fault_active(const FaultSpec& fault, std::uint64_t step);
[[nodiscard]] bool sensor_stream_suppressed(const FaultInjectionPlan& plan, std::uint64_t step);
[[nodiscard]] Vec3 wind_fault_for_step(const FaultInjectionPlan& plan, std::uint64_t step);
[[nodiscard]] DroneState apply_observation_faults(
    const DroneState& state,
    const FaultInjectionPlan& plan,
    std::uint64_t step);
void append_fault_events_for_step(
    const FaultInjectionPlan& plan,
    std::uint64_t step,
    std::vector<FaultEvent>& events);
[[nodiscard]] std::string fault_events_to_json(const std::vector<FaultEvent>& events);
[[nodiscard]] std::string terrain_tiles_json_for_faults(const FaultInjectionPlan& plan);

} // namespace agbot::flight_sim
