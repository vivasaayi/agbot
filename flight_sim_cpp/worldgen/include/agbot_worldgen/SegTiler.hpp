#pragma once

#include <cstdint>
#include <vector>

namespace agbot::worldgen {

// One tile of an image, in image pixel coordinates.
struct TileRect {
    int x0 = 0;
    int y0 = 0;
    int width = 0;
    int height = 0;
};

// Plans an overlapping tile grid over an image. Tiles are tile_size square
// where the image allows; when the image is smaller than tile_size along an
// axis a single tile spans that whole axis (the caller pads for the model).
// The last tile per axis is clamped to the image edge so every pixel is
// covered. Order is row-major (top-to-bottom, left-to-right), deterministic.
[[nodiscard]] std::vector<TileRect> plan_tiles(
    int image_width, int image_height, int tile_size, int overlap_px);

// Stitches per-tile class/confidence rasters back into a full-image mosaic.
// Overlap resolution is center-crop priority: a pixel keeps the value from
// the tile in which it lies deepest (largest distance to the nearest tile
// edge); on ties the earliest committed tile wins, so the result is
// deterministic for a fixed commit order.
class TileStitcher {
public:
    TileStitcher(int width, int height);

    // `tile_classes` / `tile_confidence` are tile-local row-major buffers of
    // rect.width * rect.height entries. Out-of-image or size-mismatched
    // commits are ignored.
    void commit(
        const TileRect& rect,
        const std::vector<std::uint8_t>& tile_classes,
        const std::vector<float>& tile_confidence);

    [[nodiscard]] int width() const { return width_; }
    [[nodiscard]] int height() const { return height_; }
    [[nodiscard]] const std::vector<std::uint8_t>& classes() const { return classes_; }
    [[nodiscard]] const std::vector<float>& confidence() const { return confidence_; }

private:
    int width_ = 0;
    int height_ = 0;
    std::vector<std::uint8_t> classes_;
    std::vector<float> confidence_;
    std::vector<std::int32_t> priority_;
};

} // namespace agbot::worldgen
