#include "agbot_flight_sim/SensorModel.hpp"

#include <cctype>
#include <filesystem>
#include <fstream>
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

std::string trim_copy(std::string_view value) {
    std::size_t start = 0;
    while (start < value.size() && std::isspace(static_cast<unsigned char>(value[start])) != 0) {
        ++start;
    }
    std::size_t end = value.size();
    while (end > start && std::isspace(static_cast<unsigned char>(value[end - 1])) != 0) {
        --end;
    }
    return std::string(value.substr(start, end - start));
}

std::string unquote(std::string value) {
    value = trim_copy(value);
    if (value.size() >= 2 && value.front() == '"' && value.back() == '"') {
        return value.substr(1, value.size() - 2);
    }
    return value;
}

std::string canonical_profile_name(std::string_view name) {
    const std::string normalized = trim_copy(name);
    if (normalized == "cheap_gps") {
        return "cheap_gps_b2";
    }
    if (normalized == "rtk_gps") {
        return "rtk_gps_a1";
    }
    return normalized;
}

std::filesystem::path calibration_profile_path(std::string_view name) {
    return std::filesystem::path(AGBOT_FLIGHT_SIM_SOURCE_DIR)
        / "calibration"
        / (std::string(name) + ".toml");
}

void apply_profile_value(
    SensorCalibrationProfile& profile,
    const std::string& key,
    const std::string& value) {
    if (key == "name") {
        profile.name = unquote(value);
    } else if (key == "version") {
        profile.version = unquote(value);
    } else if (key == "sensor_kind") {
        profile.sensor_kind = unquote(value);
    } else if (key == "gps_position_noise_m") {
        profile.gps_position_noise_m = std::stod(value);
    } else if (key == "imu_attitude_noise_rad") {
        profile.imu_attitude_noise_rad = std::stod(value);
    } else if (key == "barometer_altitude_noise_m") {
        profile.barometer_altitude_noise_m = std::stod(value);
    } else if (key == "magnetometer_heading_noise_rad") {
        profile.magnetometer_heading_noise_rad = std::stod(value);
    } else if (key == "gps_position_bias_m") {
        profile.gps_position_bias_m = std::stod(value);
    } else if (key == "imu_yaw_bias_rad") {
        profile.imu_yaw_bias_rad = std::stod(value);
    } else if (key == "barometer_altitude_bias_m") {
        profile.barometer_altitude_bias_m = std::stod(value);
    } else if (key == "magnetometer_heading_bias_rad") {
        profile.magnetometer_heading_bias_rad = std::stod(value);
    } else if (key == "lidar_range_noise_m") {
        profile.lidar_range_noise_m = std::stod(value);
    } else if (key == "lidar_range_bias_m") {
        profile.lidar_range_bias_m = std::stod(value);
    } else if (key == "multispectral_radiometric_noise") {
        profile.multispectral_radiometric_noise = std::stod(value);
    } else if (key == "multispectral_alignment_error_px") {
        profile.multispectral_alignment_error_px = std::stod(value);
    } else {
        throw std::runtime_error("unknown sensor profile key: " + key);
    }
}

SensorCalibrationProfile load_sensor_profile_from_file(std::string_view requested_name) {
    const std::string name = canonical_profile_name(requested_name);
    if (name.empty()) {
        throw std::runtime_error("unknown sensor profile: " + std::string(requested_name));
    }

    const std::filesystem::path path = calibration_profile_path(name);
    std::ifstream input(path);
    if (!input) {
        throw std::runtime_error("unknown sensor profile: " + std::string(requested_name));
    }

    SensorCalibrationProfile profile;
    profile.name = name;
    std::string line;
    while (std::getline(input, line)) {
        const std::size_t comment = line.find('#');
        if (comment != std::string::npos) {
            line.erase(comment);
        }
        line = trim_copy(line);
        if (line.empty()) {
            continue;
        }
        const std::size_t equals = line.find('=');
        if (equals == std::string::npos) {
            throw std::runtime_error("malformed sensor profile line in " + path.string());
        }
        const std::string key = trim_copy(std::string_view(line).substr(0, equals));
        const std::string value = trim_copy(std::string_view(line).substr(equals + 1));
        apply_profile_value(profile, key, value);
    }

    if (profile.name != name) {
        throw std::runtime_error(
            "sensor profile file name " + name + " does not match profile name " + profile.name);
    }
    return profile;
}

} // namespace

std::string SensorReading::to_json() const {
    std::ostringstream output;
    output << std::fixed << std::setprecision(6)
           << "{\"profile\":\"" << escape_json(profile_name) << "\""
           << ",\"profile_version\":\"" << escape_json(profile_version) << "\""
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
    return load_sensor_profile_from_file("ideal");
}

SensorCalibrationProfile sensor_profile_by_name(std::string_view name) {
    return load_sensor_profile_from_file(name);
}

SensorReading calibrated_sensor_reading(
    const DroneState& state,
    const SensorCalibrationProfile& profile,
    std::uint64_t seed,
    std::uint64_t step) {
    SensorReading reading;
    reading.profile_name = profile.name;
    reading.profile_version = profile.version;
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
           << ",\"version\":\"" << escape_json(profile.version) << "\""
           << ",\"sensor_kind\":\"" << escape_json(profile.sensor_kind) << "\""
           << ",\"calibration_file\":\"calibration/" << escape_json(profile.name) << ".toml\""
           << ",\"distribution\":\"deterministic_uniform\""
           << ",\"gps_position_noise_m\":" << profile.gps_position_noise_m
           << ",\"imu_attitude_noise_rad\":" << profile.imu_attitude_noise_rad
           << ",\"barometer_altitude_noise_m\":" << profile.barometer_altitude_noise_m
           << ",\"magnetometer_heading_noise_rad\":" << profile.magnetometer_heading_noise_rad
           << ",\"gps_position_bias_m\":" << profile.gps_position_bias_m
           << ",\"imu_yaw_bias_rad\":" << profile.imu_yaw_bias_rad
           << ",\"barometer_altitude_bias_m\":" << profile.barometer_altitude_bias_m
           << ",\"magnetometer_heading_bias_rad\":" << profile.magnetometer_heading_bias_rad
           << ",\"lidar_range_noise_m\":" << profile.lidar_range_noise_m
           << ",\"lidar_range_bias_m\":" << profile.lidar_range_bias_m
           << ",\"multispectral_radiometric_noise\":" << profile.multispectral_radiometric_noise
           << ",\"multispectral_alignment_error_px\":" << profile.multispectral_alignment_error_px
           << "}";
    return output.str();
}

} // namespace agbot::flight_sim
