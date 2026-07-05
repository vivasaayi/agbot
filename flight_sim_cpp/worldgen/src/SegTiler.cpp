#include "agbot_worldgen/SegTiler.hpp"

#include <algorithm>
#include <cstddef>

namespace agbot::worldgen {
namespace {

// Tile start positions along one axis: 0, stride, 2*stride, ... with the
// final tile clamped so it ends exactly on the image edge.
std::vector<int> axis_positions(int image_extent, int tile_size, int stride) {
    std::vector<int> positions{0};
    while (positions.back() + tile_size < image_extent) {
        positions.push_back(
            std::min(positions.back() + stride, image_extent - tile_size));
    }
    return positions;
}

} // namespace

std::vector<TileRect> plan_tiles(
    int image_width, int image_height, int tile_size, int overlap_px) {
    std::vector<TileRect> tiles;
    if (image_width <= 0 || image_height <= 0 || tile_size <= 0) {
        return tiles;
    }
    const int overlap = std::clamp(overlap_px, 0, tile_size - 1);
    const int stride = tile_size - overlap;
    const int tile_w = std::min(tile_size, image_width);
    const int tile_h = std::min(tile_size, image_height);
    const std::vector<int> xs = axis_positions(image_width, tile_w, stride);
    const std::vector<int> ys = axis_positions(image_height, tile_h, stride);
    tiles.reserve(xs.size() * ys.size());
    for (const int y : ys) {
        for (const int x : xs) {
            tiles.push_back({x, y, tile_w, tile_h});
        }
    }
    return tiles;
}

TileStitcher::TileStitcher(int width, int height)
    : width_(std::max(0, width)),
      height_(std::max(0, height)),
      classes_(static_cast<std::size_t>(width_) * static_cast<std::size_t>(height_), 0),
      confidence_(static_cast<std::size_t>(width_) * static_cast<std::size_t>(height_), 0.0f),
      priority_(static_cast<std::size_t>(width_) * static_cast<std::size_t>(height_), -1) {}

void TileStitcher::commit(
    const TileRect& rect,
    const std::vector<std::uint8_t>& tile_classes,
    const std::vector<float>& tile_confidence) {
    if (rect.width <= 0 || rect.height <= 0 || rect.x0 < 0 || rect.y0 < 0 ||
        rect.x0 + rect.width > width_ || rect.y0 + rect.height > height_) {
        return;
    }
    const std::size_t expected =
        static_cast<std::size_t>(rect.width) * static_cast<std::size_t>(rect.height);
    if (tile_classes.size() != expected || tile_confidence.size() != expected) {
        return;
    }
    for (int ty = 0; ty < rect.height; ++ty) {
        for (int tx = 0; tx < rect.width; ++tx) {
            const std::int32_t priority = std::min(
                std::min(tx, rect.width - 1 - tx), std::min(ty, rect.height - 1 - ty));
            const std::size_t image_index =
                static_cast<std::size_t>(rect.y0 + ty) * static_cast<std::size_t>(width_) +
                static_cast<std::size_t>(rect.x0 + tx);
            if (priority > priority_[image_index]) {
                const std::size_t tile_index =
                    static_cast<std::size_t>(ty) * static_cast<std::size_t>(rect.width) +
                    static_cast<std::size_t>(tx);
                priority_[image_index] = priority;
                classes_[image_index] = tile_classes[tile_index];
                confidence_[image_index] = tile_confidence[tile_index];
            }
        }
    }
}

} // namespace agbot::worldgen
