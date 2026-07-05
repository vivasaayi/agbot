#include "agbot_nav/Sensing.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <random>

namespace agbot::nav {

namespace {

constexpr double kPi = 3.14159265358979323846;
constexpr double kEps = 1e-12;
constexpr double kSurfaceBiasM = 1e-3;

double deg_to_rad(double degrees) {
    return degrees * kPi / 180.0;
}

Vec3 cross(const Vec3& a, const Vec3& b) {
    return {
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    };
}

bool point_in_polygon_2d(double x, double z, const std::vector<Vec3>& polygon) {
    if (polygon.size() < 3) {
        return false;
    }
    bool inside = false;
    for (std::size_t i = 0, j = polygon.size() - 1; i < polygon.size(); j = i++) {
        const Vec3& pi = polygon[i];
        const Vec3& pj = polygon[j];
        const bool crosses = ((pi.z > z) != (pj.z > z))
            && (x < (pj.x - pi.x) * (z - pi.z) / ((pj.z - pi.z) + kEps) + pi.x);
        if (crosses) {
            inside = !inside;
        }
    }
    return inside;
}

// Nearest ray parameter t >= 0 where the ray enters the prism formed by
// extruding `footprint` from y0 to y1, or a negative value when there is no
// hit within max_t.
double ray_prism_entry(
    const Vec3& origin,
    const Vec3& direction,
    const std::vector<Vec3>& footprint,
    double y0,
    double y1,
    double max_t) {
    if (footprint.size() < 3) {
        return -1.0;
    }

    // Vertical slab interval [ty0, ty1] where origin + t*d has y in [y0, y1].
    double ty0 = 0.0;
    double ty1 = max_t;
    if (std::abs(direction.y) > kEps) {
        double ta = (y0 - origin.y) / direction.y;
        double tb = (y1 - origin.y) / direction.y;
        if (ta > tb) {
            std::swap(ta, tb);
        }
        ty0 = std::max(0.0, ta);
        ty1 = std::min(max_t, tb);
    } else if (origin.y < y0 || origin.y > y1) {
        return -1.0;
    }
    if (ty0 > ty1) {
        return -1.0;
    }

    // Crossing parameters of the 2D ray against the footprint boundary.
    std::vector<double> crossings;
    crossings.reserve(footprint.size());
    for (std::size_t i = 0, j = footprint.size() - 1; i < footprint.size(); j = i++) {
        const Vec3& a = footprint[j];
        const Vec3& b = footprint[i];
        const double ex = b.x - a.x;
        const double ez = b.z - a.z;
        const double denom = direction.x * ez - direction.z * ex;
        if (std::abs(denom) <= kEps) {
            continue; // parallel
        }
        const double t = ((a.x - origin.x) * ez - (a.z - origin.z) * ex) / denom;
        const double s = std::abs(ex) > std::abs(ez)
            ? (origin.x + direction.x * t - a.x) / ex
            : (origin.z + direction.z * t - a.z) / ez;
        if (t >= 0.0 && t <= max_t && s >= 0.0 && s < 1.0) {
            crossings.push_back(t);
        }
    }
    std::sort(crossings.begin(), crossings.end());

    const bool origin_inside = point_in_polygon_2d(origin.x, origin.z, footprint);

    // Walk the alternating inside/outside intervals of the 2D ray and
    // intersect each inside interval with the vertical slab.
    double interval_start = 0.0;
    bool inside = origin_inside;
    double best = -1.0;
    auto consider = [&](double lo, double hi) {
        const double clipped_lo = std::max(lo, ty0);
        const double clipped_hi = std::min(hi, ty1);
        if (clipped_lo <= clipped_hi && (best < 0.0 || clipped_lo < best)) {
            best = clipped_lo;
        }
    };
    for (const double t : crossings) {
        if (inside) {
            consider(interval_start, t);
        }
        interval_start = t;
        inside = !inside;
    }
    if (inside) {
        consider(interval_start, max_t);
    }
    return best;
}

// Nearest ray parameter t >= 0 where the ray enters the vertical cylinder of
// radius `radius` around axis (cx, cz), spanning y in [y0, y1], or a negative
// value when there is no hit within max_t. Cap entries fall out of the
// slab/radial interval intersection naturally.
double ray_cylinder_entry(
    const Vec3& origin,
    const Vec3& direction,
    double cx,
    double cz,
    double radius,
    double y0,
    double y1,
    double max_t) {
    if (radius <= 0.0 || y1 <= y0) {
        return -1.0;
    }

    // Vertical slab interval [ty0, ty1] where origin + t*d has y in [y0, y1].
    double ty0 = 0.0;
    double ty1 = max_t;
    if (std::abs(direction.y) > kEps) {
        double ta = (y0 - origin.y) / direction.y;
        double tb = (y1 - origin.y) / direction.y;
        if (ta > tb) {
            std::swap(ta, tb);
        }
        ty0 = std::max(0.0, ta);
        ty1 = std::min(max_t, tb);
    } else if (origin.y < y0 || origin.y > y1) {
        return -1.0;
    }
    if (ty0 > ty1) {
        return -1.0;
    }

    const double ox = origin.x - cx;
    const double oz = origin.z - cz;
    const double a = direction.x * direction.x + direction.z * direction.z;
    if (a <= kEps) {
        // Vertical ray: hits only when horizontally inside the disk.
        return (ox * ox + oz * oz <= radius * radius) ? ty0 : -1.0;
    }
    const double b = 2.0 * (ox * direction.x + oz * direction.z);
    const double c = ox * ox + oz * oz - radius * radius;
    const double disc = b * b - 4.0 * a * c;
    if (disc < 0.0) {
        return -1.0;
    }
    const double sq = std::sqrt(disc);
    const double t_radial_lo = (-b - sq) / (2.0 * a);
    const double t_radial_hi = (-b + sq) / (2.0 * a);
    const double lo = std::max({t_radial_lo, ty0, 0.0});
    const double hi = std::min({t_radial_hi, ty1, max_t});
    return lo <= hi ? lo : -1.0;
}

} // namespace

DepthCameraSensor::DepthCameraSensor(const agbot::config::ParamTable& params) {
    width_ = static_cast<int>(agbot::config::integer_or(params, "width", width_));
    height_ = static_cast<int>(agbot::config::integer_or(params, "height", height_));
    horizontal_fov_deg_ =
        agbot::config::double_or(params, "horizontal_fov_deg", horizontal_fov_deg_);
    vertical_fov_deg_ = agbot::config::double_or(params, "vertical_fov_deg", vertical_fov_deg_);
    max_range_m_ = agbot::config::double_or(params, "max_range_m", max_range_m_);
    mount_yaw_rad_ = agbot::config::double_or(params, "mount_yaw_rad", mount_yaw_rad_);
    mount_pitch_rad_ = agbot::config::double_or(params, "mount_pitch_rad", mount_pitch_rad_);
    mount_height_m_ = agbot::config::double_or(params, "mount_height_m", mount_height_m_);
    range_noise_a_ = agbot::config::double_or(params, "range_noise_a", range_noise_a_);
    dropout_pct_ = agbot::config::double_or(params, "dropout_pct", dropout_pct_);
    seed_ = static_cast<std::uint64_t>(
        agbot::config::integer_or(params, "seed", static_cast<std::int64_t>(seed_)));
}

SensorFrame DepthCameraSensor::sense(
    const NavWorld& world,
    const agbot::vehicles::EntityState& state,
    double time_s) {
    SensorFrame frame;
    frame.stamp_s = time_s;
    frame.width = width_;
    frame.height = height_;
    if (width_ <= 0 || height_ <= 0 || max_range_m_ <= 0.0
        || horizontal_fov_deg_ <= 0.0 || horizontal_fov_deg_ >= 179.0
        || vertical_fov_deg_ <= 0.0 || vertical_fov_deg_ >= 179.0) {
        frame.status = "invalid_camera_config";
        frame.reason = "camera intrinsics are invalid";
        return frame;
    }

    const Vec3 origin{
        state.position.x,
        state.position.y + mount_height_m_,
        state.position.z,
    };
    const double yaw = state.yaw_rad + mount_yaw_rad_;
    const double pitch = state.pitch_rad + mount_pitch_rad_; // positive pitches down
    const Vec3 forward{
        std::cos(pitch) * std::cos(yaw),
        -std::sin(pitch),
        std::cos(pitch) * std::sin(yaw),
    };
    const Vec3 world_up{0.0, 1.0, 0.0};
    const Vec3 right = cross(forward, world_up).normalized();
    const Vec3 camera_up = cross(right, forward).normalized();
    const double tan_half_h = std::tan(deg_to_rad(horizontal_fov_deg_) * 0.5);
    const double tan_half_v = std::tan(deg_to_rad(vertical_fov_deg_) * 0.5);

    std::mt19937_64 rng(seed_ ^ (frame_index_ * 0x9e3779b97f4a7c15ULL));
    ++frame_index_;
    std::normal_distribution<double> unit_normal(0.0, 1.0);
    std::uniform_real_distribution<double> unit_uniform(0.0, 1.0);

    frame.depth_m.assign(static_cast<std::size_t>(width_) * static_cast<std::size_t>(height_), 0.0);
    frame.cloud.points.reserve(frame.depth_m.size());
    frame.cloud.classes.reserve(frame.depth_m.size());
    frame.cloud.object_ids.reserve(frame.depth_m.size());

    for (int py = 0; py < height_; ++py) {
        for (int px = 0; px < width_; ++px) {
            const double u =
                ((static_cast<double>(px) + 0.5) / static_cast<double>(width_) - 0.5) * 2.0;
            const double v =
                (0.5 - (static_cast<double>(py) + 0.5) / static_cast<double>(height_)) * 2.0;
            const Vec3 dir =
                (forward + right * (tan_half_h * u) + camera_up * (tan_half_v * v)).normalized();

            double best_t = -1.0;
            std::uint32_t hit_class = kClassGround;
            std::uint32_t hit_object = 0;

            // Ground plane.
            if (dir.y < -kEps) {
                const double t = (world.ground_height_m - origin.y) / dir.y;
                if (t >= 0.0 && t <= max_range_m_) {
                    best_t = t;
                }
            }

            // Scene object prisms.
            for (std::size_t oi = 0; oi < world.scene.objects.size(); ++oi) {
                const auto& object = world.scene.objects[oi];
                const double limit = best_t > 0.0 ? best_t : max_range_m_;
                const double t = ray_prism_entry(
                    origin,
                    dir,
                    object.footprint_local_m,
                    world.ground_height_m,
                    world.ground_height_m + object.height_m,
                    limit);
                if (t >= 0.0 && (best_t < 0.0 || t < best_t)) {
                    best_t = t;
                    hit_class = kClassObstacle;
                    hit_object = static_cast<std::uint32_t>(oi + 1);
                }
            }

            // Dynamic agents as vertical cylinders. Empty agent lists skip
            // this loop entirely, keeping the static-world depth image and
            // noise stream bit-identical to the pre-agent behavior.
            for (const DynamicAgent& agent : world.agents) {
                const double limit = best_t > 0.0 ? best_t : max_range_m_;
                const double t = ray_cylinder_entry(
                    origin,
                    dir,
                    agent.x,
                    agent.z,
                    agent.radius_m,
                    world.ground_height_m,
                    world.ground_height_m + agent.height_m,
                    limit);
                if (t >= 0.0 && (best_t < 0.0 || t < best_t)) {
                    best_t = t;
                    hit_class = agent_class_id(agent.kind);
                    hit_object = kDynamicObjectIdBase + agent.id;
                }
            }

            // Draw noise samples for every pixel so the random stream stays
            // aligned regardless of hits (keeps frames deterministic under
            // scene changes elsewhere in the image).
            const double noise = unit_normal(rng);
            const double dropout_draw = unit_uniform(rng);
            if (best_t < 0.0) {
                continue;
            }
            if (dropout_pct_ > 0.0 && dropout_draw * 100.0 < dropout_pct_) {
                continue;
            }

            double range = best_t;
            if (range_noise_a_ > 0.0) {
                range += noise * range_noise_a_ * range * range;
                range = std::clamp(range, 0.0, max_range_m_);
            }
            frame.depth_m[static_cast<std::size_t>(py) * static_cast<std::size_t>(width_)
                          + static_cast<std::size_t>(px)] = range;
            // Bias the world point a hair into the surface along the ray so
            // grid rasterization assigns boundary hits to the obstacle-side
            // cell instead of the free cell that merely touches the face.
            frame.cloud.points.push_back(origin + dir * (range + kSurfaceBiasM));
            frame.cloud.classes.push_back(hit_class);
            frame.cloud.object_ids.push_back(hit_object);
        }
    }
    return frame;
}

const SensorRegistry& default_sensor_registry() {
    static const SensorRegistry registry = [] {
        SensorRegistry built;
        built.register_factory(
            "depth_camera",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<ISensor> {
                return std::make_unique<DepthCameraSensor>(params);
            });
        return built;
    }();
    return registry;
}

} // namespace agbot::nav
