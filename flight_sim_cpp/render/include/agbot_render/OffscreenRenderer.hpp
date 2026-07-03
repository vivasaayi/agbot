#pragma once

#include "agbot_render/Mat4.hpp"
#include "agbot_render/RenderScene.hpp"

#include <cstddef>
#include <cstdint>
#include <vector>

namespace agbot::render {

// A pinhole sensor pose for the offscreen render path.
struct OffscreenCamera {
    Vec3f eye{0.0F, 0.0F, 0.0F};
    Vec3f target{0.0F, 0.0F, -1.0F};
    Vec3f up{0.0F, 1.0F, 0.0F};
    float fov_y_rad = 1.0471975512F;   // 60 degrees
    float near_m = 0.5F;
    float far_m = 8000.0F;
};

// Co-registered sensor outputs from one render: colour, linear eye-space depth
// (metres; negative where nothing was hit), and per-pixel semantic class id
// (0 = background/sky). All three observe the same scene geometry.
struct SensorFrame {
    int width = 0;
    int height = 0;
    std::vector<std::uint8_t> rgb;        // width*height*3, row-major, top-left origin
    std::vector<float> depth;             // width*height, linear metres; < 0 = miss
    std::vector<std::uint16_t> semantic;  // width*height, 0 = background
    std::size_t covered_pixels = 0;       // pixels with a geometry hit

    [[nodiscard]] double coverage_ratio() const;
};

// Rasterizes the scene's textured and static meshes into a SensorFrame with a
// deterministic z-buffered CPU rasterizer. static_meshes[i] is written with
// semantic id semantic_ids[i] (default i+1); textured meshes get ids 100+j and
// flat shading. Determinism: fixed mesh/triangle/pixel iteration, integer edge
// functions, perspective-correct depth.
[[nodiscard]] SensorFrame render_offscreen(
    const RenderScene& scene,
    const OffscreenCamera& camera,
    int width,
    int height,
    const std::vector<std::uint16_t>& semantic_ids = {});

// FNV1a-64 over rgb + quantized depth (mm) + semantic; identical frames hash equal.
[[nodiscard]] std::uint64_t frame_hash(const SensorFrame& frame);

} // namespace agbot::render
