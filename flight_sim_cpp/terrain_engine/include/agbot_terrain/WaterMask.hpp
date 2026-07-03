#pragma once

#include "agbot_terrain/Raster.hpp"

#include <cstddef>
#include <cstdint>
#include <vector>

namespace agbot::terrain {

// A boolean water mask over a heightfield grid (row 0 = north), plus a count of
// masked cells.
struct WaterMask {
    int width = 0;
    int height = 0;
    std::vector<std::uint8_t> is_water;   // row-major, 1 = water
    std::size_t water_cells = 0;

    [[nodiscard]] bool at(int row, int col) const {
        return is_water[static_cast<std::size_t>(row) * static_cast<std::size_t>(width) +
                        static_cast<std::size_t>(col)] != 0;
    }
};

// Detects water as the set of cells at or below `sea_level_m` that are connected
// (4-neighbourhood) to the grid boundary. This is the "no invented harbor holes"
// rule: a below-sea-level cell that is NOT reachable from the edge (an interior
// depression, an excavation, a nodata pit) is deliberately left as land so DEM
// resampling cannot fabricate a lake in the middle of the map. Nodata cells are
// treated as candidate water only when boundary-connected.
[[nodiscard]] WaterMask compute_water_mask(const Raster& elevation, float sea_level_m);

} // namespace agbot::terrain
