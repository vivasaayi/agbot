#pragma once

#include <array>
#include <cmath>

namespace agbot::render {

struct Vec3f {
    float x = 0.0F;
    float y = 0.0F;
    float z = 0.0F;
};

inline Vec3f vec3_add(const Vec3f& a, const Vec3f& b) {
    return Vec3f{a.x + b.x, a.y + b.y, a.z + b.z};
}

inline Vec3f vec3_sub(const Vec3f& a, const Vec3f& b) {
    return Vec3f{a.x - b.x, a.y - b.y, a.z - b.z};
}

inline Vec3f vec3_scale(const Vec3f& a, float s) {
    return Vec3f{a.x * s, a.y * s, a.z * s};
}

inline float vec3_dot(const Vec3f& a, const Vec3f& b) {
    return a.x * b.x + a.y * b.y + a.z * b.z;
}

inline Vec3f vec3_cross(const Vec3f& a, const Vec3f& b) {
    return Vec3f{
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    };
}

inline float vec3_length(const Vec3f& a) {
    return std::sqrt(vec3_dot(a, a));
}

inline Vec3f vec3_normalize(const Vec3f& a) {
    const float len = vec3_length(a);
    if (len <= 0.0F) {
        return Vec3f{0.0F, 0.0F, 0.0F};
    }
    return vec3_scale(a, 1.0F / len);
}

// Column-major 4x4 matrix (OpenGL convention).
// Element (row r, column c) lives at m[c * 4 + r].
struct Mat4 {
    std::array<float, 16> m{};

    float at(int row, int col) const { return m[static_cast<std::size_t>(col * 4 + row)]; }
    float& at(int row, int col) { return m[static_cast<std::size_t>(col * 4 + row)]; }
    const float* data() const { return m.data(); }
};

inline Mat4 mat4_identity() {
    Mat4 out;
    out.m = {1.0F, 0.0F, 0.0F, 0.0F,
             0.0F, 1.0F, 0.0F, 0.0F,
             0.0F, 0.0F, 1.0F, 0.0F,
             0.0F, 0.0F, 0.0F, 1.0F};
    return out;
}

// out = a * b (applies b first, then a — standard column-vector convention).
inline Mat4 mat4_multiply(const Mat4& a, const Mat4& b) {
    Mat4 out;
    for (int col = 0; col < 4; ++col) {
        for (int row = 0; row < 4; ++row) {
            float sum = 0.0F;
            for (int k = 0; k < 4; ++k) {
                sum += a.at(row, k) * b.at(k, col);
            }
            out.at(row, col) = sum;
        }
    }
    return out;
}

inline Mat4 mat4_translate(const Vec3f& t) {
    Mat4 out = mat4_identity();
    out.at(0, 3) = t.x;
    out.at(1, 3) = t.y;
    out.at(2, 3) = t.z;
    return out;
}

inline Mat4 mat4_scale(const Vec3f& s) {
    Mat4 out = mat4_identity();
    out.at(0, 0) = s.x;
    out.at(1, 1) = s.y;
    out.at(2, 2) = s.z;
    return out;
}

// Right-handed perspective projection mapping to clip z in [-1, 1] (OpenGL).
inline Mat4 mat4_perspective(float fovy_radians, float aspect, float near_plane, float far_plane) {
    const float f = 1.0F / std::tan(fovy_radians * 0.5F);
    Mat4 out;
    out.at(0, 0) = f / aspect;
    out.at(1, 1) = f;
    out.at(2, 2) = -(far_plane + near_plane) / (far_plane - near_plane);
    out.at(2, 3) = -(2.0F * far_plane * near_plane) / (far_plane - near_plane);
    out.at(3, 2) = -1.0F;
    return out;
}

// Right-handed lookAt view matrix (camera looks from eye toward center, +Y-ish up).
inline Mat4 mat4_look_at(const Vec3f& eye, const Vec3f& center, const Vec3f& up) {
    const Vec3f forward = vec3_normalize(vec3_sub(center, eye));
    const Vec3f side = vec3_normalize(vec3_cross(forward, up));
    const Vec3f true_up = vec3_cross(side, forward);

    Mat4 out = mat4_identity();
    out.at(0, 0) = side.x;
    out.at(0, 1) = side.y;
    out.at(0, 2) = side.z;
    out.at(1, 0) = true_up.x;
    out.at(1, 1) = true_up.y;
    out.at(1, 2) = true_up.z;
    out.at(2, 0) = -forward.x;
    out.at(2, 1) = -forward.y;
    out.at(2, 2) = -forward.z;
    out.at(0, 3) = -vec3_dot(side, eye);
    out.at(1, 3) = -vec3_dot(true_up, eye);
    out.at(2, 3) = vec3_dot(forward, eye);
    return out;
}

struct Vec4f {
    float x = 0.0F;
    float y = 0.0F;
    float z = 0.0F;
    float w = 0.0F;
};

inline Vec4f mat4_transform(const Mat4& m, const Vec4f& v) {
    Vec4f out;
    out.x = m.at(0, 0) * v.x + m.at(0, 1) * v.y + m.at(0, 2) * v.z + m.at(0, 3) * v.w;
    out.y = m.at(1, 0) * v.x + m.at(1, 1) * v.y + m.at(1, 2) * v.z + m.at(1, 3) * v.w;
    out.z = m.at(2, 0) * v.x + m.at(2, 1) * v.y + m.at(2, 2) * v.z + m.at(2, 3) * v.w;
    out.w = m.at(3, 0) * v.x + m.at(3, 1) * v.y + m.at(3, 2) * v.z + m.at(3, 3) * v.w;
    return out;
}

// Transforms a point (w = 1) and performs the perspective divide.
inline Vec3f mat4_transform_point(const Mat4& m, const Vec3f& p) {
    const Vec4f h = mat4_transform(m, Vec4f{p.x, p.y, p.z, 1.0F});
    if (std::fabs(h.w) < 1e-12F) {
        return Vec3f{h.x, h.y, h.z};
    }
    return Vec3f{h.x / h.w, h.y / h.w, h.z / h.w};
}

} // namespace agbot::render
