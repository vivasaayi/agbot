#include "agbot_nav/CityEvidence.hpp"

#include "agbot_nav/GlobalPlanner.hpp"
#include "agbot_render/Mat4.hpp"

#include <algorithm>
#include <cmath>
#include <cstdint>

namespace agbot::nav {

namespace {

namespace fs = agbot::flight_sim;

// Even-odd scanline fill of one feature's rings (exterior + holes together) into
// cell space. A cell center inside an odd number of rings is set lethal, which
// leaves holes (courtyards) free automatically.
void rasterize_feature(const agbot::worldgen::ExtractedFeature& feature,
                       const fs::GeoCoordinate& origin, OccupancyGrid& grid) {
    // Collect every ring as fractional cell coordinates (fx along width, fz
    // along height), and track the covered row range.
    struct Edge {
        double x0, z0, x1, z1;
    };
    std::vector<Edge> edges;
    double min_fz = 1e18;
    double max_fz = -1e18;

    auto add_ring = [&](const std::vector<fs::GeoCoordinate>& ring) {
        if (ring.size() < 3) {
            return;
        }
        std::vector<std::pair<double, double>> pts;
        pts.reserve(ring.size());
        for (const auto& c : ring) {
            const fs::Vec3 p = fs::local_from_geo(c, origin);
            const double fx = (p.x - grid.origin_x) / grid.resolution_m;
            const double fz = (p.z - grid.origin_z) / grid.resolution_m;
            pts.emplace_back(fx, fz);
            min_fz = std::min(min_fz, fz);
            max_fz = std::max(max_fz, fz);
        }
        for (std::size_t i = 0; i < pts.size(); ++i) {
            const auto& a = pts[i];
            const auto& b = pts[(i + 1) % pts.size()];
            edges.push_back({a.first, a.second, b.first, b.second});
        }
    };

    add_ring(feature.exterior);
    for (const auto& hole : feature.holes) {
        add_ring(hole);
    }
    if (edges.empty()) {
        return;
    }

    int cz0 = std::max(0, static_cast<int>(std::floor(min_fz)));
    int cz1 = std::min(grid.height - 1, static_cast<int>(std::ceil(max_fz)));
    std::vector<double> crossings;
    for (int cz = cz0; cz <= cz1; ++cz) {
        const double zc = static_cast<double>(cz) + 0.5; // row-center scanline
        crossings.clear();
        for (const Edge& e : edges) {
            const double z0 = e.z0;
            const double z1 = e.z1;
            // Half-open edge test avoids double-counting shared vertices.
            if ((z0 <= zc && z1 > zc) || (z1 <= zc && z0 > zc)) {
                const double t = (zc - z0) / (z1 - z0);
                crossings.push_back(e.x0 + t * (e.x1 - e.x0));
            }
        }
        if (crossings.size() < 2) {
            continue;
        }
        std::sort(crossings.begin(), crossings.end());
        for (std::size_t i = 0; i + 1 < crossings.size(); i += 2) {
            int cx0 = std::max(0, static_cast<int>(std::ceil(crossings[i] - 0.5)));
            int cx1 = std::min(grid.width - 1,
                               static_cast<int>(std::floor(crossings[i + 1] - 0.5)));
            for (int cx = cx0; cx <= cx1; ++cx) {
                grid.set(cx, cz, OccupancyGrid::kLethal);
            }
        }
    }
}

// Grow lethal cells by `cells` in Chebyshev distance. Two-pass separable
// dilation keeps this O(width*height*cells) rather than O(n*cells^2).
void inflate(OccupancyGrid& grid, int cells) {
    if (cells <= 0) {
        return;
    }
    std::vector<std::uint8_t> src = grid.cells;
    // Horizontal pass.
    for (int cz = 0; cz < grid.height; ++cz) {
        for (int cx = 0; cx < grid.width; ++cx) {
            bool lethal = false;
            for (int dx = -cells; dx <= cells && !lethal; ++dx) {
                const int nx = cx + dx;
                if (nx >= 0 && nx < grid.width &&
                    src[grid.index(nx, cz)] >= OccupancyGrid::kLethal) {
                    lethal = true;
                }
            }
            if (lethal) {
                grid.set(cx, cz, OccupancyGrid::kLethal);
            }
        }
    }
    // Vertical pass over the horizontally-dilated buffer.
    src = grid.cells;
    for (int cz = 0; cz < grid.height; ++cz) {
        for (int cx = 0; cx < grid.width; ++cx) {
            bool lethal = false;
            for (int dz = -cells; dz <= cells && !lethal; ++dz) {
                const int nz = cz + dz;
                if (nz >= 0 && nz < grid.height &&
                    src[grid.index(cx, nz)] >= OccupancyGrid::kLethal) {
                    lethal = true;
                }
            }
            if (lethal) {
                grid.set(cx, cz, OccupancyGrid::kLethal);
            }
        }
    }
}

// Point at half the arc length along a path. A* smoothing string-pulls plans
// down to a few vertices, so a vertex index is a poor midpoint; interpolating by
// arc length gives a stable geometric middle regardless of vertex count.
Vec3 path_midpoint(const Path& path) {
    if (path.points.empty()) {
        return {};
    }
    if (path.points.size() == 1) {
        return path.points.front();
    }
    const double half = 0.5 * path.length_m();
    double acc = 0.0;
    for (std::size_t i = 1; i < path.points.size(); ++i) {
        const double seg = (path.points[i] - path.points[i - 1]).horizontal_length();
        if (acc + seg >= half && seg > 0.0) {
            const double t = (half - acc) / seg;
            return path.points[i - 1] + (path.points[i] - path.points[i - 1]) * t;
        }
        acc += seg;
    }
    return path.points.back();
}

// Nearest free (non-lethal, in-bounds) world position to `p` within max_ring
// cells, searched by expanding Chebyshev rings with deterministic ordering.
// Returns false when the point is off-grid or no free cell is within range.
bool snap_to_free(const OccupancyGrid& grid, const Vec3& p, int max_ring, Vec3& out) {
    int cx = 0;
    int cz = 0;
    if (!grid.world_to_cell(p.x, p.z, cx, cz)) {
        return false;
    }
    if (grid.at(cx, cz) < OccupancyGrid::kLethal) {
        out = p;
        return true;
    }
    for (int r = 1; r <= max_ring; ++r) {
        for (int dz = -r; dz <= r; ++dz) {
            for (int dx = -r; dx <= r; ++dx) {
                if (std::max(std::abs(dx), std::abs(dz)) != r) {
                    continue; // ring boundary only
                }
                const int nx = cx + dx;
                const int nz = cz + dz;
                if (grid.in_bounds(nx, nz) && grid.at(nx, nz) < OccupancyGrid::kLethal) {
                    out = grid.cell_to_world(nx, nz);
                    return true;
                }
            }
        }
    }
    return false;
}

} // namespace

OccupancyGrid build_city_occupancy(
    const std::vector<agbot::worldgen::ExtractedFeature>& buildings,
    const fs::GeoCoordinate& origin, const CityOccupancyParams& params) {
    OccupancyGrid grid;
    grid.resolution_m = params.resolution_m;
    grid.origin_x = -params.half_extent_m;
    grid.origin_z = -params.half_extent_m;
    grid.width = std::max(1, static_cast<int>(2.0 * params.half_extent_m / params.resolution_m));
    grid.height = grid.width;
    grid.reset(0);
    for (const auto& b : buildings) {
        rasterize_feature(b, origin, grid);
    }
    inflate(grid, params.inflation_cells);
    return grid;
}

double path_min_clearance_m(const OccupancyGrid& grid, const Path& path, int max_ring) {
    double min_clear = static_cast<double>(max_ring) * grid.resolution_m;
    for (const Vec3& p : path.points) {
        int cx = 0;
        int cz = 0;
        if (!grid.world_to_cell(p.x, p.z, cx, cz)) {
            continue;
        }
        for (int r = 0; r <= max_ring; ++r) {
            bool hit = false;
            for (int dz = -r; dz <= r && !hit; ++dz) {
                for (int dx = -r; dx <= r && !hit; ++dx) {
                    if (std::max(std::abs(dx), std::abs(dz)) != r) {
                        continue; // ring boundary only
                    }
                    const int nx = cx + dx;
                    const int nz = cz + dz;
                    if (grid.in_bounds(nx, nz) &&
                        grid.at(nx, nz) >= OccupancyGrid::kLethal) {
                        hit = true;
                    }
                }
            }
            if (hit) {
                min_clear = std::min(min_clear, static_cast<double>(r) * grid.resolution_m);
                break;
            }
        }
    }
    return min_clear;
}

const char* to_string(EvidenceFailure failure) {
    switch (failure) {
    case EvidenceFailure::None:
        return "none";
    case EvidenceFailure::StartBlocked:
        return "start_blocked";
    case EvidenceFailure::GoalBlocked:
        return "goal_blocked";
    case EvidenceFailure::NoInitialPlan:
        return "no_initial_plan";
    case EvidenceFailure::NoRecoveryPlan:
        return "no_recovery_plan";
    }
    return "unknown";
}

EvidencePlanResult run_evidence_loop(
    const std::vector<agbot::worldgen::ExtractedFeature>& buildings,
    const fs::GeoCoordinate& origin, const EvidenceLoopSpec& spec) {
    EvidencePlanResult r;
    OccupancyGrid grid = build_city_occupancy(buildings, origin, spec.occupancy);
    for (const auto cell : grid.cells) {
        if (cell >= OccupancyGrid::kLethal) {
            ++r.lethal_cells;
        }
    }

    // Snap endpoints off buildings to the nearest free cell (goal tolerance).
    // With snapping disabled, a blocked endpoint is reason-coded instead.
    Vec3 start = spec.start;
    Vec3 goal = spec.goal;
    if (!snap_to_free(grid, start, spec.snap_radius_cells, start)) {
        r.failure = EvidenceFailure::StartBlocked;
        r.euclidean_m = (spec.goal - spec.start).horizontal_length();
        return r;
    }
    if (!snap_to_free(grid, goal, spec.snap_radius_cells, goal)) {
        r.failure = EvidenceFailure::GoalBlocked;
        r.euclidean_m = (spec.goal - spec.start).horizontal_length();
        return r;
    }
    r.euclidean_m = (goal - start).horizontal_length();

    AStarPlanner planner;
    PlanResult plan = planner.plan(grid, start, goal);
    r.time_to_first_plan = plan.expanded;
    if (!plan.ok) {
        r.failure = EvidenceFailure::NoInitialPlan;
        return r;
    }
    r.ok = true;
    r.length_m = plan.path.length_m();
    r.min_clearance_m = path_min_clearance_m(grid, plan.path, 8);
    r.collision_free = true;
    for (const Vec3& p : plan.path.points) {
        if (grid.cost_at_world(p.x, p.z) >= OccupancyGrid::kLethal) {
            r.collision_free = false;
        }
    }

    // Recovery probe: block the plan midpoint and replan around it.
    if (spec.recovery_block_cells > 0 && plan.path.points.size() >= 2) {
        const Vec3 mid = path_midpoint(plan.path);
        int cx = 0;
        int cz = 0;
        if (grid.world_to_cell(mid.x, mid.z, cx, cz)) {
            const int b = spec.recovery_block_cells;
            for (int dz = -b; dz <= b; ++dz) {
                for (int dx = -b; dx <= b; ++dx) {
                    const int nx = cx + dx;
                    const int nz = cz + dz;
                    if (grid.in_bounds(nx, nz)) {
                        grid.set(nx, nz, OccupancyGrid::kLethal);
                    }
                }
            }
            ++r.recovery_count;
            PlanResult replan = planner.plan(grid, start, goal);
            ++r.replan_count;
            r.time_in_recovery = replan.expanded;
            if (replan.ok) {
                r.length_m = replan.path.length_m();
                r.min_clearance_m =
                    std::min(r.min_clearance_m, path_min_clearance_m(grid, replan.path, 8));
                r.collision_free = true;
                for (const Vec3& p : replan.path.points) {
                    if (grid.cost_at_world(p.x, p.z) >= OccupancyGrid::kLethal) {
                        r.collision_free = false;
                    }
                }
            } else {
                r.ok = false;
                r.failure = EvidenceFailure::NoRecoveryPlan;
            }
        }
    }
    return r;
}

OccupancyGrid occupancy_from_sensor_frame(const agbot::render::SensorFrame& frame,
                                          const agbot::render::OffscreenCamera& camera,
                                          const SensorOccupancyParams& params) {
    OccupancyGrid grid;
    grid.resolution_m = params.grid.resolution_m;
    grid.origin_x = -params.grid.half_extent_m;
    grid.origin_z = -params.grid.half_extent_m;
    grid.width =
        std::max(1, static_cast<int>(2.0 * params.grid.half_extent_m / params.grid.resolution_m));
    grid.height = grid.width;
    grid.reset(0);
    if (frame.width <= 0 || frame.height <= 0) {
        return grid;
    }

    // Camera basis matching render's mat4_look_at (forward = target-eye, right =
    // normalize(cross(forward, up)), true_up = cross(right, forward)).
    using agbot::render::Vec3f;
    const Vec3f forward = vec3_normalize(vec3_sub(camera.target, camera.eye));
    const Vec3f right = vec3_normalize(vec3_cross(forward, camera.up));
    const Vec3f true_up = vec3_cross(right, forward);
    const float aspect = static_cast<float>(frame.width) / static_cast<float>(frame.height);
    const float tan_half = std::tan(camera.fov_y_rad * 0.5f);

    for (int y = 0; y < frame.height; ++y) {
        for (int x = 0; x < frame.width; ++x) {
            const std::size_t idx = static_cast<std::size_t>(y) *
                    static_cast<std::size_t>(frame.width) +
                static_cast<std::size_t>(x);
            const float d = frame.depth[idx]; // eye-space metres along forward
            if (d <= 0.0f || d > params.max_range_m) {
                continue; // sky/miss or out of range
            }
            const float ndc_x = 2.0f * (static_cast<float>(x) + 0.5f) /
                    static_cast<float>(frame.width) -
                1.0f;
            const float ndc_y = 1.0f - 2.0f * (static_cast<float>(y) + 0.5f) /
                    static_cast<float>(frame.height);
            const float eye_x = ndc_x * aspect * tan_half * d;
            const float eye_y = ndc_y * tan_half * d;
            const Vec3f world = vec3_add(
                vec3_add(vec3_add(camera.eye, vec3_scale(right, eye_x)),
                         vec3_scale(true_up, eye_y)),
                vec3_scale(forward, d));
            if (world.y <= params.min_obstacle_height_m) {
                continue; // ground / terrain / road surface, not an obstacle
            }
            int cx = 0;
            int cz = 0;
            if (grid.world_to_cell(world.x, world.z, cx, cz)) {
                grid.set(cx, cz, OccupancyGrid::kLethal);
            }
        }
    }
    return grid;
}

OccupancyConsistency occupancy_consistency(const OccupancyGrid& sensor,
                                           const OccupancyGrid& footprint, int tolerance_cells) {
    OccupancyConsistency c;
    const bool same_grid = sensor.width == footprint.width && sensor.height == footprint.height;
    // Is any cell within tolerance of (cx,cz) lethal in `other`?
    const auto near_lethal = [tolerance_cells](const OccupancyGrid& other, int cx, int cz) {
        for (int dz = -tolerance_cells; dz <= tolerance_cells; ++dz) {
            for (int dx = -tolerance_cells; dx <= tolerance_cells; ++dx) {
                const int nx = cx + dx;
                const int nz = cz + dz;
                if (other.in_bounds(nx, nz) && other.at(nx, nz) >= OccupancyGrid::kLethal) {
                    return true;
                }
            }
        }
        return false;
    };
    if (!same_grid) {
        return c;
    }
    for (int cz = 0; cz < sensor.height; ++cz) {
        for (int cx = 0; cx < sensor.width; ++cx) {
            const bool s = sensor.at(cx, cz) >= OccupancyGrid::kLethal;
            const bool f = footprint.at(cx, cz) >= OccupancyGrid::kLethal;
            if (s) {
                ++c.sensor_lethal;
                if (near_lethal(footprint, cx, cz)) {
                    ++c.agree_lethal;
                }
            }
            if (f) {
                ++c.footprint_lethal;
            }
        }
    }
    if (c.sensor_lethal > 0) {
        c.precision = static_cast<double>(c.agree_lethal) / static_cast<double>(c.sensor_lethal);
    }
    // Recall: footprint cells seen by the sensor (within tolerance).
    if (c.footprint_lethal > 0) {
        std::size_t seen = 0;
        for (int cz = 0; cz < footprint.height; ++cz) {
            for (int cx = 0; cx < footprint.width; ++cx) {
                if (footprint.at(cx, cz) >= OccupancyGrid::kLethal && near_lethal(sensor, cx, cz)) {
                    ++seen;
                }
            }
        }
        c.recall = static_cast<double>(seen) / static_cast<double>(c.footprint_lethal);
    }
    return c;
}

} // namespace agbot::nav
