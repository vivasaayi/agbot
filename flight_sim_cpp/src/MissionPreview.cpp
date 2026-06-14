#include "agbot_flight_sim/MissionPreview.hpp"

#include <algorithm>
#include <cmath>

namespace agbot::flight_sim {
namespace {

constexpr double kBoundaryEpsilon = 1e-9;
constexpr double kCoverageSampleSpacingM = 2.0;

bool same_coordinate(const GeoCoordinate& left, const GeoCoordinate& right) {
    return std::abs(left.latitude - right.latitude) <= kBoundaryEpsilon
        && std::abs(left.longitude - right.longitude) <= kBoundaryEpsilon;
}

bool point_in_polygon_xz(const Vec3& point, const std::vector<Vec3>& polygon) {
    if (polygon.size() < 4) {
        return false;
    }

    bool inside = false;
    for (std::size_t current = 0, previous = polygon.size() - 1; current < polygon.size(); previous = current++) {
        const Vec3& a = polygon[current];
        const Vec3& b = polygon[previous];
        const bool crosses = ((a.z > point.z) != (b.z > point.z))
            && (point.x < (b.x - a.x) * (point.z - a.z) / ((b.z - a.z) + kBoundaryEpsilon) + a.x);
        if (crosses) {
            inside = !inside;
        }
    }
    return inside;
}

double segment_length_xz(const Vec3& start, const Vec3& end) {
    const double dx = end.x - start.x;
    const double dz = end.z - start.z;
    return std::sqrt(dx * dx + dz * dz);
}

Vec3 interpolate_xz(const Vec3& start, const Vec3& end, double t) {
    return {
        start.x + (end.x - start.x) * t,
        start.y + (end.y - start.y) * t,
        start.z + (end.z - start.z) * t,
    };
}

double covered_path_fraction(
    const std::vector<Vec3>& path,
    const std::vector<Vec3>& boundary) {
    if (path.size() < 2 || boundary.size() < 4) {
        return 0.0;
    }

    double total_length_m = 0.0;
    double covered_length_m = 0.0;
    for (std::size_t index = 1; index < path.size(); ++index) {
        const Vec3& start = path[index - 1];
        const Vec3& end = path[index];
        const double length_m = segment_length_xz(start, end);
        if (length_m <= kBoundaryEpsilon) {
            continue;
        }

        const int samples = std::max(1, static_cast<int>(std::ceil(length_m / kCoverageSampleSpacingM)));
        const double sample_length_m = length_m / static_cast<double>(samples);
        for (int sample = 0; sample < samples; ++sample) {
            const double t = (static_cast<double>(sample) + 0.5) / static_cast<double>(samples);
            if (point_in_polygon_xz(interpolate_xz(start, end, t), boundary)) {
                covered_length_m += sample_length_m;
            }
        }
        total_length_m += length_m;
    }

    if (total_length_m <= kBoundaryEpsilon) {
        return 0.0;
    }
    return std::clamp(covered_length_m / total_length_m, 0.0, 1.0);
}

} // namespace

MissionPreviewOverlay build_mission_preview_overlay(const Mission& mission) {
    MissionPreviewOverlay overlay;
    overlay.mission_path_local.push_back(mission.home);
    for (const Waypoint& waypoint : mission.waypoints) {
        overlay.mission_path_local.push_back(waypoint.position);
    }

    if (!mission.field_boundary || mission.field_boundary->coordinates.size() < 4) {
        overlay.status = "no boundary";
        return overlay;
    }
    if (!mission.home_geo) {
        overlay.status = "boundary not georeferenced";
        return overlay;
    }

    overlay.boundary_geo = mission.field_boundary->coordinates;
    if (!same_coordinate(overlay.boundary_geo.front(), overlay.boundary_geo.back())) {
        overlay.boundary_geo.push_back(overlay.boundary_geo.front());
    }
    overlay.boundary_local.reserve(overlay.boundary_geo.size());
    for (const GeoCoordinate& coordinate : overlay.boundary_geo) {
        overlay.boundary_local.push_back(local_from_geo(coordinate, *mission.home_geo));
    }

    overlay.has_boundary = true;
    overlay.coverage_fraction = covered_path_fraction(overlay.mission_path_local, overlay.boundary_local);
    overlay.status = "field boundary aligned";
    return overlay;
}

} // namespace agbot::flight_sim
