#pragma once

#include "agbot_render/Camera.hpp"
#include "agbot_render/RenderScene.hpp"

namespace agbot::render {

// Small RHI-ish surface: everything graphics-API specific lives behind this
// interface so a bgfx / WebGPU / Vulkan backend can be swapped in later
// without touching scene, camera, or app-loop code.
//
// Contract: a graphics context (e.g. an OpenGL core-profile context) must be
// current on the calling thread before init() and stay current for all other
// calls. Context/window creation is the app layer's job.
class Renderer {
public:
    virtual ~Renderer() = default;

    virtual bool init(int width_px, int height_px) = 0;
    virtual void shutdown() = 0;

    // Uploads the scene to GPU buffers, replacing any previously uploaded scene.
    virtual bool uploadScene(const RenderScene& scene) = 0;

    virtual void resize(int width_px, int height_px) = 0;
    virtual void drawFrame(const Camera& camera) = 0;

    // Last error (shader compile/link log, etc). Empty when healthy.
    virtual const char* last_error() const = 0;
};

} // namespace agbot::render
