#pragma once

#include "agbot_flight_sim/SceneSynthesis.hpp"
#include "agbot_flight_sim/Vec3.hpp"

#include <cmath>
#include <cstdint>
#include <string>
#include <vector>

namespace agbot::nav {

using agbot::flight_sim::Vec3;

struct PointCloud {
    std::vector<Vec3> points;
    std::vector<std::uint32_t> classes; // parallel to points; semantic class ids

    [[nodiscard]] std::size_t size() const { return points.size(); }
    [[nodiscard]] bool empty() const { return points.empty(); }
};

// Semantic class ids used across the pipeline.
inline constexpr std::uint32_t kClassGround = 0;
inline constexpr std::uint32_t kClassObstacle = 1;

struct Pose2D {
    double x = 0.0;
    double z = 0.0;
    double yaw = 0.0;
};

// Occupancy cost grid on the XZ plane. Cell values: 0..254 traversal cost
// (0 = free, 254 = lethal), 255 = unknown.
struct OccupancyGrid {
    double origin_x = 0.0; // world x of cell (0, 0) corner
    double origin_z = 0.0; // world z of cell (0, 0) corner
    double resolution_m = 0.25;
    int width = 0;  // cells along x
    int height = 0; // cells along z
    std::vector<std::uint8_t> cells;
    double stamp_s = 0.0;

    static constexpr std::uint8_t kLethal = 254;
    static constexpr std::uint8_t kUnknown = 255;

    void reset(std::uint8_t fill) {
        cells.assign(static_cast<std::size_t>(width) * static_cast<std::size_t>(height), fill);
    }

    [[nodiscard]] bool in_bounds(int cx, int cz) const {
        return cx >= 0 && cz >= 0 && cx < width && cz < height;
    }

    [[nodiscard]] std::size_t index(int cx, int cz) const {
        return static_cast<std::size_t>(cz) * static_cast<std::size_t>(width)
            + static_cast<std::size_t>(cx);
    }

    [[nodiscard]] std::uint8_t at(int cx, int cz) const { return cells[index(cx, cz)]; }
    void set(int cx, int cz, std::uint8_t value) { cells[index(cx, cz)] = value; }

    [[nodiscard]] bool world_to_cell(double wx, double wz, int& cx, int& cz) const {
        cx = static_cast<int>(std::floor((wx - origin_x) / resolution_m));
        cz = static_cast<int>(std::floor((wz - origin_z) / resolution_m));
        return in_bounds(cx, cz);
    }

    [[nodiscard]] Vec3 cell_to_world(int cx, int cz) const {
        return {
            origin_x + (static_cast<double>(cx) + 0.5) * resolution_m,
            0.0,
            origin_z + (static_cast<double>(cz) + 0.5) * resolution_m,
        };
    }

    // Cost at a world position; kUnknown when outside the grid.
    [[nodiscard]] std::uint8_t cost_at_world(double wx, double wz) const {
        int cx = 0;
        int cz = 0;
        if (!world_to_cell(wx, wz, cx, cz)) {
            return kUnknown;
        }
        return at(cx, cz);
    }
};

// A costmap is an occupancy grid after obstacle inflation.
using Costmap = OccupancyGrid;

struct Path {
    std::vector<Vec3> points;

    [[nodiscard]] double length_m() const {
        double total = 0.0;
        for (std::size_t i = 1; i < points.size(); ++i) {
            total += (points[i] - points[i - 1]).horizontal_length();
        }
        return total;
    }
};

struct TrajectoryPoint {
    Vec3 position;
    double yaw = 0.0;
    double v = 0.0;
    double t = 0.0;
};

struct Trajectory {
    std::vector<TrajectoryPoint> points;
};

// Synthetic world the navigation sensors observe: a flat ground plane plus the
// scene-synthesis objects treated as extruded footprint prisms.
struct NavWorld {
    double ground_height_m = 0.0;
    agbot::flight_sim::SceneSynthesisManifest scene;
};

struct NavTelemetry {
    double time_s = 0.0;
    Pose2D pose;
    double speed_mps = 0.0;
    std::uint8_t robot_cell_cost = 0;
    std::size_t costmap_occupied_cells = 0;
    std::size_t costmap_lethal_cells = 0;
    double path_length_m = 0.0;
    double min_obstacle_distance_m = 0.0; // min sensed range in the latest frame
    double distance_to_goal_m = 0.0;
    bool goal_reached = false;
};

} // namespace agbot::nav
