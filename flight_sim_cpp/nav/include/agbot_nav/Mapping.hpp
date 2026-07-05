#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_nav/NavTypes.hpp"
#include "agbot_vehicles/ParamRegistry.hpp"

#include <memory>
#include <string>

namespace agbot::nav {

class IMapper {
public:
    virtual ~IMapper() = default;
    // Integrate one batch of world-frame obstacle points observed from
    // sensor_pose at stamp_s.
    virtual void integrate(
        const PointCloud& obstacles,
        const Pose2D& sensor_pose,
        double stamp_s) = 0;
    [[nodiscard]] virtual const OccupancyGrid& grid() const = 0;
    [[nodiscard]] virtual std::string name() const = 0;
};

// Log-odds-lite occupancy mapper in the spirit of
// lidar_mapper::build_occupancy_grid: points quantize to cells, each hit
// increments the cell cost, an optional per-integration decay forgets dynamic
// obstacles, and an optional 2D free-space carve decrements cells along the
// sensor ray. Params: origin_x, origin_z, width, height, resolution_m,
// hit_increment, decay, carve_free, carve_decrement.
class OccupancyGridMapper final : public IMapper {
public:
    OccupancyGridMapper();
    explicit OccupancyGridMapper(const agbot::config::ParamTable& params);

    void integrate(
        const PointCloud& obstacles,
        const Pose2D& sensor_pose,
        double stamp_s) override;

    [[nodiscard]] const OccupancyGrid& grid() const override { return grid_; }
    [[nodiscard]] std::string name() const override { return "occupancy_grid"; }

private:
    OccupancyGrid grid_;
    int hit_increment_ = 96;
    int decay_ = 0;
    bool carve_free_ = false;
    int carve_decrement_ = 32;
};

// Obstacle inflation: cells at or above lethal_threshold stay lethal, nearby
// cells receive an exponentially decaying cost within inflation_radius_m.
// The falloff is normalized by the radius so the radius genuinely reshapes
// the cost field: cost(d) = round((kLethal - 1) * exp(-cost_scaling * d /
// inflation_radius_m)). Params: inflation_radius_m, cost_scaling,
// lethal_threshold.
class InflationLayer {
public:
    InflationLayer() = default;
    explicit InflationLayer(const agbot::config::ParamTable& params);

    [[nodiscard]] Costmap inflate(const OccupancyGrid& grid) const;

    [[nodiscard]] double inflation_radius_m() const { return inflation_radius_m_; }
    [[nodiscard]] std::uint8_t lethal_threshold() const { return lethal_threshold_; }

private:
    double inflation_radius_m_ = 0.8;
    double cost_scaling_ = 3.0;
    std::uint8_t lethal_threshold_ = 200;
};

using MapperRegistry = agbot::vehicles::ParamRegistry<IMapper>;
[[nodiscard]] const MapperRegistry& default_mapper_registry();

} // namespace agbot::nav
