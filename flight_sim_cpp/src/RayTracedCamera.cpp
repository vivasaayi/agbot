#include "agbot_flight_sim/RayTracedCamera.hpp"

#include "agbot_flight_sim/TwinContractV1.hpp"

#include <algorithm>
#include <cmath>
#include <iomanip>
#include <sstream>
#include <stdexcept>
#include <string_view>

namespace agbot::flight_sim {
namespace {

constexpr double kPi = 3.14159265358979323846;

struct LocalBounds {
    double min_x = 0.0;
    double max_x = 0.0;
    double min_z = 0.0;
    double max_z = 0.0;
};

struct ObjectHit {
    const SceneObject* object = nullptr;
    double height_m = 0.0;
};

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

void write_double(std::ostringstream& output, double value, int precision = 6) {
    output << std::fixed << std::setprecision(precision) << value;
}

void write_vec3_json(std::ostringstream& output, const Vec3& value) {
    output << "{\"x\":";
    write_double(output, value.x, 3);
    output << ",\"y\":";
    write_double(output, value.y, 3);
    output << ",\"z\":";
    write_double(output, value.z, 3);
    output << "}";
}

double deg_to_rad(double degrees) {
    return degrees * kPi / 180.0;
}

bool valid_config(const RayTracedCameraConfig& config) {
    return config.width > 0
        && config.height > 0
        && std::isfinite(config.horizontal_fov_deg)
        && std::isfinite(config.vertical_fov_deg)
        && config.horizontal_fov_deg > 0.0
        && config.vertical_fov_deg > 0.0
        && config.horizontal_fov_deg < 179.0
        && config.vertical_fov_deg < 179.0
        && std::isfinite(config.max_range_m)
        && config.max_range_m > 0.0;
}

LocalBounds local_bounds_for_profile(const TerrainProfile& profile) {
    const GeoCoordinate origin = profile.bounds.center();
    const Vec3 north_west =
        local_from_geo({profile.bounds.max_latitude, profile.bounds.min_longitude, 0.0}, origin);
    const Vec3 south_east =
        local_from_geo({profile.bounds.min_latitude, profile.bounds.max_longitude, 0.0}, origin);
    return {
        std::min(north_west.x, south_east.x),
        std::max(north_west.x, south_east.x),
        std::min(north_west.z, south_east.z),
        std::max(north_west.z, south_east.z),
    };
}

bool contains_local(const LocalBounds& bounds, Vec3 position) {
    return position.x >= bounds.min_x
        && position.x <= bounds.max_x
        && position.z >= bounds.min_z
        && position.z <= bounds.max_z;
}

bool point_in_polygon(double x, double z, const std::vector<Vec3>& polygon) {
    if (polygon.size() < 3) {
        return false;
    }
    bool inside = false;
    for (std::size_t i = 0, j = polygon.size() - 1; i < polygon.size(); j = i++) {
        const Vec3& pi = polygon[i];
        const Vec3& pj = polygon[j];
        const bool crosses = ((pi.z > z) != (pj.z > z))
            && (x < (pj.x - pi.x) * (z - pi.z) / ((pj.z - pi.z) + 1e-12) + pi.x);
        if (crosses) {
            inside = !inside;
        }
    }
    return inside;
}

ObjectHit hit_object_at(const SceneSynthesisManifest& scene, Vec3 ground_position) {
    ObjectHit hit;
    for (const SceneObject& object : scene.objects) {
        if (!point_in_polygon(ground_position.x, ground_position.z, object.footprint_local_m)) {
            continue;
        }
        if (object.height_m >= hit.height_m) {
            hit.object = &object;
            hit.height_m = object.height_m;
        }
    }
    return hit;
}

std::string frame_json_without_hash(const RayTracedFrame& frame) {
    std::ostringstream output;
    output << "{"
           << "\"status\":\"" << escape_json(frame.status) << "\""
           << ",\"reason\":\"" << escape_json(frame.reason) << "\""
           << ",\"width\":" << frame.width
           << ",\"height\":" << frame.height
           << ",\"timestamp_s\":";
    write_double(output, frame.timestamp_s, 3);
    output << ",\"pose\":";
    write_vec3_json(output, frame.pose);
    output << ",\"scene_hash\":\"" << escape_json(frame.scene_hash) << "\""
           << ",\"pixels\":[";
    for (std::size_t index = 0; index < frame.pixels.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        const RayTracedPixel& pixel = frame.pixels[index];
        output << "{\"x\":" << pixel.x
               << ",\"y\":" << pixel.y
               << ",\"depth_m\":";
        write_double(output, pixel.depth_m, 3);
        output << ",\"world_position_m\":";
        write_vec3_json(output, pixel.world_position_m);
        output << ",\"class_name\":\"" << escape_json(pixel.class_name) << "\""
               << ",\"object_id\":\"" << escape_json(pixel.object_id) << "\""
               << ",\"object_seed\":" << pixel.object_seed
               << "}";
    }
    output << "]}";
    return output.str();
}

RayTracedFrame empty_frame(
    const DroneState& state,
    const SceneSynthesisManifest& scene,
    const RayTracedCameraConfig& config,
    std::string status,
    std::string reason) {
    RayTracedFrame frame;
    frame.status = std::move(status);
    frame.reason = std::move(reason);
    frame.width = std::max(0, config.width);
    frame.height = std::max(0, config.height);
    frame.timestamp_s = state.mission_time_s;
    frame.pose = state.position;
    frame.scene_hash = scene.scene_hash;
    frame.frame_hash = sha256_hex(frame_json_without_hash(frame));
    return frame;
}

} // namespace

const RayTracedPixel& RayTracedFrame::pixel_at(int x, int y) const {
    if (x < 0 || y < 0 || x >= width || y >= height) {
        throw std::out_of_range("ray-traced pixel coordinate out of range");
    }
    const std::size_t index = static_cast<std::size_t>(y * width + x);
    if (index >= pixels.size()) {
        throw std::out_of_range("ray-traced pixel missing from frame");
    }
    return pixels[index];
}

std::string RayTracedFrame::to_json() const {
    std::string json = frame_json_without_hash(*this);
    json.pop_back();
    json += ",\"frame_hash\":\"" + escape_json(frame_hash) + "\"}";
    return json;
}

RayTracedFrame raytrace_camera_frame(
    const DroneState& state,
    const TerrainProfile& profile,
    const SceneSynthesisManifest& scene,
    RayTracedCameraConfig config) {
    if (!valid_config(config)) {
        return empty_frame(state, scene, config, "invalid_camera_config", "camera intrinsics are invalid");
    }
    if (!profile.asserted || profile.crs.empty()) {
        return empty_frame(state, scene, config, "no_scene_coverage", "terrain profile is not asserted");
    }

    const LocalBounds local_bounds = local_bounds_for_profile(profile);
    if (!contains_local(local_bounds, state.position)) {
        return empty_frame(state, scene, config, "no_scene_coverage", "camera pose outside scene extent");
    }

    RayTracedFrame frame;
    frame.status = "ok";
    frame.width = config.width;
    frame.height = config.height;
    frame.timestamp_s = state.mission_time_s;
    frame.pose = state.position;
    frame.scene_hash = scene.scene_hash;
    frame.pixels.reserve(static_cast<std::size_t>(config.width * config.height));

    const double altitude_m = std::max(0.1, state.position.y);
    const double half_width_m = std::tan(deg_to_rad(config.horizontal_fov_deg) * 0.5) * altitude_m;
    const double half_height_m = std::tan(deg_to_rad(config.vertical_fov_deg) * 0.5) * altitude_m;

    for (int y = 0; y < config.height; ++y) {
        for (int x = 0; x < config.width; ++x) {
            const double u = ((static_cast<double>(x) + 0.5) / static_cast<double>(config.width) - 0.5) * 2.0;
            const double v = ((static_cast<double>(y) + 0.5) / static_cast<double>(config.height) - 0.5) * 2.0;
            const Vec3 ground {
                state.position.x + u * half_width_m,
                0.0,
                state.position.z + v * half_height_m,
            };

            RayTracedPixel pixel;
            pixel.x = x;
            pixel.y = y;

            if (!contains_local(local_bounds, ground)) {
                pixel.class_name = "no_coverage";
                pixel.world_position_m = ground;
                frame.pixels.push_back(pixel);
                continue;
            }

            const ObjectHit hit = hit_object_at(scene, ground);
            const double hit_y = hit.object == nullptr ? 0.0 : hit.height_m;
            pixel.world_position_m = {ground.x, hit_y, ground.z};
            pixel.depth_m = (state.position - pixel.world_position_m).length();
            if (pixel.depth_m > config.max_range_m) {
                pixel.class_name = "range_exceeded";
            } else if (hit.object != nullptr) {
                pixel.class_name = hit.object->class_name;
                pixel.object_id = hit.object->object_id;
                pixel.object_seed = hit.object->placement_seed;
            } else {
                pixel.class_name = "terrain";
            }
            frame.pixels.push_back(pixel);
        }
    }

    frame.frame_hash = sha256_hex(frame_json_without_hash(frame));
    return frame;
}

} // namespace agbot::flight_sim
