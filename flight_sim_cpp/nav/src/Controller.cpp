#include "agbot_nav/Controller.hpp"

#include <algorithm>
#include <cmath>
#include <limits>

namespace agbot::nav {

namespace {

constexpr double kPi = 3.14159265358979323846;

double wrap_angle(double angle) {
    while (angle > kPi) {
        angle -= 2.0 * kPi;
    }
    while (angle < -kPi) {
        angle += 2.0 * kPi;
    }
    return angle;
}

struct PathReference {
    double crosstrack = 0.0; // signed: positive when the vehicle is left of the path (+cross)
    double path_yaw = 0.0;
};

PathReference reference_at(const Path& path, const Vec3& probe) {
    PathReference reference;
    if (path.points.size() < 2) {
        return reference;
    }
    double best_distance = std::numeric_limits<double>::infinity();
    for (std::size_t i = 1; i < path.points.size(); ++i) {
        const Vec3& a = path.points[i - 1];
        const Vec3& b = path.points[i];
        const double abx = b.x - a.x;
        const double abz = b.z - a.z;
        const double denom = abx * abx + abz * abz;
        if (denom <= 1e-12) {
            continue;
        }
        double t = ((probe.x - a.x) * abx + (probe.z - a.z) * abz) / denom;
        t = std::clamp(t, 0.0, 1.0);
        const double px = a.x + abx * t;
        const double pz = a.z + abz * t;
        const double dx = probe.x - px;
        const double dz = probe.z - pz;
        const double distance = std::sqrt(dx * dx + dz * dz);
        if (distance < best_distance) {
            best_distance = distance;
            const double segment_length = std::sqrt(denom);
            const double dir_x = abx / segment_length;
            const double dir_z = abz / segment_length;
            reference.path_yaw = std::atan2(dir_z, dir_x);
            // 2D cross product on the XZ plane: positive when the probe sits
            // on the +cross side of the segment direction.
            reference.crosstrack = dir_x * dz - dir_z * dx;
        }
    }
    return reference;
}

} // namespace

PidStanleyController::PidStanleyController(const agbot::config::ParamTable& params) {
    kp_ = agbot::config::double_or(params, "kp", kp_);
    ki_ = agbot::config::double_or(params, "ki", ki_);
    kd_ = agbot::config::double_or(params, "kd", kd_);
    integral_limit_ = agbot::config::double_or(params, "integral_limit", integral_limit_);
    k_e_ = agbot::config::double_or(params, "k_e", k_e_);
    k_soft_ = agbot::config::double_or(params, "k_soft", k_soft_);
}

void PidStanleyController::reset() {
    integral_ = 0.0;
    previous_error_ = 0.0;
    has_previous_error_ = false;
}

agbot::vehicles::Actuation PidStanleyController::control(
    const agbot::vehicles::EntityState& state,
    const Path& reference,
    double v_cmd,
    const agbot::vehicles::VehicleLimits& limits,
    double dt_s) {
    agbot::vehicles::Actuation actuation;
    if (dt_s <= 0.0) {
        return actuation;
    }

    // Longitudinal PID -> throttle in [-1, 1].
    const double speed = state.velocity.x * std::cos(state.yaw_rad)
        + state.velocity.z * std::sin(state.yaw_rad);
    const double error = v_cmd - speed;
    integral_ = std::clamp(integral_ + error * dt_s, -integral_limit_, integral_limit_);
    const double derivative =
        has_previous_error_ ? (error - previous_error_) / dt_s : 0.0;
    previous_error_ = error;
    has_previous_error_ = true;
    const double accel = kp_ * error + ki_ * integral_ + kd_ * derivative;
    actuation.throttle = accel >= 0.0
        ? std::min(1.0, accel / std::max(1e-6, limits.max_accel_mps2))
        : std::max(-1.0, accel / std::max(1e-6, limits.max_brake_mps2));

    // Stanley lateral control from the front axle.
    if (reference.points.size() >= 2) {
        const Vec3 front_axle{
            state.position.x + std::cos(state.yaw_rad) * limits.wheelbase_m,
            state.position.y,
            state.position.z + std::sin(state.yaw_rad) * limits.wheelbase_m,
        };
        const PathReference ref = reference_at(reference, front_axle);
        const double heading_error = wrap_angle(ref.path_yaw - state.yaw_rad);
        const double crosstrack_term =
            std::atan2(k_e_ * ref.crosstrack, k_soft_ + std::max(0.0, speed));
        actuation.steer_rad = std::clamp(
            heading_error - crosstrack_term, -limits.max_steer_rad, limits.max_steer_rad);
    }
    return actuation;
}

const ControllerRegistry& default_controller_registry() {
    static const ControllerRegistry registry = [] {
        ControllerRegistry built;
        built.register_factory(
            "pid_stanley",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<IController> {
                return std::make_unique<PidStanleyController>(params);
            });
        return built;
    }();
    return registry;
}

} // namespace agbot::nav
