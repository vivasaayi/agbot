#pragma once

#include "agbot_flight_sim/DroneSimulation.hpp"

#include <cstdint>
#include <string>
#include <string_view>

namespace agbot::flight_sim {

struct SensorCalibrationProfile {
    std::string name = "ideal";
    double gps_position_noise_m = 0.0;
    double imu_attitude_noise_rad = 0.0;
    double barometer_altitude_noise_m = 0.0;
    double magnetometer_heading_noise_rad = 0.0;
    double gps_position_bias_m = 0.0;
    double imu_yaw_bias_rad = 0.0;
    double barometer_altitude_bias_m = 0.0;
    double magnetometer_heading_bias_rad = 0.0;
};

struct SensorReading {
    std::string profile_name;
    std::uint64_t seed = 0;
    std::uint64_t step = 0;
    Vec3 gps_position_m;
    Vec3 velocity_mps;
    double imu_yaw_rad = 0.0;
    double imu_pitch_rad = 0.0;
    double imu_roll_rad = 0.0;
    double barometer_altitude_m = 0.0;
    double magnetometer_heading_rad = 0.0;

    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] SensorCalibrationProfile ideal_sensor_profile();
[[nodiscard]] SensorCalibrationProfile sensor_profile_by_name(std::string_view name);
[[nodiscard]] SensorReading calibrated_sensor_reading(
    const DroneState& state,
    const SensorCalibrationProfile& profile,
    std::uint64_t seed,
    std::uint64_t step);
[[nodiscard]] std::string sensor_config_json(const SensorCalibrationProfile& profile);

} // namespace agbot::flight_sim
