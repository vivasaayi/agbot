#pragma once

#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_flight_sim/Mission.hpp"

#include <cstdint>
#include <vector>

namespace agbot::worldgen {

// A class-id raster georeferenced to an AOI. Row-major, row 0 is the NORTH
// edge (max latitude), column 0 the WEST edge (min longitude); the mask is
// assumed to span the AOI exactly.
struct ClassMask {
    int width = 0;
    int height = 0;
    std::vector<std::uint8_t> classes;
};

struct PolygonizeOptions {
    // Douglas-Peucker tolerance in meters (local frame around the AOI
    // center); 0 disables simplification. Collinear boundary points are
    // always collapsed regardless of this value.
    double simplify_tol_m = 0.0;
    // Drop polygons whose planar area (exterior minus holes) is smaller.
    double min_area_m2 = 0.0;
};

// One vectorized connected blob of the target class. Rings are geodetic
// (lat/lon), stored without a duplicated closing point; the exterior winds
// counter-clockwise, holes clockwise (in the east/north plane).
struct RasterPolygon {
    std::vector<agbot::flight_sim::GeoCoordinate> exterior;
    std::vector<std::vector<agbot::flight_sim::GeoCoordinate>> holes;
    double area_m2 = 0.0;
    // Component label in scanline discovery order; matches `labels_out`.
    std::int32_t component_label = -1;
    // Number of raster cells in the component.
    int cell_count = 0;
};

// Vectorizes every 4-connected component of `target` cells in `mask` into a
// polygon with holes via boundary tracing on the cell-corner grid, followed
// by collinear collapse, optional Douglas-Peucker simplification, and a
// minimum-area filter. Output order is deterministic: components appear in
// scanline discovery order. When `labels_out` is non-null it receives one
// int32 per cell: -1 for non-target cells, else the component label.
[[nodiscard]] std::vector<RasterPolygon> polygonize_class(
    const ClassMask& mask,
    const agbot::flight_sim::GeoBounds& aoi,
    std::uint8_t target,
    const PolygonizeOptions& options,
    std::vector<std::int32_t>* labels_out = nullptr);

} // namespace agbot::worldgen
