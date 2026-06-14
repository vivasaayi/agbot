#pragma once

#include "agbot_flight_sim/DeterministicRunner.hpp"
#include "agbot_flight_sim/DroneSimulation.hpp"
#include "agbot_flight_sim/Mission.hpp"

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>

namespace agbot::flight_sim {

enum class TwinCommandType {
    Arm,
    Disarm,
    Step,
    SetManualInput,
    SetWind,
    Abort,
};

struct FlightCommandV1 {
    std::string contract_version = kTwinContractVersion;
    std::string command_id;
    TwinCommandType command_type = TwinCommandType::Step;
    std::uint64_t issued_at_unix_ms = 0;
    std::string payload_json = "{}";
    std::uint32_t ack_timeout_ms = 1000;
    double step_duration_s = 0.0;
    ManualControlInput manual_input;
    Vec3 wind_mps;
};

struct TwinErrorV1 {
    std::string contract_version = kTwinContractVersion;
    std::string code;
    std::string message;
    bool retryable = false;

    [[nodiscard]] std::string to_json() const;
};

struct TelemetryV1 {
    std::string contract_version = kTwinContractVersion;
    std::string command_id;
    double time_s = 0.0;
    std::string mode;
    Vec3 position;
    Vec3 velocity;
    Vec3 attitude;
    double battery_percent = 0.0;
    std::size_t target_waypoint_index = 0;
    bool armed = false;

    [[nodiscard]] std::string to_json() const;
};

struct TwinCommandAckV1 {
    std::string contract_version = kTwinContractVersion;
    std::string command_id;
    bool accepted = false;
    std::optional<TwinErrorV1> error;
    std::optional<TelemetryV1> telemetry;

    [[nodiscard]] std::string to_json() const;
};

struct TwinBackendConfig {
    bool available = true;
    double max_command_step_s = 5.0;
};

class TwinBackend {
public:
    explicit TwinBackend(Mission mission, TwinBackendConfig config = {}, SimulationConfig simulation_config = {});

    [[nodiscard]] bool available() const;
    void set_available(bool available);
    [[nodiscard]] const DroneSimulation& simulation() const;

    [[nodiscard]] TwinCommandAckV1 dispatch(const FlightCommandV1& command);

private:
    [[nodiscard]] TwinCommandAckV1 reject(
        const FlightCommandV1& command,
        std::string code,
        std::string message,
        bool retryable) const;
    [[nodiscard]] TelemetryV1 telemetry_for(const FlightCommandV1& command) const;

    DroneSimulation simulation_;
    TwinBackendConfig config_;
};

[[nodiscard]] const char* to_string(TwinCommandType command_type);

} // namespace agbot::flight_sim
