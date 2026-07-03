#pragma once

#include "agbot_flight_sim/Mission.hpp"
#include "agbot_nav/NavTypes.hpp"
#include "agbot_worldgen/Feature.hpp"

#include <cstddef>
#include <vector>

// M6 — occupancy-based autonomy evidence loop.
//
// The delivery-robot navigation gate consumes the *same* compiled city geometry
// the sensor path observes: building footprints are rasterized into an
// occupancy costmap, a global planner routes a robot start->goal over it, and
// the plan is scored (length, clearance, collision-free, recovery). Keeping the
// occupancy source identical to the rendered geometry is what makes the gate a
// consistency check rather than a toy demo.
namespace agbot::nav {

// Occupancy rasterization parameters. The grid is a square centered on the
// scene origin with side 2*half_extent_m at resolution_m per cell. Lethal cells
// are grown by inflation_cells (Chebyshev) so plans keep clearance from walls.
struct CityOccupancyParams {
    double half_extent_m = 1350.0;
    double resolution_m = 3.0;
    int inflation_cells = 1;
};

// Rasterize building footprints (geodetic exterior rings, with holes subtracted)
// into a lethal occupancy grid using an even-odd scanline fill in cell space,
// then inflate. Courtyards (holes) are left free; street corridors between
// footprints stay free. Coordinates are projected to local ENU meters around
// `origin` via agbot::flight_sim::local_from_geo.
[[nodiscard]] OccupancyGrid build_city_occupancy(
    const std::vector<agbot::worldgen::ExtractedFeature>& buildings,
    const agbot::flight_sim::GeoCoordinate& origin,
    const CityOccupancyParams& params = {});

// Minimum clearance (m) from a path to the nearest lethal cell, via a bounded
// Chebyshev-ring search around each path point. Returns max_ring*resolution_m
// when no lethal cell is found within the search radius.
[[nodiscard]] double path_min_clearance_m(const OccupancyGrid& grid, const Path& path,
                                          int max_ring);

// Terminal failure class for the evidence loop; None on success.
enum class EvidenceFailure {
    None,
    StartBlocked,
    GoalBlocked,
    NoInitialPlan,
    NoRecoveryPlan,
};

[[nodiscard]] const char* to_string(EvidenceFailure failure);

// Where the robot starts and finishes, plus the occupancy rasterization to use.
struct EvidenceLoopSpec {
    Vec3 start{-1200.0, 0.0, -900.0};
    Vec3 goal{1200.0, 0.0, 900.0};
    CityOccupancyParams occupancy;
    // Half-side (in cells) of the box blocked at the plan midpoint to force a
    // recovery replan; 0 disables the recovery probe.
    int recovery_block_cells = 6;
    // If start/goal land on a lethal cell (e.g. inside a building), snap to the
    // nearest free cell within this many cells (goal tolerance). 0 disables
    // snapping, so a blocked endpoint is reason-coded instead.
    int snap_radius_cells = 30;
};

// Nav gate (Gate 5) evidence for one delivery-robot run.
struct EvidencePlanResult {
    bool ok = false;             // a valid goal-reaching plan exists (post-recovery)
    bool collision_free = false; // no plan vertex sits on a lethal cell
    double length_m = 0.0;       // final plan length
    double euclidean_m = 0.0;    // straight-line start->goal
    double min_clearance_m = 0.0;
    int replan_count = 0;
    int recovery_count = 0;
    std::size_t lethal_cells = 0;
    EvidenceFailure failure = EvidenceFailure::None;
};

// Build the city occupancy grid, plan start->goal with A*, then block the plan
// midpoint and replan to exercise recovery. Deterministic given identical
// inputs (the planner uses fixed tie-breaking).
[[nodiscard]] EvidencePlanResult run_evidence_loop(
    const std::vector<agbot::worldgen::ExtractedFeature>& buildings,
    const agbot::flight_sim::GeoCoordinate& origin,
    const EvidenceLoopSpec& spec = {});

} // namespace agbot::nav
