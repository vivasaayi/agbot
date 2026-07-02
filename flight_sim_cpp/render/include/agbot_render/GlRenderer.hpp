#pragma once

#include "agbot_render/Renderer.hpp"

#include <cstdint>
#include <string>
#include <vector>

namespace agbot::render {

// OpenGL 4.1 Core Profile renderer: VAOs/VBOs/EBOs, GLSL 410 shaders,
// single directional Blinn-Phong-lite pass, no fixed-function calls.
// All GL types are kept as opaque uint32 handles in the header so that no
// GL header leaks into consumers.
class GlRenderer final : public Renderer {
public:
    GlRenderer() = default;
    ~GlRenderer() override;

    GlRenderer(const GlRenderer&) = delete;
    GlRenderer& operator=(const GlRenderer&) = delete;

    bool init(int width_px, int height_px) override;
    void shutdown() override;
    bool uploadScene(const RenderScene& scene) override;
    void resize(int width_px, int height_px) override;
    void drawFrame(const Camera& camera) override;
    const char* last_error() const override { return last_error_.c_str(); }

    // Sky gradient (fullscreen horizon-to-zenith quad drawn behind the scene).
    // Off by default so the offscreen --self-check keeps a flat clear color and
    // its pixel-diff against the clear color stays meaningful; the windowed
    // viewer turns it on. GlRenderer-specific knob, not part of the RHI.
    void set_sky_enabled(bool enabled) { sky_enabled_ = enabled; }
    bool sky_enabled() const { return sky_enabled_; }

    // Draw-call counters for the most recent drawFrame (self-check evidence).
    int last_untextured_draw_count() const { return last_untextured_draws_; }
    int last_textured_draw_count() const { return last_textured_draws_; }

private:
    struct GpuMesh {
        std::uint32_t vao = 0;
        std::uint32_t vbo = 0;
        std::uint32_t ebo = 0;
        std::int32_t index_count = 0;
    };

    struct GpuTexturedMesh {
        std::uint32_t vao = 0;
        std::uint32_t vbo = 0;
        std::uint32_t ebo = 0;
        std::uint32_t texture = 0;
        std::int32_t index_count = 0;
    };

    bool build_shader_program();
    bool build_textured_shader_program();
    bool build_sky_shader_program();
    void destroy_scene_buffers();

    std::uint32_t program_ = 0;
    std::int32_t loc_mvp_ = -1;
    std::int32_t loc_model_ = -1;
    std::int32_t loc_sun_dir_ = -1;
    std::int32_t loc_view_pos_ = -1;

    std::uint32_t tex_program_ = 0;
    std::int32_t tex_loc_mvp_ = -1;
    std::int32_t tex_loc_model_ = -1;
    std::int32_t tex_loc_sun_dir_ = -1;
    std::int32_t tex_loc_view_pos_ = -1;
    std::int32_t tex_loc_texture_ = -1;

    std::uint32_t sky_program_ = 0;
    std::uint32_t sky_vao_ = 0;
    bool sky_enabled_ = false;

    std::vector<GpuMesh> meshes_;
    std::vector<GpuTexturedMesh> textured_meshes_;
    GpuMesh marker_cube_;
    std::vector<RenderScene::Marker> markers_;
    float sun_dir_[3] = {0.35F, -0.8F, 0.45F};

    int last_untextured_draws_ = 0;
    int last_textured_draws_ = 0;

    int width_px_ = 0;
    int height_px_ = 0;
    bool initialized_ = false;
    std::string last_error_;
};

} // namespace agbot::render
