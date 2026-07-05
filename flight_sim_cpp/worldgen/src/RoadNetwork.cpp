#include "agbot_worldgen/RoadNetwork.hpp"

#include <algorithm>
#include <cmath>
#include <cstring>
#include <limits>
#include <numeric>
#include <unordered_map>

namespace agbot::worldgen {
namespace {

using agbot::flight_sim::GeoCoordinate;

constexpr std::uint32_t kInvalid = 0xffffffffu;

// Spatial-hash welder: maps nearby (< tolerance) points onto one canonical
// point id. The representative coordinate is the first-seen point, which is
// deterministic because callers feed points in sorted-way order.
class PointWelder {
public:
    explicit PointWelder(double tolerance_m)
        : tolerance_m_(std::max(tolerance_m, 1e-6)), cell_m_(std::max(tolerance_m, 1e-6)) {}

    std::uint32_t weld(double x, double z) {
        const std::int64_t cx = static_cast<std::int64_t>(std::floor(x / cell_m_));
        const std::int64_t cz = static_cast<std::int64_t>(std::floor(z / cell_m_));
        for (std::int64_t dz = -1; dz <= 1; ++dz) {
            for (std::int64_t dx = -1; dx <= 1; ++dx) {
                const auto it = cells_.find(key(cx + dx, cz + dz));
                if (it == cells_.end()) {
                    continue;
                }
                for (const std::uint32_t id : it->second) {
                    const double ex = xs_[id] - x;
                    const double ez = zs_[id] - z;
                    if (ex * ex + ez * ez <= tolerance_m_ * tolerance_m_) {
                        return id;
                    }
                }
            }
        }
        const std::uint32_t id = static_cast<std::uint32_t>(xs_.size());
        xs_.push_back(x);
        zs_.push_back(z);
        cells_[key(cx, cz)].push_back(id);
        return id;
    }

    [[nodiscard]] double x_of(std::uint32_t id) const { return xs_[id]; }
    [[nodiscard]] double z_of(std::uint32_t id) const { return zs_[id]; }
    [[nodiscard]] std::size_t size() const { return xs_.size(); }

private:
    static std::uint64_t key(std::int64_t cx, std::int64_t cz) {
        return (static_cast<std::uint64_t>(static_cast<std::uint32_t>(cx)) << 32) |
               static_cast<std::uint64_t>(static_cast<std::uint32_t>(cz));
    }

