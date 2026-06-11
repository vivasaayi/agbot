#include "agbot_flight_sim/LidarSimulator.hpp"

#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <limits>
#include <optional>
#include <sstream>
#include <stdexcept>
#include <string_view>

namespace agbot::flight_sim {
namespace {

constexpr double kPi = 3.14159265358979323846;

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

double deg_to_rad(double degrees) {
    return degrees * kPi / 180.0;
}

double rad_to_deg(double radians) {
    return radians * 180.0 / kPi;
}

double normalize_degrees(double degrees) {
    double normalized = std::fmod(degrees, 360.0);
    if (normalized < 0.0) {
        normalized += 360.0;
    }
    return normalized;
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

std::string deterministic_timestamp(double mission_time_s) {
    const auto total_ms = static_cast<long long>(std::llround(std::max(0.0, mission_time_s) * 1000.0));
    const long long total_seconds = total_ms / 1000;
    const long long milliseconds = total_ms % 1000;
    const long long hours = (total_seconds / 3600) % 24;
    const long long minutes = (total_seconds / 60) % 60;
    const long long seconds = total_seconds % 60;

    std::ostringstream output;
    output << "1970-01-01T"
           << std::setw(2) << std::setfill('0') << hours << ":"
           << std::setw(2) << std::setfill('0') << minutes << ":"
           << std::setw(2) << std::setfill('0') << seconds << "."
           << std::setw(3) << std::setfill('0') << milliseconds << "Z";
    return output.str();
}

std::string deterministic_uuid(std::uint64_t seed, std::uint64_t step) {
    std::array<std::uint8_t, 16> bytes {};
    std::uint64_t left = mix64(seed ^ (step * 0x517cc1b727220a95ULL));
    std::uint64_t right = mix64(left ^ 0x94d049bb133111ebULL);
    for (int index = 0; index < 8; ++index) {
        bytes[static_cast<std::size_t>(index)] =
            static_cast<std::uint8_t>((left >> ((7 - index) * 8)) & 0xffU);
        bytes[static_cast<std::size_t>(index + 8)] =
            static_cast<std::uint8_t>((right >> ((7 - index) * 8)) & 0xffU);
    }
    bytes[6] = static_cast<std::uint8_t>((bytes[6] & 0x0fU) | 0x40U);
    bytes[8] = static_cast<std::uint8_t>((bytes[8] & 0x3fU) | 0x80U);

    std::ostringstream output;
    output << std::hex << std::setfill('0');
    for (std::size_t index = 0; index < bytes.size(); ++index) {
        if (index == 4 || index == 6 || index == 8 || index == 10) {
            output << "-";
        }
        output << std::setw(2) << static_cast<unsigned int>(bytes[index]);
    }
    return output.str();
}

std::optional<int> terrain_resolution(const TerrainMesh& terrain) {
    const double root = std::sqrt(static_cast<double>(terrain.vertices.size()));
    const int resolution = static_cast<int>(std::llround(root));
    if (resolution < 2 || static_cast<std::size_t>(resolution * resolution) != terrain.vertices.size()) {
        return std::nullopt;
    }
    return resolution;
}

std::optional<double> terrain_height_at(const TerrainMesh& terrain, double x, double z) {
    const auto resolution = terrain_resolution(terrain);
    if (!resolution.has_value()) {
        return std::nullopt;
    }

    const Vec3& first = terrain.vertices.front().position;
    const Vec3& last = terrain.vertices.back().position;
    const double min_x = std::min(first.x, last.x);
    const double max_x = std::max(first.x, last.x);
    const double min_z = std::min(first.z, last.z);
    const double max_z = std::max(first.z, last.z);
    if (max_x <= min_x || max_z <= min_z || x < min_x || x > max_x || z < min_z || z > max_z) {
        return std::nullopt;
    }

    const double u = (x - min_x) / (max_x - min_x);
    const double v = (z - min_z) / (max_z - min_z);
    const double fx = u * static_cast<double>(*resolution - 1);
    const double fz = v * static_cast<double>(*resolution - 1);
    const int x0 = static_cast<int>(std::floor(fx));
    const int z0 = static_cast<int>(std::floor(fz));
    const int x1 = std::min(x0 + 1, *resolution - 1);
    const int z1 = std::min(z0 + 1, *resolution - 1);
    const double tx = fx - static_cast<double>(x0);
    const double tz = fz - static_cast<double>(z0);

    const auto at = [&terrain, resolution](int grid_x, int grid_z) {
        return terrain.vertices[static_cast<std::size_t>(grid_z * *resolution + grid_x)].position.y;
    };

    const double top = at(x0, z0) * (1.0 - tx) + at(x1, z0) * tx;
    const double bottom = at(x0, z1) * (1.0 - tx) + at(x1, z1) * tx;
    return top * (1.0 - tz) + bottom * tz;
}

struct RayHit {
    Vec3 position;
    double range_m = 0.0;
};

std::optional<RayHit> raycast_heightfield(
    const Vec3& origin,
    const Vec3& direction,
    const TerrainMesh& terrain,
    double max_range_m) {
    if (max_range_m <= 0.0 || direction.length() <= 1e-9) {
        return std::nullopt;
    }

    constexpr int kSteps = 1024;
    std::optional<double> previous_t;
    std::optional<double> previous_clearance;

    for (int step = 0; step <= kSteps; ++step) {
        const double t = max_range_m * static_cast<double>(step) / static_cast<double>(kSteps);
        const Vec3 position = origin + direction * t;
        const auto terrain_y = terrain_height_at(terrain, position.x, position.z);
        if (!terrain_y.has_value()) {
            continue;
        }

        const double clearance = position.y - *terrain_y;
        if (previous_t.has_value() && previous_clearance.has_value() && *previous_clearance > 0.0 && clearance <= 0.0) {
            double low = *previous_t;
            double high = t;
            for (int iteration = 0; iteration < 24; ++iteration) {
                const double mid = (low + high) * 0.5;
                const Vec3 mid_position = origin + direction * mid;
                const auto mid_terrain_y = terrain_height_at(terrain, mid_position.x, mid_position.z);
                if (!mid_terrain_y.has_value()) {
                    high = mid;
                    continue;
                }
                if (mid_position.y - *mid_terrain_y > 0.0) {
                    low = mid;
                } else {
                    high = mid;
                }
            }

            Vec3 hit = origin + direction * high;
            if (const auto hit_y = terrain_height_at(terrain, hit.x, hit.z)) {
                hit.y = *hit_y;
            }
            return RayHit {hit, high};
        }

        previous_t = t;
        previous_clearance = clearance;
    }

    return std::nullopt;
}

std::uint8_t quality_for_range(double range_m, const LidarRaycastConfig& config) {
    if (config.max_range_m <= 0.0) {
        return 0;
    }
    const double falloff = std::clamp(1.0 - (range_m / config.max_range_m) * 0.5, 0.0, 1.0);
    const double quality = static_cast<double>(config.min_quality)
        + (static_cast<double>(config.max_quality) - static_cast<double>(config.min_quality)) * falloff;
    return static_cast<std::uint8_t>(std::clamp(std::llround(quality), 0LL, 255LL));
}

} // namespace

std::string LidarPoint::to_json() const {
    std::ostringstream output;
    output << std::fixed << std::setprecision(3)
           << "{\"timestamp\":\"" << escape_json(timestamp) << "\""
           << ",\"angle\":" << angle_deg
           << ",\"distance\":" << distance_mm
           << ",\"quality\":" << static_cast<unsigned int>(quality)
           << ",\"range_m\":" << range_m
           << ",\"x\":" << position_m.x
           << ",\"y\":" << position_m.y
           << ",\"z\":" << position_m.z
           << ",\"direction\":{\"x\":" << direction.x
           << ",\"y\":" << direction.y
           << ",\"z\":" << direction.z << "}"
           << ",\"ring\":" << ring
           << ",\"azimuth_index\":" << azimuth_index
           << "}";
    return output.str();
}

std::string LidarScan::to_json() const {
    std::ostringstream output;
    output << std::fixed << std::setprecision(3)
           << "{\"timestamp\":\"" << escape_json(timestamp) << "\""
           << ",\"points\":[";
    for (std::size_t index = 0; index < points.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        output << points[index].to_json();
    }
    output << "]"
           << ",\"scan_id\":\"" << escape_json(scan_id) << "\""
           << ",\"sensor_id\":\"" << escape_json(sensor_id) << "\""
           << ",\"frame_id\":\"" << escape_json(frame_id) << "\""
           << ",\"status\":\"" << escape_json(status) << "\""
           << ",\"seed\":" << seed
           << ",\"step\":" << step
           << ",\"sensor_position_m\":{\"x\":" << sensor_position_m.x
           << ",\"y\":" << sensor_position_m.y
           << ",\"z\":" << sensor_position_m.z << "}"
           << ",\"point_count\":" << points.size()
           << "}";
    return output.str();
}

LidarScan raycast_lidar_scan(
    const DroneState& state,
    const TerrainMesh& terrain,
    const LidarRaycastConfig& config,
    std::uint64_t seed,
    std::uint64_t step) {
    LidarScan scan;
    scan.timestamp = deterministic_timestamp(state.mission_time_s);
    scan.scan_id = deterministic_uuid(seed, step);
    scan.sensor_id = config.sensor_id;
    scan.seed = seed;
    scan.step = step;
    scan.sensor_position_m = state.position;

    if (!config.enabled) {
        scan.status = "disabled";
        return scan;
    }
    if (!terrain_resolution(terrain).has_value()) {
        scan.status = "empty_scene";
        return scan;
    }
    if (config.horizontal_samples == 0 || config.vertical_samples == 0 || config.max_range_m <= 0.0) {
        scan.status = "invalid_config";
        return scan;
    }

    const double max_off_nadir_rad = deg_to_rad(std::max(0.0, config.vertical_fov_deg) * 0.5);
    std::uint64_t point_index = 0;
    for (std::uint32_t ring = 0; ring < config.vertical_samples; ++ring) {
        const bool nadir_ring = ring == 0;
        const std::uint32_t azimuth_count = nadir_ring ? 1U : config.horizontal_samples;
        const double off_nadir_rad = config.vertical_samples == 1
            ? 0.0
            : max_off_nadir_rad * static_cast<double>(ring) / static_cast<double>(config.vertical_samples - 1);

        for (std::uint32_t azimuth_index = 0; azimuth_index < azimuth_count; ++azimuth_index) {
            const double azimuth_rad = state.yaw_rad
                + (nadir_ring ? 0.0 : (2.0 * kPi * static_cast<double>(azimuth_index) /
                    static_cast<double>(config.horizontal_samples)));
            const Vec3 direction {
                std::sin(off_nadir_rad) * std::cos(azimuth_rad),
                -std::cos(off_nadir_rad),
                std::sin(off_nadir_rad) * std::sin(azimuth_rad),
            };

            const auto hit = raycast_heightfield(state.position, direction.normalized(), terrain, config.max_range_m);
            if (!hit.has_value()) {
                ++point_index;
                continue;
            }

            const double noise = config.range_noise_m == 0.0
                ? 0.0
                : symmetric_unit(seed, step + point_index, 0x6c1da501U) * config.range_noise_m;
            const double range_m = std::clamp(hit->range_m + noise, 0.0, config.max_range_m);
            const Vec3 noisy_position = state.position + direction.normalized() * range_m;

            LidarPoint point;
            point.timestamp = scan.timestamp;
            point.angle_deg = normalize_degrees(rad_to_deg(azimuth_rad - state.yaw_rad));
            point.distance_mm = range_m * 1000.0;
            point.quality = quality_for_range(range_m, config);
            point.range_m = range_m;
            point.position_m = noisy_position;
            point.direction = direction.normalized();
            point.ring = ring;
            point.azimuth_index = azimuth_index;
            scan.points.push_back(point);
            ++point_index;
        }
    }

    scan.status = scan.points.empty() ? "no_hits" : "ok";
    return scan;
}

TerrainMesh build_lidar_flat_terrain_for_mission(
    const Mission& mission,
    int resolution,
    double padding_m) {
    if (resolution < 2) {
        throw std::invalid_argument("LiDAR terrain resolution must be at least 2");
    }

    double min_x = mission.home.x;
    double max_x = mission.home.x;
    double min_z = mission.home.z;
    double max_z = mission.home.z;
    for (const Waypoint& waypoint : mission.waypoints) {
        min_x = std::min(min_x, waypoint.position.x);
        max_x = std::max(max_x, waypoint.position.x);
        min_z = std::min(min_z, waypoint.position.z);
        max_z = std::max(max_z, waypoint.position.z);
    }

    const double width_m = std::max(10.0, (max_x - min_x) + padding_m * 2.0);
    const double depth_m = std::max(10.0, (max_z - min_z) + padding_m * 2.0);
    const double center_x = (min_x + max_x) * 0.5;
    const double center_z = (min_z + max_z) * 0.5;
    std::vector<float> heightmap(static_cast<std::size_t>(resolution * resolution), 0.0f);
    TerrainMesh mesh = build_terrain_mesh(heightmap, resolution, width_m, depth_m);
    for (TerrainVertex& vertex : mesh.vertices) {
        vertex.position.x += center_x;
        vertex.position.z += center_z;
    }
    return mesh;
}

std::string lidar_config_json(const LidarRaycastConfig& config) {
    std::ostringstream output;
    output << std::fixed << std::setprecision(3)
           << "{\"enabled\":" << (config.enabled ? "true" : "false")
           << ",\"profile\":\"" << escape_json(config.profile_name) << "\""
           << ",\"sensor_id\":\"" << escape_json(config.sensor_id) << "\""
           << ",\"horizontal_samples\":" << config.horizontal_samples
           << ",\"vertical_samples\":" << config.vertical_samples
           << ",\"vertical_fov_deg\":" << config.vertical_fov_deg
           << ",\"max_range_m\":" << config.max_range_m
           << ",\"range_noise_m\":" << config.range_noise_m
           << ",\"min_quality\":" << static_cast<unsigned int>(config.min_quality)
           << ",\"max_quality\":" << static_cast<unsigned int>(config.max_quality)
           << "}";
    return output.str();
}

} // namespace agbot::flight_sim
