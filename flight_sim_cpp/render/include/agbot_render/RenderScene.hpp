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

// Textured vertex: position + normal + UV. Used for meshes that sample a
// per-mesh RGBA texture (e.g. OSM basemap tiles draped over terrain) instead
// of carrying per-vertex color.
struct TexturedVertex {
    float px = 0.0F;
    float py = 0.0F;
    float pz = 0.0F;
    float nx = 0.0F;
    float ny = 1.0F;
    float nz = 0.0F;
    float u = 0.0F;
    float v = 0.0F;
};

static_assert(sizeof(TexturedVertex) == 8 * sizeof(float),
              "TexturedVertex must be tightly packed (8 floats)");

// CPU-side RGBA8 texture image (row-major, tightly packed, width*height*4 bytes).
struct TextureImage {
    int width = 0;
    int height = 0;
    std::vector<std::uint8_t> rgba;
};

struct TexturedMesh {
    std::vector<TexturedVertex> vertices;
    std::vector<std::uint32_t> indices;
    TextureImage texture;
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
    // Optional textured meshes (format v2, AGBSCN02). Empty for v1 scenes.
    std::vector<TexturedMesh> textured_meshes;
};

static_assert(sizeof(RenderScene::Marker) == 7 * sizeof(float),
              "Marker must be tightly packed (7 floats)");

} // namespace agbot::render