    double tolerance_m_;
    double cell_m_;
    std::vector<double> xs_;
    std::vector<double> zs_;
    std::unordered_map<std::uint64_t, std::vector<std::uint32_t>> cells_;
};

double polyline_length_m(const std::vector<Vec3>& polyline) {
    double total = 0.0;
    for (std::size_t i = 1; i < polyline.size(); ++i) {
        const double dx = polyline[i].x - polyline[i - 1].x;
        const double dz = polyline[i].z - polyline[i - 1].z;
        total += std::sqrt(dx * dx + dz * dz);
    }
    return total;
}

int parse_lanes(const std::map<std::string, std::string>& attributes) {
    const auto it = attributes.find("lanes");
    if (it == attributes.end()) {
        return 0;
    }
    try {
        return std::max(0, std::stoi(it->second));
    } catch (...) {
        return 0;
    }
}

std::string attribute_or(
    const std::map<std::string, std::string>& attributes,
    const std::string& key,
    const std::string& fallback) {
    const auto it = attributes.find(key);
    return it == attributes.end() ? fallback : it->second;
}

void hash_bytes(std::uint64_t& hash, const void* data, std::size_t size) {
    const auto* bytes = static_cast<const unsigned char*>(data);
    for (std::size_t i = 0; i < size; ++i) {
        hash ^= bytes[i];
        hash *= 1099511628211ull;
    }
}

} // namespace

RoadNetworkParams road_network_params_from(const agbot::config::ParamTable& table) {
    namespace cfg = agbot::config;
    RoadNetworkParams params;
    params.weld_tol_m = cfg::double_or(table, "weld_tol_m", params.weld_tol_m);
    params.default_speed_mps =
        cfg::double_or(table, "default_speed_mps", params.default_speed_mps);
    if (const cfg::ParamTable* speeds = cfg::find_table(table, "class_speed_mps")) {
        for (const auto& [cls, value] : *speeds) {
            if (value.is_number()) {
                params.class_speed_mps[cls] = value.as_double();
            }
        }
    }
    return params;
}

RoadNetwork RoadNetwork::build(
    const std::vector<ExtractedFeature>& features,
    const GeoCoordinate& origin,
    const RoadNetworkParams& params) {
    RoadNetwork network;

    // Deterministic feature order: sorted by source_id (SceneMesh idiom).
    std::vector<const ExtractedFeature*> roads;
    for (const ExtractedFeature& feature : features) {
        if (feature.cls == FeatureClass::Road && feature.exterior.size() >= 2) {
            roads.push_back(&feature);
        }
    }
    std::sort(roads.begin(), roads.end(),
              [](const ExtractedFeature* a, const ExtractedFeature* b) {
                  return a->source_id < b->source_id;
              });

    // Weld every polyline vertex; collapse consecutive duplicates.
    PointWelder welder(params.weld_tol_m);
    std::vector<std::vector<std::uint32_t>> way_points(roads.size());
    std::vector<std::uint32_t> use_count;
    for (std::size_t way = 0; way < roads.size(); ++way) {
        for (const GeoCoordinate& geo : roads[way]->exterior) {
            const Vec3 local = agbot::flight_sim::local_from_geo(geo, origin);
            const std::uint32_t id = welder.weld(local.x, local.z);
            if (!way_points[way].empty() && way_points[way].back() == id) {
                continue;
            }
            way_points[way].push_back(id);
        }
        if (way_points[way].size() < 2) {
            way_points[way].clear();
        }
    }
    use_count.assign(welder.size(), 0);
    std::vector<bool> is_node(welder.size(), false);
    for (const std::vector<std::uint32_t>& points : way_points) {
        if (points.empty()) {
            continue;
        }
        is_node[points.front()] = true;
        is_node[points.back()] = true;
        for (const std::uint32_t id : points) {
            if (++use_count[id] >= 2) {
                is_node[id] = true; // shared by two ways, or revisited (loop)
            }
        }
    }

    // Deterministic node ids: welded node points sorted by (x, z).
    std::vector<std::uint32_t> node_points;
    for (std::uint32_t id = 0; id < welder.size(); ++id) {
        if (is_node[id]) {
            node_points.push_back(id);
        }
    }
    std::sort(node_points.begin(), node_points.end(),
              [&welder](std::uint32_t a, std::uint32_t b) {
                  if (welder.x_of(a) != welder.x_of(b)) {
                      return welder.x_of(a) < welder.x_of(b);
                  }
                  if (welder.z_of(a) != welder.z_of(b)) {
                      return welder.z_of(a) < welder.z_of(b);
                  }
                  return a < b;
              });
    std::vector<std::uint32_t> node_id_of(welder.size(), kInvalid);
    network.nodes_.reserve(node_points.size());
    for (std::uint32_t node_id = 0; node_id < node_points.size(); ++node_id) {
        const std::uint32_t point = node_points[node_id];
        node_id_of[point] = node_id;
        network.nodes_.push_back({node_id, welder.x_of(point), welder.z_of(point)});
    }

    // Split each way at node vertices into directed edges.
    const auto speed_for = [&params](const std::string& highway) {
        const auto it = params.class_speed_mps.find(highway);
        return it == params.class_speed_mps.end() ? params.default_speed_mps : it->second;
    };
    const auto emit_edge = [&network](std::uint32_t from, std::uint32_t to,
                                      std::vector<Vec3> polyline, double speed_mps,
                                      const std::string& highway, const std::string& way_id,
                                      int lanes) -> std::uint32_t {
        const double length = polyline_length_m(polyline);
        if (length < 1e-6) {
            return kInvalid;
        }
        RoadEdge edge;
        edge.id = static_cast<std::uint32_t>(network.edges_.size());
        edge.from = from;
        edge.to = to;
        edge.length_m = length;
        edge.speed_mps = speed_mps;
        edge.travel_time_s = length / speed_mps;
        edge.highway = highway;
        edge.way_id = way_id;
        edge.lanes = lanes;
        edge.polyline = std::move(polyline);
        network.edges_.push_back(std::move(edge));
        network.reverse_edge_.push_back(kNoEdge);
        return network.edges_.back().id;
    };

    for (std::size_t way = 0; way < roads.size(); ++way) {
        const std::vector<std::uint32_t>& points = way_points[way];
        if (points.empty()) {
            continue;
        }
        const ExtractedFeature& feature = *roads[way];
        const std::string highway = attribute_or(feature.attributes, "highway", "");
        const std::string oneway = attribute_or(feature.attributes, "oneway", "no");
        const std::string way_id = attribute_or(feature.attributes, "way_id", feature.source_id);
        const int lanes = parse_lanes(feature.attributes);
        const double speed = std::max(speed_for(highway), 0.1);

        std::size_t run_start = 0;
        for (std::size_t i = 1; i < points.size(); ++i) {
            if (!is_node[points[i]]) {
                continue;
            }
            std::vector<Vec3> polyline;
            polyline.reserve(i - run_start + 1);
            for (std::size_t p = run_start; p <= i; ++p) {
                polyline.push_back({welder.x_of(points[p]), 0.0, welder.z_of(points[p])});
            }
            const std::uint32_t from = node_id_of[points[run_start]];
            const std::uint32_t to = node_id_of[points[i]];
            if (oneway == "yes") {
                emit_edge(from, to, std::move(polyline), speed, highway, way_id, lanes);
            } else if (oneway == "-1") {
                std::reverse(polyline.begin(), polyline.end());
                emit_edge(to, from, std::move(polyline), speed, highway, way_id, lanes);
            } else {
                std::vector<Vec3> reversed(polyline.rbegin(), polyline.rend());
                const std::uint32_t forward =
                    emit_edge(from, to, std::move(polyline), speed, highway, way_id, lanes);
                const std::uint32_t backward =
                    emit_edge(to, from, std::move(reversed), speed, highway, way_id, lanes);
                if (forward != kInvalid && backward != kInvalid) {
                    network.reverse_edge_[forward] = backward;
                    network.reverse_edge_[backward] = forward;
                }
            }
            run_start = i;
        }
    }

    network.adjacency_.assign(network.nodes_.size(), {});
    for (const RoadEdge& edge : network.edges_) {
        network.adjacency_[edge.from].push_back(edge.id);
    }
    return network;
}

const std::vector<std::uint32_t>& RoadNetwork::out_edges(std::uint32_t node_id) const {
    static const std::vector<std::uint32_t> kEmpty;
    if (node_id >= adjacency_.size()) {
        return kEmpty;
    }
    return adjacency_[node_id];
}

const RoadEdge* RoadNetwork::reverse_edge(std::uint32_t edge_id) const {
    if (edge_id >= reverse_edge_.size() || reverse_edge_[edge_id] == kNoEdge) {
        return nullptr;
    }
    return &edges_[reverse_edge_[edge_id]];
}

EdgeProjection RoadNetwork::nearest_edge_point(double x, double z) const {
    EdgeProjection best;
    best.distance_m = std::numeric_limits<double>::infinity();
    for (const RoadEdge& edge : edges_) {
        double s_offset = 0.0;
        for (std::size_t i = 1; i < edge.polyline.size(); ++i) {
            const Vec3& a = edge.polyline[i - 1];
            const Vec3& b = edge.polyline[i];
            const double dx = b.x - a.x;
            const double dz = b.z - a.z;
            const double length_sq = dx * dx + dz * dz;
            const double segment_length = std::sqrt(length_sq);
            double t = 0.0;
            if (length_sq > 0.0) {
                t = std::clamp(((x - a.x) * dx + (z - a.z) * dz) / length_sq, 0.0, 1.0);
            }
            const double px = a.x + t * dx;
            const double pz = a.z + t * dz;
            const double distance = std::hypot(px - x, pz - z);
            if (distance < best.distance_m) {
                best.ok = true;
                best.edge_id = edge.id;
                best.point = {px, 0.0, pz};
                best.distance_m = distance;
                best.s_along_m = s_offset + t * segment_length;
            }
            s_offset += segment_length;
        }
    }
    return best;
}

std::size_t RoadNetwork::largest_component_size() const {
    if (nodes_.empty()) {
        return 0;
    }
    std::vector<std::uint32_t> parent(nodes_.size());
    std::iota(parent.begin(), parent.end(), 0u);
    const auto find = [&parent](std::uint32_t v) {
        while (parent[v] != v) {
            parent[v] = parent[parent[v]];
            v = parent[v];
        }
        return v;
    };
    for (const RoadEdge& edge : edges_) {
        const std::uint32_t a = find(edge.from);
        const std::uint32_t b = find(edge.to);
        if (a != b) {
            parent[a] = b;
        }
    }
    std::unordered_map<std::uint32_t, std::size_t> sizes;
    std::size_t largest = 0;
    for (std::uint32_t v = 0; v < nodes_.size(); ++v) {
        largest = std::max(largest, ++sizes[find(v)]);
    }
    return largest;
}

std::uint64_t RoadNetwork::graph_hash() const {
    std::uint64_t hash = 1469598103934665603ull;
    for (const RoadNode& node : nodes_) {
        hash_bytes(hash, &node.x, sizeof(node.x));
        hash_bytes(hash, &node.z, sizeof(node.z));
    }
    for (const RoadEdge& edge : edges_) {
        hash_bytes(hash, &edge.from, sizeof(edge.from));
        hash_bytes(hash, &edge.to, sizeof(edge.to));
        hash_bytes(hash, &edge.length_m, sizeof(edge.length_m));
    }
    return hash;
}

} // namespace agbot::worldgen
