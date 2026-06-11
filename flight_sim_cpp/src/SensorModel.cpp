#include "agbot_flight_sim/SensorModel.hpp"

#include <iomanip>
#include <sstream>
#include <stdexcept>

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

} // namespace

std::string SensorReading::to_json() const {
    std::ostringstream output;
    output << std::fixed << std::setprecision(6)
           << "{\"profile\":\"" << escape_json(profile_name) << "\""
           << ",\"distribution\":\"deterministic_uniform\""
           << ",\"seed\":" << seed
           << ",\"step\":" << step
           << ",\"gps_position_m\":{\"x\":" << gps_position_m.x
           << ",\"y\":" << gps_position_m.y
           << ",\"z\":" << gps_position_m.z << "}"
           << ",\"velocity_mps\":{\"x\":" << velocity_mps.x
           << ",\"y\":" << velocity_mps.y
           << ",\"z\":" << velocity_mps.z << "}"
           << ",\"imu\":{\"yaw_rad\":" << imu_yaw_rad
           << ",\"pitch_rad\":" << imu_pitch_rad
           << ",\"roll_rad\":" << imu_roll_rad << "}"
           << ",\"barometer_altitude_m\":" << barometer_altitude_m
           << ",\"magnetometer_heading_rad\":" << magnetometer_heading_rad
           << "}";
    return output.str();
}

SensorCalibrationProfile ideal_sensor_profile() {
    return {};
}

SensorCalibrationProfile sensor_profile_by_name(std::string_view name) {
    if (name == "ideal") {
        return ideal_sensor_profile();
    }
    if (name == "cheap_gps") {
        SensorCalibrationProfile profile;
        profile.name = "cheap_gps";
        profile.gps_position_noise_m = 1.5;
        profile.barometer_altitude_noise_m = 0.2;
        profile.magnetometer_heading_noise_rad = 0.01;
        return profile;
    }
    if (name == "rtk_gps") {
        SensorCalibrationProfile profile;
        profile.name = "rtk_gps";
        profile.gps_position_noise_m = 0.03;
        profile.barometer_altitude_noise_m = 0.05;
        profile.magnetometer_heading_noise_rad = 0.005;
        return profile;
    }
    if (name == "noisy_imu") {
        SensorCalibrationProfile profile;
        profile.name = "noisy_imu";
        profile.gps_position_noise_m = 0.5;
        profile.imu_attitude_noise_rad = 0.04;
        profile.barometer_altitude_noise_m = 0.3;
        profile.magnetometer_heading_noise_rad = 0.04;
        return profile;
    }

    throw std::runtime_error("unknown sensor profile: " + std::string(name));
}

SensorReading calibrated_sensor_reading(
    const DroneState& state,
    const SensorCalibrationProfile& profile,
    std::uint64_t seed,
    std::uint64_t step) {
    SensorReading reading;
    reading.profile_name = profile.name;
    reading.seed = seed;
    reading.step = step;
    reading.gps_position_m = {
        state.position.x + profile.gps_position_bias_m
            + symmetric_unit(seed, step, 0x4101U) * profile.gps_position_noise_m,
        state.position.y + profile.gps_position_bias_m
            + symmetric_unit(seed, step, 0x4102U) * profile.gps_position_noise_m,
        state.position.z + profile.gps_position_bias_m
            + symmetric_unit(seed, step, 0x4103U) * profile.gps_position_noise_m,
    };
    reading.velocity_mps = state.velocity;
    reading.imu_yaw_rad = state.yaw_rad + profile.imu_yaw_bias_rad
        + symmetric_unit(seed, step, 0x4201U) * profile.imu_attitude_noise_rad;
    reading.imu_pitch_rad = state.pitch_rad
        + symmetric_unit(seed, step, 0x4202U) * profile.imu_attitude_noise_rad;
    reading.imu_roll_rad = state.roll_rad
        + symmetric_unit(seed, step, 0x4203U) * profile.imu_attitude_noise_rad;
    reading.barometer_altitude_m = state.position.y + profile.barometer_altitude_bias_m
        + symmetric_unit(seed, step, 0x4301U) * profile.barometer_altitude_noise_m;
    reading.magnetometer_heading_rad = state.yaw_rad + profile.magnetometer_heading_bias_rad
        + symmetric_unit(seed, step, 0x4401U) * profile.magnetometer_heading_noise_rad;
    return reading;
}

std::string sensor_config_json(const SensorCalibrationProfile& profile) {
    std::ostringstream output;
    output << std::fixed << std::setprecision(3)
           << "{\"profile\":\"" << escape_json(profile.name) << "\""
           << ",\"distribution\":\"deterministic_uniform\""
           << ",\"gps_position_noise_m\":" << profile.gps_position_noise_m
           << ",\"imu_attitude_noise_rad\":" << profile.imu_attitude_noise_rad
           << ",\"barometer_altitude_noise_m\":" << profile.barometer_altitude_noise_m
           << ",\"magnetometer_heading_noise_rad\":" << profile.magnetometer_heading_noise_rad
           << ",\"gps_position_bias_m\":" << profile.gps_position_bias_m
           << ",\"imu_yaw_bias_rad\":" << profile.imu_yaw_bias_rad
           << ",\"barometer_altitude_bias_m\":" << profile.barometer_altitude_bias_m
           << ",\"magnetometer_heading_bias_rad\":" << profile.magnetometer_heading_bias_rad
           << "}";
    return output.str();
}

} // namespace agbot::flight_sim
