#include "agbot_nav/Mapping.hpp"

#include <algorithm>
#include <cmath>
#include <cstdlib>

namespace agbot::nav {

namespace {

std::uint8_t saturating_add(std::uint8_t value, int delta, std::uint8_t max_value) {
    const int result = static_cast<int>(value) + delta;
    return static_cast<std::uint8_t>(std::clamp(result, 0, static_cast<int>(max_value)));
}

} // namespace

OccupancyGridMapper::OccupancyGridMapper() {
    grid_.origin_x = -10.0;
    grid_.origin_z = -10.0;
    grid_.resolution_m = 0.25;
    grid_.width = 80;
    grid_.height = 80;
    grid_.reset(0);
}

OccupancyGridMapper::OccupancyGridMapper(const agbot::config::ParamTable& params) {
    grid_.origin_x = agbot::config::double_or(params, "origin_x", -10.0);
    grid_.origin_z = agbot::config::double_or(params, "origin_z", -10.0);
    grid_.resolution_m = agbot::config::double_or(params, "resolution_m", 0.25);
    grid_.width = static_cast<int>(agbot::config::integer_or(params, "width", 80));
    grid_.height = static_cast<int>(agbot::config::integer_or(params, "height", 80));
    hit_increment_ = static_cast<int>(agbot::config::integer_or(params, "hit_increment", 96));
    decay_ = static_cast<int>(agbot::config::integer_or(params, "decay", 0));
    carve_free_ = agbot::config::bool_or(params, "carve_free", false);
    carve_decrement_ = static_cast<int>(agbot::config::integer_or(params, "carve_decrement", 32));
    grid_.reset(0);
}

void OccupancyGridMapper::integrate(
    const PointCloud& obstacles,
    const Pose2D& sensor_pose,
    double stamp_s) {
    grid_.stamp_s = stamp_s;

    if (decay_ > 0) {
        for (std::uint8_t& cell : grid_.cells) {
            cell = saturating_add(cell, -decay_, OccupancyGrid::kLethal);
        }
    }

    int sensor_cx = 0;
    int sensor_cz = 0;
    const bool sensor_in_grid =
        grid_.world_to_cell(sensor_pose.x, sensor_pose.z, sensor_cx, sensor_cz);

    for (const Vec3& point : obstacles.points) {
        int cx = 0;
        int cz = 0;
        if (!grid_.world_to_cell(point.x, point.z, cx, cz)) {
            continue;
        }

        if (carve_free_ && sensor_in_grid) {
            // Bresenham 2D carve from the sensor cell to (but excluding) the
            // hit cell.
            int x = sensor_cx;
            int z = sensor_cz;
            const int dx = std::abs(cx - x);
            const int dz = std::abs(cz - z);
            const int sx = cx > x ? 1 : -1;
            const int sz = cz > z ? 1 : -1;
            int err = dx - dz;
            while (x != cx || z != cz) {
                if (grid_.in_bounds(x, z)) {
                    grid_.set(x, z,
                              saturating_add(grid_.at(x, z), -carve_decrement_,
                                             OccupancyGrid::kLethal));
                }
                const int doubled = 2 * err;
                if (doubled > -dz) {
                    err -= dz;
                    x += sx;
                }
                if (doubled < dx) {
                    err += dx;
                    z += sz;
                }
            }
        }

        grid_.set(cx, cz,
                  saturating_add(grid_.at(cx, cz), hit_increment_, OccupancyGrid::kLethal));
    }
}

InflationLayer::InflationLayer(const agbot::config::ParamTable& params) {
    inflation_radius_m_ =
        agbot::config::double_or(params, "inflation_radius_m", inflation_radius_m_);
    cost_scaling_ = agbot::config::double_or(params, "cost_scaling", cost_scaling_);
    lethal_threshold_ = static_cast<std::uint8_t>(std::clamp<std::int64_t>(
        agbot::config::integer_or(params, "lethal_threshold", lethal_threshold_), 1, 254));
}

Costmap InflationLayer::inflate(const OccupancyGrid& grid) const {
    Costmap costmap = grid;
    if (grid.width <= 0 || grid.height <= 0 || grid.resolution_m <= 0.0) {
        return costmap;
    }
    const int radius_cells =
        static_cast<int>(std::ceil(inflation_radius_m_ / grid.resolution_m));

    for (int cz = 0; cz < grid.height; ++cz) {
        for (int cx = 0; cx < grid.width; ++cx) {
            const std::uint8_t source = grid.at(cx, cz);
            if (source == OccupancyGrid::kUnknown || source < lethal_threshold_) {
                continue;
            }
            costmap.set(cx, cz, OccupancyGrid::kLethal);
            for (int dz = -radius_cells; dz <= radius_cells; ++dz) {
                for (int dx = -radius_cells; dx <= radius_cells; ++dx) {
                    if (dx == 0 && dz == 0) {
                        continue;
                    }
                    const int nx = cx + dx;
                    const int nz = cz + dz;
                    if (!grid.in_bounds(nx, nz)) {
                        continue;
                    }
                    const double distance_m =
                        std::sqrt(static_cast<double>(dx * dx + dz * dz)) * grid.resolution_m;
                    if (distance_m > inflation_radius_m_) {
                        continue;
                    }
                    const double cost = std::round(
                        static_cast<double>(OccupancyGrid::kLethal - 1)
                        * std::exp(-cost_scaling_ * distance_m
                                   / std::max(1e-6, inflation_radius_m_)));
                    const std::uint8_t inflated =
                        static_cast<std::uint8_t>(std::clamp(cost, 0.0, 253.0));
                    const std::uint8_t current = costmap.at(nx, nz);
                    if (current == OccupancyGrid::kUnknown || inflated > current) {
                        costmap.set(nx, nz, inflated);
                    }
                }
            }
        }
    }
    return costmap;
}

const MapperRegistry& default_mapper_registry() {
    static const MapperRegistry registry = [] {
        MapperRegistry built;
        built.register_factory(
            "occupancy_grid",
            [](const agbot::config::ParamTable& params) -> std::unique_ptr<IMapper> {
                return std::make_unique<OccupancyGridMapper>(params);
            });
        return built;
    }();
    return registry;
}

} // namespace agbot::nav
