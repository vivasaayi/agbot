#pragma once

#include <cmath>
#include <ostream>

namespace agbot::flight_sim {

struct Vec3 {
    double x = 0.0;
    double y = 0.0;
    double z = 0.0;

    constexpr Vec3() = default;
    constexpr Vec3(double x_value, double y_value, double z_value)
        : x(x_value), y(y_value), z(z_value) {}

    [[nodiscard]] double length() const {
        return std::sqrt(x * x + y * y + z * z);
    }

    [[nodiscard]] double horizontal_length() const {
        return std::sqrt(x * x + z * z);
    }

    [[nodiscard]] Vec3 normalized() const {
        const double value = length();
        if (value <= 1e-9) {
            return {};
        }
        return {x / value, y / value, z / value};
    }
};

inline Vec3 operator+(const Vec3& lhs, const Vec3& rhs) {
    return {lhs.x + rhs.x, lhs.y + rhs.y, lhs.z + rhs.z};
}

inline Vec3 operator-(const Vec3& lhs, const Vec3& rhs) {
    return {lhs.x - rhs.x, lhs.y - rhs.y, lhs.z - rhs.z};
}

inline Vec3 operator*(const Vec3& value, double scalar) {
    return {value.x * scalar, value.y * scalar, value.z * scalar};
}

inline Vec3 operator/(const Vec3& value, double scalar) {
    return {value.x / scalar, value.y / scalar, value.z / scalar};
}

inline Vec3& operator+=(Vec3& lhs, const Vec3& rhs) {
    lhs.x += rhs.x;
    lhs.y += rhs.y;
    lhs.z += rhs.z;
    return lhs;
}

inline std::ostream& operator<<(std::ostream& os, const Vec3& value) {
    os << "(" << value.x << ", " << value.y << ", " << value.z << ")";
    return os;
}

} // namespace agbot::flight_sim
