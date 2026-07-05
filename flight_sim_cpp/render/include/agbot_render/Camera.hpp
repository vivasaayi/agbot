#pragma once

#include "agbot_render/Mat4.hpp"

namespace agbot::render {

// Fly (FPS-style) camera. Yaw/pitch in radians.
// Convention: yaw = 0, pitch = 0 looks down -Z; positive yaw turns right
// (toward +X); positive pitch looks up. World up is +Y.
class Camera {
public:
    Vec3f position{0.0F, 2.0F, 10.0F};
    float yaw_rad = 0.0F;
    float pitch_rad = 0.0F;
    float fov_y_deg = 60.0F;
    float near_plane = 0.1F;
    float far_plane = 5000.0F;

    Vec3f forward() const;
    Vec3f right() const;
    Vec3f up() const;

    Mat4 view_matrix() const;
    Mat4 proj_matrix(float aspect) const;
    Mat4 view_proj_matrix(float aspect) const;

    // Movement helpers for the fly camera (meters / radians).
    void move_forward(float distance_m);
    void move_right(float distance_m);
    void move_up(float distance_m);
    void add_yaw_pitch(float delta_yaw_rad, float delta_pitch_rad);
    void zoom_fov(float delta_deg);
};

} // namespace agbot::render
