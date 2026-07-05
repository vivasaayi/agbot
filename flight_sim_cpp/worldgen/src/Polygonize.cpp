#include "agbot_worldgen/Polygonize.hpp"

#include "agbot_worldgen/Feature.hpp"

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <utility>

namespace agbot::worldgen {
namespace {

using agbot::flight_sim::GeoBounds;
using agbot::flight_sim::GeoCoordinate;
using agbot::flight_sim::Vec3;

// Integer corner coordinate on the (width+1) x (height+1) cell-corner grid.
struct Corner {
    int x = 0;
    int y = 0;
};

struct BoundaryEdge {
    int from = 0; // corner index: y * (width + 1) + x
    int to = 0;
    std::int32_t label = -1;
};

// 4-connected component labeling in scanline discovery order. Returns the
// number of components; `labels` gets -1 for non-target cells.
std::int32_t label_components(
    const ClassMask& mask, std::uint8_t target, std::vector<std::int32_t>& labels) {
    const int w = mask.width;
    const int h = mask.height;
    labels.assign(static_cast<std::size_t>(w) * static_cast<std::size_t>(h), -1);
    std::int32_t next_label = 0;
    std::vector<int> stack;
    for (int y = 0; y < h; ++y) {
        for (int x = 0; x < w; ++x) {
            const std::size_t index = static_cast<std::size_t>(y) * w + x;
            if (mask.classes[index] != target || labels[index] >= 0) {
                continue;
            }
            const std::int32_t label = next_label++;
            labels[index] = label;
            stack.clear();
            stack.push_back(static_cast<int>(index));
            while (!stack.empty()) {
                const int cell = stack.back();
                stack.pop_back();
                const int cx = cell % w;
                const int cy = cell / w;
                const auto visit = [&](int nx, int ny) {
                    if (nx < 0 || nx >= w || ny < 0 || ny >= h) {
                        return;
                    }
                    const std::size_t neighbor = static_cast<std::size_t>(ny) * w + nx;
                    if (mask.classes[neighbor] == target && labels[neighbor] < 0) {
                        labels[neighbor] = label;
                        stack.push_back(static_cast<int>(neighbor));
                    }
                };
                visit(cx, cy - 1);
                visit(cx - 1, cy);
                visit(cx + 1, cy);
                visit(cx, cy + 1);
            }
        }
    }
    return next_label;
}

// Emits directed boundary edges on the corner grid with the component
// interior on the RIGHT of the travel direction (raster coordinates, y grows
// south). Emission order is deterministic (scanline over cells).
std::vector<BoundaryEdge> collect_boundary_edges(
    const ClassMask& mask, const std::vector<std::int32_t>& labels) {
    const int w = mask.width;
    const int h = mask.height;
    const int cw = w + 1; // corner-grid width
    std::vector<BoundaryEdge> edges;
    const auto label_at = [&](int x, int y) -> std::int32_t {
        if (x < 0 || x >= w || y < 0 || y >= h) {
            return -1;
        }
        return labels[static_cast<std::size_t>(y) * w + x];
    };
    for (int y = 0; y < h; ++y) {
        for (int x = 0; x < w; ++x) {
            const std::int32_t label = label_at(x, y);
            if (label < 0) {
                continue;
            }
            const int nw = y * cw + x;          // north-west corner
            const int ne = y * cw + x + 1;      // north-east
            const int sw = (y + 1) * cw + x;    // south-west
            const int se = (y + 1) * cw + x + 1;
            if (label_at(x, y - 1) != label) {
                edges.push_back({nw, ne, label}); // top side, heading east
            }
            if (label_at(x + 1, y) != label) {
                edges.push_back({ne, se, label}); // right side, heading south
            }
            if (label_at(x, y + 1) != label) {
                edges.push_back({se, sw, label}); // bottom side, heading west
            }
            if (label_at(x - 1, y) != label) {
                edges.push_back({sw, nw, label}); // left side, heading north
            }
        }
    }
    return edges;
}

// Direction of an edge on the corner grid (unit steps).
struct Direction {
    int dx = 0;
    int dy = 0;
};

Direction edge_direction(const BoundaryEdge& edge, int corner_width) {
    const int fx = edge.from % corner_width;
    const int fy = edge.from / corner_width;
    const int tx = edge.to % corner_width;
    const int ty = edge.to / corner_width;
    return {tx - fx, ty - fy};
}

// Clockwise rotation in raster coordinates (y down): the interior side.
Direction rotate_cw(const Direction& d) {
    return {-d.dy, d.dx};
}

Direction rotate_ccw(const Direction& d) {
    return {d.dy, -d.dx};
}

// A closed loop of corner points (no duplicated closing point), collinear
// runs already collapsed.
struct CornerLoop {
    std::vector<Corner> points;
    std::int32_t label = -1;
    double signed_area_cells = 0.0; // shoelace in raster coords (y down)
};

// Stitches directed edges into closed loops. At a saddle corner (two
// diagonal-touching boundaries of the same component meeting at one corner)
// the sharpest RIGHT turn is taken so the trace hugs the interior; this
// keeps exterior and hole boundaries separate. Loops start from the first
// unused edge in emission order, so the result is deterministic.
std::vector<CornerLoop> stitch_loops(
    const std::vector<BoundaryEdge>& edges, int corner_width) {
    // Outgoing edge indices per corner, in emission order (max 4).
    std::vector<std::vector<int>> outgoing;
    int max_corner = 0;
    for (const BoundaryEdge& edge : edges) {
        max_corner = std::max(max_corner, std::max(edge.from, edge.to));
    }
    outgoing.resize(static_cast<std::size_t>(max_corner) + 1);
    for (std::size_t i = 0; i < edges.size(); ++i) {
        outgoing[static_cast<std::size_t>(edges[i].from)].push_back(static_cast<int>(i));
    }

    std::vector<bool> used(edges.size(), false);
    std::vector<CornerLoop> loops;
    for (std::size_t start = 0; start < edges.size(); ++start) {
        if (used[start]) {
            continue;
        }
        CornerLoop loop;
        loop.label = edges[start].label;
        std::vector<Corner> raw;
        int current = static_cast<int>(start);
        while (!used[static_cast<std::size_t>(current)]) {
            used[static_cast<std::size_t>(current)] = true;
            const BoundaryEdge& edge = edges[static_cast<std::size_t>(current)];
            raw.push_back({edge.from % corner_width, edge.from / corner_width});

            const Direction incoming = edge_direction(edge, corner_width);
            const Direction prefer[3] = {
                rotate_cw(incoming), incoming, rotate_ccw(incoming)};
            int next = -1;
            for (const Direction& want : prefer) {
                for (const int candidate : outgoing[static_cast<std::size_t>(edge.to)]) {
                    if (used[static_cast<std::size_t>(candidate)] ||
                        edges[static_cast<std::size_t>(candidate)].label != edge.label) {
                        continue;
                    }
                    const Direction d =
                        edge_direction(edges[static_cast<std::size_t>(candidate)], corner_width);
                    if (d.dx == want.dx && d.dy == want.dy) {
                        next = candidate;
                        break;
                    }
                }
                if (next >= 0) {
                    break;
                }
            }
            if (next < 0) {
                break; // loop closed (start edge already used)
            }
            current = next;
        }

        // Collapse collinear runs (closed-ring aware).
        const std::size_t n = raw.size();
        for (std::size_t i = 0; i < n; ++i) {
            const Corner& prev = raw[(i + n - 1) % n];
            const Corner& here = raw[i];
            const Corner& following = raw[(i + 1) % n];
            const int d1x = here.x - prev.x;
            const int d1y = here.y - prev.y;
            const int d2x = following.x - here.x;
            const int d2y = following.y - here.y;
            if (d1x * d2y - d1y * d2x != 0) {
                loop.points.push_back(here);
            }
        }
        if (loop.points.size() < 3) {
            continue;
        }
        double doubled = 0.0;
        for (std::size_t i = 0; i < loop.points.size(); ++i) {
            const Corner& a = loop.points[i];
            const Corner& b = loop.points[(i + 1) % loop.points.size()];
            doubled += static_cast<double>(a.x) * b.y - static_cast<double>(b.x) * a.y;
        }
        loop.signed_area_cells = doubled * 0.5;
        loops.push_back(std::move(loop));
    }
    return loops;
}

GeoCoordinate corner_to_geo(const Corner& corner, const ClassMask& mask, const GeoBounds& aoi) {
    const double fx = static_cast<double>(corner.x) / static_cast<double>(mask.width);
    const double fy = static_cast<double>(corner.y) / static_cast<double>(mask.height);
    return {
        aoi.max_latitude - fy * (aoi.max_latitude - aoi.min_latitude),
        aoi.min_longitude + fx * (aoi.max_longitude - aoi.min_longitude),
        0.0,
    };
}

double point_segment_distance(const Vec3& point, const Vec3& start, const Vec3& end) {
    const double dx = end.x - start.x;
    const double dz = end.z - start.z;
    const double length_sq = dx * dx + dz * dz;
    double t = 0.0;
    if (length_sq > 0.0) {
        t = ((point.x - start.x) * dx + (point.z - start.z) * dz) / length_sq;
        t = std::clamp(t, 0.0, 1.0);
    }
    const double px = start.x + t * dx - point.x;
    const double pz = start.z + t * dz - point.z;
    return std::sqrt(px * px + pz * pz);
}

void douglas_peucker(
    const std::vector<Vec3>& points,
    std::size_t first,
    std::size_t last,
    double tolerance_m,
    std::vector<bool>& keep) {
    if (last <= first + 1) {
        return;
    }
    double max_distance = -1.0;
    std::size_t max_index = first;
    for (std::size_t index = first + 1; index < last; ++index) {
        const double distance = point_segment_distance(points[index], points[first], points[last]);
        if (distance > max_distance) {
            max_distance = distance;
            max_index = index;
        }
    }
    if (max_distance > tolerance_m) {
        keep[max_index] = true;
        douglas_peucker(points, first, max_index, tolerance_m, keep);
        douglas_peucker(points, max_index, last, tolerance_m, keep);
    }
}

// Douglas-Peucker in local meters over a closed ring (closing edge
// included). Keeps the original ring when simplification would collapse it
// below a triangle.
std::vector<GeoCoordinate> simplify_ring(
    const std::vector<GeoCoordinate>& ring,
    const GeoCoordinate& origin,
    double tolerance_m) {
    if (tolerance_m <= 0.0 || ring.size() <= 3) {
        return ring;
    }
    std::vector<Vec3> local;
    local.reserve(ring.size() + 1);
    for (const GeoCoordinate& point : ring) {
        local.push_back(agbot::flight_sim::local_from_geo(point, origin));
    }
    local.push_back(local.front());

    std::vector<bool> keep(local.size(), false);
    keep.front() = true;
    keep.back() = true;
    douglas_peucker(local, 0, local.size() - 1, tolerance_m, keep);

    std::vector<GeoCoordinate> simplified;
    simplified.reserve(ring.size());
    for (std::size_t index = 0; index < ring.size(); ++index) {
        if (keep[index]) {
            simplified.push_back(ring[index]);
        }
    }
    if (simplified.size() < 3) {
        return ring;
    }
    return simplified;
}

// Signed shoelace in local meters (x east, z north): positive = CCW.
double signed_area_local(
    const std::vector<GeoCoordinate>& ring, const GeoCoordinate& origin) {
    if (ring.size() < 3) {
        return 0.0;
    }
    double doubled = 0.0;
    Vec3 previous = agbot::flight_sim::local_from_geo(ring.back(), origin);
    for (const GeoCoordinate& point : ring) {
        const Vec3 current = agbot::flight_sim::local_from_geo(point, origin);
        doubled += previous.x * current.z - current.x * previous.z;
        previous = current;
    }
    return doubled * 0.5;
}

void enforce_orientation(
    std::vector<GeoCoordinate>& ring, const GeoCoordinate& origin, bool want_ccw) {
    const double area = signed_area_local(ring, origin);
    if ((want_ccw && area < 0.0) || (!want_ccw && area > 0.0)) {
        std::reverse(ring.begin(), ring.end());
    }
}

} // namespace

std::vector<RasterPolygon> polygonize_class(
    const ClassMask& mask,
    const GeoBounds& aoi,
    std::uint8_t target,
    const PolygonizeOptions& options,
    std::vector<std::int32_t>* labels_out) {
    std::vector<RasterPolygon> polygons;
    if (mask.width <= 0 || mask.height <= 0 ||
        mask.classes.size() !=
            static_cast<std::size_t>(mask.width) * static_cast<std::size_t>(mask.height)) {
        if (labels_out != nullptr) {
            labels_out->clear();
        }
        return polygons;
    }

    std::vector<std::int32_t> labels;
    const std::int32_t component_count = label_components(mask, target, labels);
    std::vector<int> cell_counts(static_cast<std::size_t>(component_count), 0);
    for (const std::int32_t label : labels) {
        if (label >= 0) {
            ++cell_counts[static_cast<std::size_t>(label)];
        }
    }
    if (labels_out != nullptr) {
        *labels_out = labels;
    }
    if (component_count == 0) {
        return polygons;
    }

    const std::vector<BoundaryEdge> edges = collect_boundary_edges(mask, labels);
    std::vector<CornerLoop> loops = stitch_loops(edges, mask.width + 1);

    // Group loops per component; the loop with the largest |area| is the
    // exterior, all others are holes.
    std::vector<std::vector<std::size_t>> loops_by_label(
        static_cast<std::size_t>(component_count));
    for (std::size_t i = 0; i < loops.size(); ++i) {
        loops_by_label[static_cast<std::size_t>(loops[i].label)].push_back(i);
    }

    const GeoCoordinate origin = aoi.center();
    for (std::int32_t label = 0; label < component_count; ++label) {
        const std::vector<std::size_t>& indices =
            loops_by_label[static_cast<std::size_t>(label)];
        if (indices.empty()) {
            continue;
        }
        std::size_t exterior_index = indices.front();
        for (const std::size_t index : indices) {
            if (std::abs(loops[index].signed_area_cells) >
                std::abs(loops[exterior_index].signed_area_cells)) {
                exterior_index = index;
            }
        }

        const auto to_geo_ring = [&](const CornerLoop& loop) {
            std::vector<GeoCoordinate> ring;
            ring.reserve(loop.points.size());
            for (const Corner& corner : loop.points) {
                ring.push_back(corner_to_geo(corner, mask, aoi));
            }
            return simplify_ring(ring, origin, options.simplify_tol_m);
        };

        RasterPolygon polygon;
        polygon.component_label = label;
        polygon.cell_count = cell_counts[static_cast<std::size_t>(label)];
        polygon.exterior = to_geo_ring(loops[exterior_index]);
        enforce_orientation(polygon.exterior, origin, /*want_ccw=*/true);
        for (const std::size_t index : indices) {
            if (index == exterior_index) {
                continue;
            }
            std::vector<GeoCoordinate> hole = to_geo_ring(loops[index]);
            if (hole.size() >= 3) {
                enforce_orientation(hole, origin, /*want_ccw=*/false);
                polygon.holes.push_back(std::move(hole));
            }
        }

        polygon.area_m2 = ring_area_m2(polygon.exterior, origin);
        for (const std::vector<GeoCoordinate>& hole : polygon.holes) {
            polygon.area_m2 -= ring_area_m2(hole, origin);
        }
        polygon.area_m2 = std::max(0.0, polygon.area_m2);
        if (polygon.area_m2 < options.min_area_m2) {
            continue;
        }
        polygons.push_back(std::move(polygon));
    }
    return polygons;
}

} // namespace agbot::worldgen
