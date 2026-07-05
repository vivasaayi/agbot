#include "agbot_terrain/WaterMask.hpp"

#include <queue>

namespace agbot::terrain {

WaterMask compute_water_mask(const Raster& elevation, float sea_level_m) {
    WaterMask mask;
    if (!elevation.valid()) {
        return mask;
    }
    mask.width = elevation.width;
    mask.height = elevation.height;
    const std::size_t cells =
        static_cast<std::size_t>(mask.width) * static_cast<std::size_t>(mask.height);
    mask.is_water.assign(cells, 0);

    // A cell is a water candidate when its elevation is at/below sea level, or
    // nodata (unknown surface at the coast). Flood-fill candidates inward from
    // the boundary so only sea-connected water is masked.
    const auto is_candidate = [&](int row, int col) {
        const float value = elevation.at(row, col);
        return Raster::is_nodata(value) || value <= sea_level_m;
    };

    std::queue<std::pair<int, int>> frontier;
    const auto enqueue = [&](int row, int col) {
        if (row < 0 || col < 0 || row >= mask.height || col >= mask.width) {
            return;
        }
        const std::size_t idx =
            static_cast<std::size_t>(row) * static_cast<std::size_t>(mask.width) +
            static_cast<std::size_t>(col);
        if (mask.is_water[idx] != 0 || !is_candidate(row, col)) {
            return;
        }
        mask.is_water[idx] = 1;
        frontier.emplace(row, col);
    };

    for (int col = 0; col < mask.width; ++col) {
        enqueue(0, col);
        enqueue(mask.height - 1, col);
    }
    for (int row = 0; row < mask.height; ++row) {
        enqueue(row, 0);
        enqueue(row, mask.width - 1);
    }
    while (!frontier.empty()) {
        const auto [row, col] = frontier.front();
        frontier.pop();
        enqueue(row - 1, col);
        enqueue(row + 1, col);
        enqueue(row, col - 1);
        enqueue(row, col + 1);
    }

    for (const std::uint8_t flag : mask.is_water) {
        mask.water_cells += flag;
    }
    return mask;
}

} // namespace agbot::terrain
