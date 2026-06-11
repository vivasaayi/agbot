#include "agbot_flight_sim/FaultInjection.hpp"

#include <algorithm>
#include <charconv>
#include <cmath>
#include <iomanip>
#include <sstream>
#include <stdexcept>
#include <string>

namespace agbot::flight_sim {
namespace {

std::string escape_json(std::string_view value) {
    std::ostringstream output;
    for (const char c : value) {
        switch (c) {
            case '"':
                output << "\\\"";
                break;
            case '\\':
                output << "\\\\";
                break;
            case '\n':
                output << "\\n";
                break;
            case '\r':
                output << "\\r";
                break;
            case '\t':
                output << "\\t";
                break;
            default:
                output << c;
                break;
        }
    }
    return output.str();
}

std::vector<std::string_view> split_colon(std::string_view text) {
    std::vector<std::string_view> parts;
    std::size_t start = 0;
    while (start <= text.size()) {
        const std::size_t end = text.find(':', start);
        if (end == std::string_view::npos) {
            parts.push_back(text.substr(start));
            break;
        }
        parts.push_back(text.substr(start, end - start));
        start = end + 1;
    }
    return parts;
}

std::uint64_t parse_u64(std::string_view text, std::string_view field) {
    std::uint64_t value = 0;
    const char* begin = text.data();
    const char* end = text.data() + text.size();
    const auto [ptr, ec] = std::from_chars(begin, end, value);
    if (ec != std::errc {} || ptr != end) {
        throw std::runtime_error("invalid fault " + std::string(field) + ": " + std::string(text));
    }
    return value;
}

double parse_double(std::string_view text, std::string_view field) {
    std::string owned(text);
    std::size_t parsed = 0;
    const double value = std::stod(owned, &parsed);
    if (parsed != owned.size()) {
        throw std::runtime_error("invalid fault " + std::string(field) + ": " + std::string(text));
    }
    return value;
}

std::uint64_t mix64(std::uint64_t value) {
    value += 0x9e3779b97f4a7c15ULL;
    value = (value ^ (value >> 30U)) * 0xbf58476d1ce4e5b9ULL;
    value = (value ^ (value >> 27U)) * 0x94d049bb133111ebULL;
    return value ^ (value >> 31U);
}

double symmetric_unit(std::uint64_t seed, std::uint64_t step, std::uint64_t salt) {
    const std::uint64_t mixed = mix64(seed ^ (step * 0x9e3779b97f4a7c15ULL) ^ salt);
    const double unit = static_cast<double>(mixed >> 11U) * (1.0 / 9007199254740992.0);
    return unit * 2.0 - 1.0;
}

double default_magnitude(const FaultSpec& fault, double fallback) {
    return fault.magnitude > 0.0 ? fault.magnitude : fallback;
}

std::string fault_to_json(const FaultSpec& fault) {
    std::ostringstream output;
    output << std::fixed << std::setprecision(3)
           << "{\"class\":\"" << to_string(fault.fault_class) << "\"";
    if (fault.seed.has_value()) {
        output << ",\"seed\":" << *fault.seed;
    } else {
        output << ",\"seed\":null";
    }
    output << ",\"start_step\":" << fault.start_step;
    if (fault.end_step.has_value()) {
        output << ",\"end_step\":" << *fault.end_step;
    } else {
        output << ",\"end_step\":null";
    }
    output << ",\"magnitude\":" << fault.magnitude
           << ",\"target\":\"" << escape_json(fault.target) << "\""
           << "}";
    return output.str();
}

std::string event_to_json(const FaultEvent& event) {
    std::ostringstream output;
    output << "{\"class\":\"" << to_string(event.fault_class) << "\""
           << ",\"seed\":" << event.seed
           << ",\"step\":" << event.step
           << ",\"target\":\"" << escape_json(event.target) << "\""
           << ",\"action\":\"" << escape_json(event.action) << "\""
           << "}";
    return output.str();
}

bool already_recorded(
    const std::vector<FaultEvent>& events,
    FaultClass fault_class,
    std::uint64_t seed,
    std::uint64_t step) {
    return std::any_of(events.begin(), events.end(), [&](const FaultEvent& event) {
        return event.fault_class == fault_class && event.seed == seed && event.step == step;
    });
}

std::string default_target_for(FaultClass fault_class) {
    switch (fault_class) {
        case FaultClass::WindGust:
            return "wind";
        case FaultClass::GpsDrift:
            return "gps";
        case FaultClass::ImuNoise:
            return "imu";
        case FaultClass::SensorDropout:
            return "telemetry";
        case FaultClass::CommLoss:
            return "command_link";
        case FaultClass::LowBattery:
            return "battery";
        case FaultClass::StaleTerrain:
            return "terrain";
        case FaultClass::BadTile:
            return "terrain_tile";
        case FaultClass::ActuatorLag:
            return "actuator";
    }
    return "unknown";
}

} // namespace

bool FaultInjectionPlan::empty() const {
    return faults.empty();
}

std::string FaultInjectionPlan::to_json() const {
    std::ostringstream output;
    output << "[";
    for (std::size_t index = 0; index < faults.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        output << fault_to_json(faults[index]);
    }
    output << "]";
    return output.str();
}

const char* to_string(FaultClass fault_class) {
    switch (fault_class) {
        case FaultClass::WindGust:
            return "wind_gust";
        case FaultClass::GpsDrift:
            return "gps_drift";
        case FaultClass::ImuNoise:
            return "imu_noise";
        case FaultClass::SensorDropout:
            return "sensor_dropout";
        case FaultClass::CommLoss:
            return "comm_loss";
        case FaultClass::LowBattery:
            return "low_battery";
        case FaultClass::StaleTerrain:
            return "stale_terrain";
        case FaultClass::BadTile:
            return "bad_tile";
        case FaultClass::ActuatorLag:
            return "actuator_lag";
    }
    return "unknown";
}

FaultClass fault_class_from_string(std::string_view value) {
    if (value == "wind_gust") {
        return FaultClass::WindGust;
    }
    if (value == "gps_drift") {
        return FaultClass::GpsDrift;
    }
    if (value == "imu_noise") {
        return FaultClass::ImuNoise;
    }
    if (value == "sensor_dropout") {
        return FaultClass::SensorDropout;
    }
    if (value == "comm_loss") {
        return FaultClass::CommLoss;
    }
    if (value == "low_battery") {
        return FaultClass::LowBattery;
    }
    if (value == "stale_terrain") {
        return FaultClass::StaleTerrain;
    }
    if (value == "bad_tile") {
        return FaultClass::BadTile;
    }
    if (value == "actuator_lag") {
        return FaultClass::ActuatorLag;
    }
    throw std::runtime_error("unknown fault class: " + std::string(value));
}

FaultSpec parse_fault_spec(std::string_view text) {
    const auto parts = split_colon(text);
    if (parts.size() < 5 || parts.size() > 6) {
        throw std::runtime_error(
            "fault spec must be class:seed:start_step:end_step:magnitude[:target]");
    }

    FaultSpec spec;
    spec.fault_class = fault_class_from_string(parts[0]);
    if (parts[1] != "-") {
        spec.seed = parse_u64(parts[1], "seed");
    }
    spec.start_step = parse_u64(parts[2], "start_step");
    if (parts[3] != "-") {
        spec.end_step = parse_u64(parts[3], "end_step");
    }
    spec.magnitude = parse_double(parts[4], "magnitude");
    spec.target = parts.size() == 6 ? std::string(parts[5]) : default_target_for(spec.fault_class);
    return spec;
}

void validate_fault_plan(const FaultInjectionPlan& plan) {
    for (const auto& fault : plan.faults) {
        if (!fault.seed.has_value()) {
            throw std::invalid_argument("fault injection requires a seed for " + std::string(to_string(fault.fault_class)));
        }
        if (fault.end_step.has_value() && *fault.end_step < fault.start_step) {
            throw std::invalid_argument("fault injection end_step precedes start_step for " + std::string(to_string(fault.fault_class)));
        }
    }
}

bool is_fault_active(const FaultSpec& fault, std::uint64_t step) {
    if (step < fault.start_step) {
        return false;
    }
    return !fault.end_step.has_value() || step <= *fault.end_step;
}

bool sensor_stream_suppressed(const FaultInjectionPlan& plan, std::uint64_t step) {
    for (const auto& fault : plan.faults) {
        if ((fault.fault_class == FaultClass::SensorDropout || fault.fault_class == FaultClass::CommLoss)
            && is_fault_active(fault, step)) {
            return true;
        }
    }
    return false;
}

Vec3 wind_fault_for_step(const FaultInjectionPlan& plan, std::uint64_t step) {
    Vec3 wind;
    for (const auto& fault : plan.faults) {
        if (fault.fault_class != FaultClass::WindGust || !is_fault_active(fault, step) || !fault.seed.has_value()) {
            continue;
        }
        const double magnitude = default_magnitude(fault, 4.0);
        wind.x += symmetric_unit(*fault.seed, step, 0x1001U) * magnitude;
        wind.z += symmetric_unit(*fault.seed, step, 0x1002U) * magnitude;
    }
    return wind;
}

DroneState apply_observation_faults(
    const DroneState& state,
    const FaultInjectionPlan& plan,
    std::uint64_t step) {
    DroneState observed = state;
    for (const auto& fault : plan.faults) {
        if (!is_fault_active(fault, step) || !fault.seed.has_value()) {
            continue;
        }
        const std::uint64_t seed = *fault.seed;
        switch (fault.fault_class) {
            case FaultClass::GpsDrift: {
                const double magnitude = default_magnitude(fault, 1.0);
                observed.position.x += symmetric_unit(seed, step, 0x2001U) * magnitude;
                observed.position.y += symmetric_unit(seed, step, 0x2002U) * magnitude * 0.25;
                observed.position.z += symmetric_unit(seed, step, 0x2003U) * magnitude;
                break;
            }
            case FaultClass::ImuNoise: {
                const double magnitude = default_magnitude(fault, 0.01);
                observed.yaw_rad += symmetric_unit(seed, step, 0x3001U) * magnitude;
                observed.pitch_rad += symmetric_unit(seed, step, 0x3002U) * magnitude;
                observed.roll_rad += symmetric_unit(seed, step, 0x3003U) * magnitude;
                break;
            }
            case FaultClass::LowBattery: {
                const double drop = default_magnitude(fault, 20.0);
                observed.battery_percent = std::max(0.0, observed.battery_percent - drop);
                break;
            }
            case FaultClass::ActuatorLag: {
                const double lag_fraction = std::clamp(default_magnitude(fault, 0.25), 0.0, 0.95);
                observed.velocity = observed.velocity * (1.0 - lag_fraction);
                break;
            }
            case FaultClass::WindGust:
            case FaultClass::SensorDropout:
            case FaultClass::CommLoss:
            case FaultClass::StaleTerrain:
            case FaultClass::BadTile:
                break;
        }
    }
    return observed;
}

void append_fault_events_for_step(
    const FaultInjectionPlan& plan,
    std::uint64_t step,
    std::vector<FaultEvent>& events) {
    for (const auto& fault : plan.faults) {
        if (!fault.seed.has_value() || step != fault.start_step) {
            continue;
        }
        if (already_recorded(events, fault.fault_class, *fault.seed, step)) {
            continue;
        }
        events.push_back({
            fault.fault_class,
            *fault.seed,
            step,
            fault.target.empty() ? default_target_for(fault.fault_class) : fault.target,
            "activated",
        });
    }
}

std::string fault_events_to_json(const std::vector<FaultEvent>& events) {
    std::ostringstream output;
    output << "[";
    for (std::size_t index = 0; index < events.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        output << event_to_json(events[index]);
    }
    output << "]";
    return output.str();
}

std::string terrain_tiles_json_for_faults(const FaultInjectionPlan& plan) {
    std::ostringstream output;
    output << "[";
    bool wrote = false;
    for (const auto& fault : plan.faults) {
        if ((fault.fault_class != FaultClass::BadTile && fault.fault_class != FaultClass::StaleTerrain)
            || !fault.seed.has_value()) {
            continue;
        }
        if (wrote) {
            output << ",";
        }
        const char* state = fault.fault_class == FaultClass::BadTile ? "flat_fallback" : "stale";
        const std::string target = fault.target.empty() ? default_target_for(fault.fault_class) : fault.target;
        output << "{\"tile\":\"" << escape_json(target) << "\""
               << ",\"state\":\"" << state << "\""
               << ",\"fault_class\":\"" << to_string(fault.fault_class) << "\""
               << ",\"fault_seed\":" << *fault.seed
               << "}";
        wrote = true;
    }
    output << "]";
    return output.str();
}

} // namespace agbot::flight_sim
