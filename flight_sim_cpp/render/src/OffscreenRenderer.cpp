#include "agbot_render/OffscreenRenderer.hpp"

#include <algorithm>
#include <cmath>
#include <limits>

namespace agbot::render {

double SensorFrame::coverage_ratio() const {
    const std::size_t total = static_cast<std::size_t>(width) * static_cast<std::size_t>(height);
    return total == 0 ? 0.0 : static_cast<double>(covered_pixels) / static_cast<double>(total);
}

namespace {

struct ClipVertex {
    Vec4f clip;
    float eye_depth = 0.0F;   // linear metres in front of the camera
    float r = 0.0F;
    float g = 0.0F;
    float b = 0.0F;
};

// Rasterizes one triangle (already in clip space) into the frame buffers.
void raster_triangle(SensorFrame& frame,
                     std::vector<float>& zbuffer,
                     const ClipVertex& a,
                     const ClipVertex& b,
                     const ClipVertex& c,
                     std::uint16_t semantic_id) {
    // Near-plane cull: skip triangles with any vertex at/behind the camera.
    if (a.clip.w <= 1e-6F || b.clip.w <= 1e-6F || c.clip.w <= 1e-6F) {
        return;
    }
    const float w0 = a.clip.w;
    const float w1 = b.clip.w;
    const float w2 = c.clip.w;

    const float sx0 = (a.clip.x / w0 * 0.5F + 0.5F) * static_cast<float>(frame.width);
    const float sy0 = (1.0F - (a.clip.y / w0 * 0.5F + 0.5F)) * static_cast<float>(frame.height);
    const float sx1 = (b.clip.x / w1 * 0.5F + 0.5F) * static_cast<float>(frame.width);
    const float sy1 = (1.0F - (b.clip.y / w1 * 0.5F + 0.5F)) * static_cast<float>(frame.height);
    const float sx2 = (c.clip.x / w2 * 0.5F + 0.5F) * static_cast<float>(frame.width);
    const float sy2 = (1.0F - (c.clip.y / w2 * 0.5F + 0.5F)) * static_cast<float>(frame.height);

    const float area = (sx1 - sx0) * (sy2 - sy0) - (sy1 - sy0) * (sx2 - sx0);
    if (std::fabs(area) < 1e-6F) {
        return;
    }
    const float inv_area = 1.0F / area;

    int min_x = static_cast<int>(std::floor(std::min({sx0, sx1, sx2})));
    int max_x = static_cast<int>(std::ceil(std::max({sx0, sx1, sx2})));
    int min_y = static_cast<int>(std::floor(std::min({sy0, sy1, sy2})));
    int max_y = static_cast<int>(std::ceil(std::max({sy0, sy1, sy2})));
    min_x = std::max(min_x, 0);
    min_y = std::max(min_y, 0);
    max_x = std::min(max_x, frame.width - 1);
    max_y = std::min(max_y, frame.height - 1);

    for (int y = min_y; y <= max_y; ++y) {
        for (int x = min_x; x <= max_x; ++x) {
            const float px = static_cast<float>(x) + 0.5F;
            const float py = static_cast<float>(y) + 0.5F;
            // Barycentric coordinates via edge functions (screen space).
            float l0 = ((sx1 - px) * (sy2 - py) - (sy1 - py) * (sx2 - px)) * inv_area;
            float l1 = ((sx2 - px) * (sy0 - py) - (sy2 - py) * (sx0 - px)) * inv_area;
            float l2 = 1.0F - l0 - l1;
            if (l0 < 0.0F || l1 < 0.0F || l2 < 0.0F) {
                continue;
            }
            // Perspective-correct interpolation weights.
            const float pa = l0 / w0;
            const float pb = l1 / w1;
            const float pc = l2 / w2;
            const float denom = pa + pb + pc;
            if (denom <= 0.0F) {
                continue;
            }
            const float depth = (pa * a.eye_depth + pb * b.eye_depth + pc * c.eye_depth) / denom;
            if (depth <= 0.0F) {
                continue;
            }
            const std::size_t idx =
                static_cast<std::size_t>(y) * static_cast<std::size_t>(frame.width) +
                static_cast<std::size_t>(x);
            if (depth >= zbuffer[idx]) {
                continue;
            }
            zbuffer[idx] = depth;
            const float rr = (pa * a.r + pb * b.r + pc * c.r) / denom;
            const float gg = (pa * a.g + pb * b.g + pc * c.g) / denom;
            const float bb = (pa * a.b + pb * b.b + pc * c.b) / denom;
            const auto to_byte = [](float v) {
                return static_cast<std::uint8_t>(std::clamp(v, 0.0F, 1.0F) * 255.0F + 0.5F);
            };
            frame.rgb[idx * 3 + 0] = to_byte(rr);
            frame.rgb[idx * 3 + 1] = to_byte(gg);
            frame.rgb[idx * 3 + 2] = to_byte(bb);
            frame.depth[idx] = depth;
            frame.semantic[idx] = semantic_id;
        }
    }
}

ClipVertex make_clip_vertex(const Mat4& view, const Mat4& view_proj, const Vec3f& pos,
                            float r, float g, float b) {
    ClipVertex out;
    out.clip = mat4_transform(view_proj, Vec4f{pos.x, pos.y, pos.z, 1.0F});
    const Vec4f eye = mat4_transform(view, Vec4f{pos.x, pos.y, pos.z, 1.0F});
    out.eye_depth = -eye.z;   // camera looks down -Z; in-front points have eye z < 0
    out.r = r;
    out.g = g;
    out.b = b;
    return out;
}

} // namespace

SensorFrame render_offscreen(const RenderScene& scene,
                             const OffscreenCamera& camera,
                             int width,
                             int height,
                             const std::vector<std::uint16_t>& semantic_ids) {
    SensorFrame frame;
    frame.width = std::max(width, 1);
    frame.height = std::max(height, 1);
    const std::size_t cells =
        static_cast<std::size_t>(frame.width) * static_cast<std::size_t>(frame.height);
    // Sky/background clear colour.
    frame.rgb.assign(cells * 3, 0);
    for (std::size_t i = 0; i < cells; ++i) {
        frame.rgb[i * 3 + 0] = 30;
        frame.rgb[i * 3 + 1] = 40;
        frame.rgb[i * 3 + 2] = 60;
    }
    frame.depth.assign(cells, -1.0F);
    frame.semantic.assign(cells, 0);
    std::vector<float> zbuffer(cells, std::numeric_limits<float>::max());

    const float aspect = static_cast<float>(frame.width) / static_cast<float>(frame.height);
    const Mat4 view = mat4_look_at(camera.eye, camera.target, camera.up);
    const Mat4 proj = mat4_perspective(camera.fov_y_rad, aspect, camera.near_m, camera.far_m);
    const Mat4 view_proj = mat4_multiply(proj, view);

    // Textured meshes (flat mid-grey; they carry no per-vertex colour here).
    for (std::size_t mi = 0; mi < scene.textured_meshes.size(); ++mi) {
        const TexturedMesh& mesh = scene.textured_meshes[mi];
        const std::uint16_t sem = static_cast<std::uint16_t>(100 + mi);
        for (std::size_t t = 0; t + 2 < mesh.indices.size(); t += 3) {
            const auto& v0 = mesh.vertices[mesh.indices[t + 0]];
            const auto& v1 = mesh.vertices[mesh.indices[t + 1]];
            const auto& v2 = mesh.vertices[mesh.indices[t + 2]];
            raster_triangle(
                frame, zbuffer,
                make_clip_vertex(view, view_proj, {v0.px, v0.py, v0.pz}, 0.5F, 0.5F, 0.5F),
                make_clip_vertex(view, view_proj, {v1.px, v1.py, v1.pz}, 0.5F, 0.5F, 0.5F),
                make_clip_vertex(view, view_proj, {v2.px, v2.py, v2.pz}, 0.5F, 0.5F, 0.5F),
                sem);
        }
    }

    // Static meshes (per-vertex colour, per-mesh semantic id).
    for (std::size_t mi = 0; mi < scene.static_meshes.size(); ++mi) {
        const RenderMesh& mesh = scene.static_meshes[mi];
        const std::uint16_t sem = mi < semantic_ids.size()
            ? semantic_ids[mi]
            : static_cast<std::uint16_t>(mi + 1);
        for (std::size_t t = 0; t + 2 < mesh.indices.size(); t += 3) {
            const RenderVertex& v0 = mesh.vertices[mesh.indices[t + 0]];
            const RenderVertex& v1 = mesh.vertices[mesh.indices[t + 1]];
            const RenderVertex& v2 = mesh.vertices[mesh.indices[t + 2]];
            raster_triangle(
                frame, zbuffer,
                make_clip_vertex(view, view_proj, {v0.px, v0.py, v0.pz}, v0.r, v0.g, v0.b),
                make_clip_vertex(view, view_proj, {v1.px, v1.py, v1.pz}, v1.r, v1.g, v1.b),
                make_clip_vertex(view, view_proj, {v2.px, v2.py, v2.pz}, v2.r, v2.g, v2.b),
                sem);
        }
    }

    for (const std::uint16_t sem : frame.semantic) {
        if (sem != 0) {
            ++frame.covered_pixels;
        }
    }
    return frame;
}

std::uint64_t frame_hash(const SensorFrame& frame) {
    std::uint64_t acc = 1469598103934665603ULL;
    const auto fold = [&acc](std::uint64_t value) {
        for (int i = 0; i < 8; ++i) {
            acc ^= (value >> (i * 8)) & 0xFFu;
            acc *= 1099511628211ULL;
        }
    };
    fold(static_cast<std::uint64_t>(frame.width));
    fold(static_cast<std::uint64_t>(frame.height));
    for (const std::uint8_t byte : frame.rgb) {
        fold(byte);
    }
    for (const float d : frame.depth) {
        const std::int64_t mm = d < 0.0F ? -1 : static_cast<std::int64_t>(d * 1000.0F + 0.5F);
        fold(static_cast<std::uint64_t>(mm));
    }
    for (const std::uint16_t sem : frame.semantic) {
        fold(sem);
    }
    return acc;
}

} // namespace agbot::render
