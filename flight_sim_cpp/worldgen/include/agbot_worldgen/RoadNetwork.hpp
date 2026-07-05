#pragma once

#include "agbot_worldgen/Feature.hpp"

#include "agbot_flight_sim/Vec3.hpp"

#include <cstdint>
#include <map>
#include <string>
#include <vector>

namespace agbot::worldgen {

using agbot::flight_sim::Vec3;

// Graph node in local meters (x = east, z = north) around the build origin.
struct RoadNode {
    std::uint32_t id = 0;
    double x = 0.0;
    double z = 0.0;
};

// Directed edge along a road centerline between two welded nodes. `polyline`
// runs from the `from` node to the `to` node in local meters.
struct RoadEdge {
    std::uint32_t id = 0;
    std::uint32_t from = 0;
    std::uint32_t to = 0;
    double length_m = 0.0;
    double speed_mps = 0.0;
    double travel_time_s = 0.0;
    std::string highway;
    std::string way_id;
    int lanes = 0;
    std::vector<Vec3> polyline;
};

struct RoadNetworkParams {
    // Vertices closer than this are welded onto one node (OSM ways share
    // exact node coordinates at intersections; the tolerance absorbs float
    // round-trips).
    double weld_tol_m = 1.5;
    // highway class -> speed (m/s); classes absent from the map fall back to
    // default_speed_mps.
    std::map<std::string, double> class_speed_mps = {
        {"motorway", 25.0},      {"trunk", 20.0},   {"primary", 14.0},
        {"secondary", 12.0},     {"tertiary", 8.0}, {"residential", 8.0},
        {"unclassified", 8.0},   {"living_street", 5.0}, {"service", 5.0},
    };
    double default_speed_mps = 8.0;
};

// Overrides RoadNetworkParams defaults from a ParamTable: weld_tol_m,
// default_speed_mps, and a nested [class_speed_mps] table.
[[nodiscard]] RoadNetworkParams road_network_params_from(
    const agbot::config::ParamTable& table);

// Result of projecting a query point onto the street network.
struct EdgeProjection {
    bool ok = false;
    std::uint32_t edge_id = 0;
    Vec3 point;          // projected point on the edge polyline
    double distance_m = 0.0; // query point -> projected point
    double s_along_m = 0.0;  // arc length from the edge start to the projection
};

// Routable street graph built from Road-class extracted features (open
// polylines in `exterior`, see RoadImportExtractor). Nodes are polyline
// endpoints plus vertices shared by more than one way (welded within
// weld_tol_m via a spatial hash); edges are the directed polyline runs
// between consecutive nodes. Oneway "yes" emits the forward edge only,
// "-1" the reversed edge only, anything else both directions. Node ids are
// deterministic (sorted by (x, z)); edges are emitted in source_id order.
class RoadNetwork {
public:
    [[nodiscard]] static RoadNetwork build(
        const std::vector<ExtractedFeature>& features,
        const agbot::flight_sim::GeoCoordinate& origin,
        const RoadNetworkParams& params);

    [[nodiscard]] const std::vector<RoadNode>& nodes() const { return nodes_; }
    [[nodiscard]] const std::vector<RoadEdge>& edges() const { return edges_; }

    // Ids of edges leaving `node_id` (sorted ascending).
    [[nodiscard]] const std::vector<std::uint32_t>& out_edges(std::uint32_t node_id) const;

    // Reverse companion of a directed edge (same geometry, opposite
    // direction, same way), or nullptr for oneway edges.
    [[nodiscard]] const RoadEdge* reverse_edge(std::uint32_t edge_id) const;

    // Closest point on any edge polyline to (x, z); brute force over
    // segments with deterministic tie-break (smaller distance, then smaller
    // edge id). ok == false when the network has no edges.
    [[nodiscard]] EdgeProjection nearest_edge_point(double x, double z) const;

    // Size of the largest weakly connected component, in nodes.
    [[nodiscard]] std::size_t largest_component_size() const;

    // FNV1a-64 over node coordinates and edge topology; determinism checks.
    [[nodiscard]] std::uint64_t graph_hash() const;

private:
    std::vector<RoadNode> nodes_;
    std::vector<RoadEdge> edges_;
    std::vector<std::vector<std::uint32_t>> adjacency_; // node id -> out edge ids
    std::vector<std::uint32_t> reverse_edge_; // edge id -> reverse id or npos
    static constexpr std::uint32_t kNoEdge = 0xffffffffu;
};

} // namespace agbot::worldgen
