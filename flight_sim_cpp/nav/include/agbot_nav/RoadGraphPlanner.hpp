#pragma once

#include "agbot_nav/GlobalPlanner.hpp"
#include "agbot_worldgen/RoadNetwork.hpp"

#include <memory>
#include <string>

namespace agbot::nav {

// Strategic global planner that routes on a real street network: start and
// goal are snapped onto the nearest road edge and the route is an A* search
// over the directed road graph (travel time or distance), reconstructed as
// the full centerline polyline (entry projection -> edges -> exit
// projection) in local meters. This is the layer above astar/hybrid_astar:
// road_graph picks the streets, a local geometric planner handles the
// last-meter geometry. Deterministic tie-breaks (f, then g, then node id).
//
// The road network is either injected via set_network() or self-loaded from
// params (roads_path + aoi_*). The costmap argument is ignored by default
// (streets are the map); set use_costmap = true to skip edges that cross
// lethal costmap cells.
//
// Params:
//   cost_mode         string  "time" | "distance" (default "time")
//   max_snap_m        float   max start/goal snap distance (default 50)
//   heuristic_weight  float   euclidean heuristic inflation (default 1.0)
//   use_costmap       bool    skip lethal edges (default false)
//   lethal_threshold  int     costmap lethal cost (default 200)
//   roads_path        string  optional Overpass/GeoJSON road file to
//                             self-load through the road_import extractor
//   aoi_min_lat, aoi_min_lon, aoi_max_lat, aoi_max_lon
//                     float   AOI for self-loading (origin = AOI center)
//   weld_tol_m, default_speed_mps, [class_speed_mps]
//                             forwarded to the road network build
class RoadGraphPlanner final : public IGlobalPlanner {
public:
    RoadGraphPlanner() = default;
    explicit RoadGraphPlanner(const agbot::config::ParamTable& params);

    void set_network(std::shared_ptr<const agbot::worldgen::RoadNetwork> network) {
        network_ = std::move(network);
        load_error_.clear();
    }

    PlanResult plan(const Costmap& costmap, const Vec3& start, const Vec3& goal) override;
    [[nodiscard]] std::string name() const override { return "road_graph"; }

private:
    std::shared_ptr<const agbot::worldgen::RoadNetwork> network_;
    std::string load_error_;
    std::string cost_mode_ = "time";
    double max_snap_m_ = 50.0;
    double heuristic_weight_ = 1.0;
    bool use_costmap_ = false;
    std::uint8_t lethal_threshold_ = 200;
};

} // namespace agbot::nav
