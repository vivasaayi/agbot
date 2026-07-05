#include "agbot_render/Camera.hpp"

#include <algorithm>
#include <cmath>

namespace agbot::render {

namespace {
constexpr float kPi = 3.14159265358979323846F;
constexpr float kMaxPitch = 1.55334303F; // ~89 degrees
} // namespace

Vec3f Camera::forward() const {
    const float cp = std::cos(pitch_rad);
    return Vec3f{
        std::sin(yaw_rad) * cp,
        std::sin(pitch_rad),
        -std::cos(yaw_rad) * cp,
    };
}

Vec3f Camera::right() const {
    return vec3_normalize(vec3_cross(forward(), Vec3f{0.0F, 1.0F, 0.0F}));
}

Vec3f Camera::up() const {
    return vec3_cross(right(), forward());
}

Mat4 Camera::view_matrix() const {
    return mat4_look_at(position, vec3_add(position, forward()), Vec3f{0.0F, 1.0F, 0.0F});
}

Mat4 Camera::proj_matrix(float aspect) const {
    const float fov_y_rad = fov_y_deg * kPi / 180.0F;
    return mat4_perspective(fov_y_rad, aspect, near_plane, far_plane);
}

Mat4 Camera::view_proj_matrix(float aspect) const {
    return mat4_multiply(proj_matrix(aspect), view_matrix());
}

void Camera::move_forward(float distance_m) {
    position = vec3_add(position, vec3_scale(forward(), distance_m));
}

void Camera::move_right(float distance_m) {
    position = vec3_add(position, vec3_scale(right(), distance_m));
}

void Camera::move_up(float distance_m) {
    position = vec3_add(position, Vec3f{0.0F, distance_m, 0.0F});
}

void Camera::add_yaw_pitch(float delta_yaw_rad, float delta_pitch_rad) {
    yaw_rad += delta_yaw_rad;
    pitch_rad = std::clamp(pitch_rad + delta_pitch_rad, -kMaxPitch, kMaxPitch);
}

void Camera::zoom_fov(float delta_deg) {
    fov_y_deg = std::clamp(fov_y_deg + delta_deg, 10.0F, 120.0F);
}

} // namespace agbot::render
