#pragma once

#include <cstdint>
#include <vector>

namespace agbot::render {

// Generic renderer-facing vertex: position + normal + RGBA color.
// Coordinates are Y-up, local meters. Layout is intentionally plain floats so
// buffers can be uploaded and serialized without conversion.
struct RenderVertex {
    float px = 0.0F;
    float py = 0.0F;
    float pz = 0.0F;
    float nx = 0.0F;
    float ny = 1.0F;
    float nz = 0.0F;
    float r = 1.0F;
    float g = 1.0F;
    float b = 1.0F;
    float a = 1.0F;
};

static_assert(sizeof(RenderVertex) == 10 * sizeof(float),
              "RenderVertex must be tightly packed (10 floats)");

struct RenderMesh {
    std::vector<RenderVertex> vertices;
    std::vector<std::uint32_t> indices;
};

struct RenderScene {
    struct Marker {
        float x = 0.0F;
        float y = 0.0F;
        float z = 0.0F;
        float r = 1.0F;
        float g = 0.0F;
        float b = 0.0F;
        float size_m = 1.0F;
    };

    std::vector<RenderMesh> static_meshes;
    std::vector<Marker> markers;
    float sun_dir[3] = {0.35F, -0.8F, 0.45F};
};

static_assert(sizeof(RenderScene::Marker) == 7 * sizeof(float),
              "Marker must be tightly packed (7 floats)");

} // namespace agbot::render
