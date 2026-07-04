// Non-GL unit tests for agbot_render: Mat4 math, camera matrices,
// scene file round-trip, and demo scene sanity. No GL context required.

#include "agbot_flight_sim/WeatherPreset.hpp"
#include "agbot_render/Atmosphere.hpp"
#include "agbot_render/Camera.hpp"
#include "agbot_render/DemoScene.hpp"
#include "agbot_render/Mat4.hpp"
#include "agbot_render/OffscreenRenderer.hpp"
#include "agbot_render/RenderScene.hpp"
#include "agbot_render/SceneFile.hpp"

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <string>

namespace {

int g_failures = 0;
int g_checks = 0;

void check(bool condition, const std::string& label) {
    ++g_checks;
    if (!condition) {
        ++g_failures;
        std::fprintf(stderr, "FAIL: %s\n", label.c_str());
    }
}

bool near_eq(float a, float b, float tol = 1e-5F) {
    return std::fabs(a - b) <= tol;
}

constexpr float kPi = 3.14159265358979323846F;

using agbot::render::Camera;
using agbot::render::Mat4;
using agbot::render::RenderMesh;
using agbot::render::RenderScene;
using agbot::render::RenderVertex;
using agbot::render::TextureImage;
using agbot::render::TexturedMesh;
using agbot::render::TexturedVertex;
using agbot::render::Vec3f;

// ---------------------------------------------------------------------------
// Mat4 math
// ---------------------------------------------------------------------------

void test_mat4_identity_multiply() {
    const Mat4 id = agbot::render::mat4_identity();
    Mat4 t = agbot::render::mat4_translate(Vec3f{1.0F, 2.0F, 3.0F});
    const Mat4 left = agbot::render::mat4_multiply(id, t);
    const Mat4 right = agbot::render::mat4_multiply(t, id);
    for (int i = 0; i < 16; ++i) {
        check(near_eq(left.m[static_cast<std::size_t>(i)], t.m[static_cast<std::size_t>(i)]),
              "identity * T == T at " + std::to_string(i));
        check(near_eq(right.m[static_cast<std::size_t>(i)], t.m[static_cast<std::size_t>(i)]),
              "T * identity == T at " + std::to_string(i));
    }

    // Translation composes: T(a) * T(b) == T(a + b).
    const Mat4 t2 = agbot::render::mat4_translate(Vec3f{-4.0F, 0.5F, 10.0F});
    const Mat4 composed = agbot::render::mat4_multiply(t, t2);
    check(near_eq(composed.at(0, 3), -3.0F), "translate compose x");
    check(near_eq(composed.at(1, 3), 2.5F), "translate compose y");
    check(near_eq(composed.at(2, 3), 13.0F), "translate compose z");
}

void test_perspective_hand_values() {
    // fovy = 90 deg, aspect = 1, near = 1, far = 100:
    //   f = 1/tan(45 deg) = 1
    //   m[2][2] = -(100+1)/(100-1) = -101/99
    //   m[2][3] = -(2*100*1)/(100-1) = -200/99
    const Mat4 p = agbot::render::mat4_perspective(kPi * 0.5F, 1.0F, 1.0F, 100.0F);
    check(near_eq(p.at(0, 0), 1.0F), "perspective m00");
    check(near_eq(p.at(1, 1), 1.0F), "perspective m11");
    check(near_eq(p.at(2, 2), -101.0F / 99.0F), "perspective m22");
    check(near_eq(p.at(2, 3), -200.0F / 99.0F), "perspective m23");
    check(near_eq(p.at(3, 2), -1.0F), "perspective m32");
    check(near_eq(p.at(3, 3), 0.0F), "perspective m33");

    // Near plane maps to z = -1, far plane to z = +1.
    const Vec3f near_pt =
        agbot::render::mat4_transform_point(p, Vec3f{0.0F, 0.0F, -1.0F});
    const Vec3f far_pt =
        agbot::render::mat4_transform_point(p, Vec3f{0.0F, 0.0F, -100.0F});
    check(near_eq(near_pt.z, -1.0F, 1e-4F), "perspective near plane -> ndc z = -1");
    check(near_eq(far_pt.z, 1.0F, 1e-4F), "perspective far plane -> ndc z = +1");

    // Aspect scales x only.
    const Mat4 p2 = agbot::render::mat4_perspective(kPi * 0.5F, 2.0F, 1.0F, 100.0F);
    check(near_eq(p2.at(0, 0), 0.5F), "perspective aspect m00");
    check(near_eq(p2.at(1, 1), 1.0F), "perspective aspect m11");
}

void test_view_matrix_orthonormal() {
    const Mat4 v = agbot::render::mat4_look_at(Vec3f{3.0F, 4.0F, 5.0F},
                                               Vec3f{-2.0F, 1.0F, 9.0F},
                                               Vec3f{0.0F, 1.0F, 0.0F});
    // Rotation rows (upper-left 3x3) must be orthonormal.
    for (int row = 0; row < 3; ++row) {
        float len2 = 0.0F;
        for (int col = 0; col < 3; ++col) {
            len2 += v.at(row, col) * v.at(row, col);
        }
        check(near_eq(len2, 1.0F, 1e-4F), "view row " + std::to_string(row) + " unit length");
    }
    for (int r0 = 0; r0 < 3; ++r0) {
        for (int r1 = r0 + 1; r1 < 3; ++r1) {
            float dot = 0.0F;
            for (int col = 0; col < 3; ++col) {
                dot += v.at(r0, col) * v.at(r1, col);
            }
            check(near_eq(dot, 0.0F, 1e-4F),
                  "view rows " + std::to_string(r0) + "," + std::to_string(r1) + " orthogonal");
        }
    }

    // The eye maps to the origin in view space.
    const Vec3f eye_in_view =
        agbot::render::mat4_transform_point(v, Vec3f{3.0F, 4.0F, 5.0F});
    check(near_eq(eye_in_view.x, 0.0F, 1e-4F), "eye -> view origin x");
    check(near_eq(eye_in_view.y, 0.0F, 1e-4F), "eye -> view origin y");
    check(near_eq(eye_in_view.z, 0.0F, 1e-4F), "eye -> view origin z");
}

void test_look_at_known_point() {
    // Camera at origin looking down -Z: view is identity-like.
    const Mat4 v = agbot::render::mat4_look_at(Vec3f{0.0F, 0.0F, 0.0F},
                                               Vec3f{0.0F, 0.0F, -1.0F},
                                               Vec3f{0.0F, 1.0F, 0.0F});
    const Vec3f p = agbot::render::mat4_transform_point(v, Vec3f{1.0F, 2.0F, -5.0F});
    check(near_eq(p.x, 1.0F), "lookAt -Z: x preserved");
    check(near_eq(p.y, 2.0F), "lookAt -Z: y preserved");
    check(near_eq(p.z, -5.0F), "lookAt -Z: z preserved");
}

// ---------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------

void test_camera_mvp_known_point() {
    Camera camera;
    camera.position = Vec3f{0.0F, 0.0F, 0.0F};
    camera.yaw_rad = 0.0F;
    camera.pitch_rad = 0.0F;
    camera.fov_y_deg = 90.0F;
    camera.near_plane = 1.0F;
    camera.far_plane = 100.0F;

    // Point straight ahead (down -Z) must land at NDC center with z in [-1, 1].
    const Mat4 mvp = camera.view_proj_matrix(1.0F);
    const Vec3f center =
        agbot::render::mat4_transform_point(mvp, Vec3f{0.0F, 0.0F, -10.0F});
    check(near_eq(center.x, 0.0F, 1e-4F), "camera MVP: ahead point ndc x = 0");
    check(near_eq(center.y, 0.0F, 1e-4F), "camera MVP: ahead point ndc y = 0");
    check(center.z > -1.0F && center.z < 1.0F, "camera MVP: ahead point ndc z in range");

    // A point up-and-right of the view axis lands in the +x/+y NDC quadrant.
    const Vec3f quadrant =
        agbot::render::mat4_transform_point(mvp, Vec3f{2.0F, 3.0F, -10.0F});
    check(quadrant.x > 0.0F, "camera MVP: right offset -> ndc x > 0");
    check(quadrant.y > 0.0F, "camera MVP: up offset -> ndc y > 0");
    // fov 90, aspect 1: ndc x = wx / -wz = 2/10.
    check(near_eq(quadrant.x, 0.2F, 1e-4F), "camera MVP: ndc x = 0.2");
    check(near_eq(quadrant.y, 0.3F, 1e-4F), "camera MVP: ndc y = 0.3");

    // Yaw 90 deg turns the camera toward +X: a point at +X is now straight ahead.
    camera.yaw_rad = kPi * 0.5F;
    const Mat4 mvp_yaw = camera.view_proj_matrix(1.0F);
    const Vec3f ahead =
        agbot::render::mat4_transform_point(mvp_yaw, Vec3f{10.0F, 0.0F, 0.0F});
    check(near_eq(ahead.x, 0.0F, 1e-4F), "camera yaw 90: +X point centered x");
    check(near_eq(ahead.y, 0.0F, 1e-4F), "camera yaw 90: +X point centered y");
}

void test_camera_axes_orthonormal() {
    Camera camera;
    camera.yaw_rad = 0.7F;
    camera.pitch_rad = -0.4F;
    const Vec3f f = camera.forward();
    const Vec3f r = camera.right();
    const Vec3f u = camera.up();
    check(near_eq(agbot::render::vec3_length(f), 1.0F, 1e-5F), "camera forward unit");
    check(near_eq(agbot::render::vec3_length(r), 1.0F, 1e-5F), "camera right unit");
    check(near_eq(agbot::render::vec3_length(u), 1.0F, 1e-5F), "camera up unit");
    check(near_eq(agbot::render::vec3_dot(f, r), 0.0F, 1e-5F), "forward ⟂ right");
    check(near_eq(agbot::render::vec3_dot(f, u), 0.0F, 1e-5F), "forward ⟂ up");
    check(near_eq(agbot::render::vec3_dot(r, u), 0.0F, 1e-5F), "right ⟂ up");
    check(near_eq(r.y, 0.0F, 1e-5F), "right stays horizontal");
}

// ---------------------------------------------------------------------------
// Scene file round-trip
// ---------------------------------------------------------------------------

RenderScene make_reference_scene() {
    RenderScene scene;

    RenderMesh tri;
    tri.vertices = {
        RenderVertex{0.0F, 0.0F, 0.0F, 0.0F, 1.0F, 0.0F, 1.0F, 0.0F, 0.0F, 1.0F},
        RenderVertex{1.0F, 0.0F, 0.0F, 0.0F, 1.0F, 0.0F, 0.0F, 1.0F, 0.0F, 0.5F},
        RenderVertex{0.0F, 0.0F, 1.0F, 0.0F, 1.0F, 0.0F, 0.0F, 0.0F, 1.0F, 0.25F},
    };
    tri.indices = {0, 1, 2};
    scene.static_meshes.push_back(tri);

    RenderMesh quad;
    quad.vertices = {
        RenderVertex{-1.0F, 2.0F, -1.0F, 0.0F, 1.0F, 0.0F, 0.9F, 0.9F, 0.9F, 1.0F},
        RenderVertex{1.0F, 2.0F, -1.0F, 0.0F, 1.0F, 0.0F, 0.9F, 0.9F, 0.9F, 1.0F},
        RenderVertex{1.0F, 2.0F, 1.0F, 0.0F, 1.0F, 0.0F, 0.9F, 0.9F, 0.9F, 1.0F},
        RenderVertex{-1.0F, 2.0F, 1.0F, 0.0F, 1.0F, 0.0F, 0.9F, 0.9F, 0.9F, 1.0F},
    };
    quad.indices = {0, 1, 2, 0, 2, 3};
    scene.static_meshes.push_back(quad);

    scene.markers.push_back(RenderScene::Marker{5.0F, 6.0F, 7.0F, 1.0F, 0.5F, 0.25F, 2.5F});
    scene.markers.push_back(RenderScene::Marker{-3.0F, 0.0F, 9.0F, 0.1F, 0.9F, 0.4F, 0.75F});

    scene.sun_dir[0] = 0.1F;
    scene.sun_dir[1] = -0.9F;
    scene.sun_dir[2] = 0.4F;
    return scene;
}

TexturedMesh make_reference_textured_mesh() {
    TexturedMesh mesh;
    mesh.vertices = {
        TexturedVertex{0.0F, 0.0F, 0.0F, 0.0F, 1.0F, 0.0F, 0.0F, 0.0F},
        TexturedVertex{4.0F, 0.0F, 0.0F, 0.0F, 1.0F, 0.0F, 1.0F, 0.0F},
        TexturedVertex{4.0F, 0.0F, 4.0F, 0.0F, 1.0F, 0.0F, 1.0F, 1.0F},
        TexturedVertex{0.0F, 0.0F, 4.0F, 0.0F, 1.0F, 0.0F, 0.0F, 1.0F},
    };
    mesh.indices = {0, 1, 2, 0, 2, 3};
    mesh.texture.width = 4;
    mesh.texture.height = 2;
    mesh.texture.rgba.resize(4U * 2U * 4U);
    for (std::size_t i = 0; i < mesh.texture.rgba.size(); ++i) {
        mesh.texture.rgba[i] = static_cast<std::uint8_t>((i * 37U + 11U) & 0xFFU);
    }
    return mesh;
}

// Hand-writes a v1 (AGBSCN01) scene file so the v1-compat read path is
// exercised against the legacy byte layout, independent of write_scene_file.
bool write_v1_scene_file(const std::filesystem::path& path, const RenderScene& scene) {
    std::ofstream out(path, std::ios::binary | std::ios::trunc);
    if (!out.is_open()) {
        return false;
    }
    out.write(agbot::render::kSceneFileMagicV1, sizeof(agbot::render::kSceneFileMagicV1));

    auto put_u32 = [&out](std::uint32_t value) {
        out.write(reinterpret_cast<const char*>(&value), sizeof(value));
    };

    put_u32(static_cast<std::uint32_t>(scene.static_meshes.size()));
    for (const RenderMesh& mesh : scene.static_meshes) {
        put_u32(static_cast<std::uint32_t>(mesh.vertices.size()));
        put_u32(static_cast<std::uint32_t>(mesh.indices.size()));
        out.write(reinterpret_cast<const char*>(mesh.vertices.data()),
                  static_cast<std::streamsize>(mesh.vertices.size() * sizeof(RenderVertex)));
        out.write(reinterpret_cast<const char*>(mesh.indices.data()),
                  static_cast<std::streamsize>(mesh.indices.size() * sizeof(std::uint32_t)));
    }
    put_u32(static_cast<std::uint32_t>(scene.markers.size()));
    out.write(reinterpret_cast<const char*>(scene.markers.data()),
              static_cast<std::streamsize>(scene.markers.size() * sizeof(RenderScene::Marker)));
    out.write(reinterpret_cast<const char*>(scene.sun_dir), sizeof(scene.sun_dir));
    return out.good();
}

void test_scene_file_round_trip() {
    const RenderScene original = make_reference_scene();
    const std::filesystem::path path =
        std::filesystem::temp_directory_path() / "agbot_render_roundtrip.agbscn";

    const auto write_error = agbot::render::write_scene_file(path, original);
    check(!write_error.has_value(),
          "scene write ok" + (write_error ? ": " + write_error->message : std::string()));

    const auto result = agbot::render::read_scene_file(path);
    check(result.ok(), "scene read ok" +
                           (result.error ? ": " + result.error->message : std::string()));
    if (!result.ok()) {
        return;
    }

    const RenderScene& loaded = result.scene;
    check(loaded.static_meshes.size() == original.static_meshes.size(),
          "round-trip mesh count");
    for (std::size_t i = 0;
         i < loaded.static_meshes.size() && i < original.static_meshes.size(); ++i) {
        const RenderMesh& a = original.static_meshes[i];
        const RenderMesh& b = loaded.static_meshes[i];
        check(a.vertices.size() == b.vertices.size(), "round-trip vertex count " + std::to_string(i));
        check(a.indices.size() == b.indices.size(), "round-trip index count " + std::to_string(i));
        if (a.vertices.size() == b.vertices.size() && !a.vertices.empty()) {
            check(std::memcmp(a.vertices.data(), b.vertices.data(),
                              a.vertices.size() * sizeof(RenderVertex)) == 0,
                  "round-trip vertices byte-equal " + std::to_string(i));
        }
        if (a.indices.size() == b.indices.size() && !a.indices.empty()) {
            check(std::memcmp(a.indices.data(), b.indices.data(),
                              a.indices.size() * sizeof(std::uint32_t)) == 0,
                  "round-trip indices byte-equal " + std::to_string(i));
        }
    }

    check(loaded.markers.size() == original.markers.size(), "round-trip marker count");
    if (loaded.markers.size() == original.markers.size() && !loaded.markers.empty()) {
        check(std::memcmp(loaded.markers.data(), original.markers.data(),
                          loaded.markers.size() * sizeof(RenderScene::Marker)) == 0,
              "round-trip markers byte-equal");
    }
    check(std::memcmp(loaded.sun_dir, original.sun_dir, sizeof(loaded.sun_dir)) == 0,
          "round-trip sun dir byte-equal");

    std::filesystem::remove(path);
}

void test_textured_types_packed() {
    check(sizeof(TexturedVertex) == 8 * sizeof(float), "TexturedVertex is 8 packed floats");
    check(sizeof(RenderVertex) == 10 * sizeof(float), "RenderVertex is 10 packed floats");
    const TextureImage empty;
    check(empty.width == 0 && empty.height == 0 && empty.rgba.empty(),
          "TextureImage default-constructs empty");
    const TexturedVertex v;
    check(near_eq(v.ny, 1.0F) && near_eq(v.u, 0.0F) && near_eq(v.v, 0.0F),
          "TexturedVertex defaults (up normal, zero uv)");
}

void test_scene_file_v2_textured_round_trip() {
    RenderScene original = make_reference_scene();
    original.textured_meshes.push_back(make_reference_textured_mesh());
    TexturedMesh second = make_reference_textured_mesh();
    second.texture.width = 2;
    second.texture.height = 2;
    second.texture.rgba.assign(2U * 2U * 4U, 0x5AU);
    original.textured_meshes.push_back(second);

    const std::filesystem::path path =
        std::filesystem::temp_directory_path() / "agbot_render_v2_roundtrip.agbscn";

    const auto write_error = agbot::render::write_scene_file(path, original);
    check(!write_error.has_value(),
          "v2 scene write ok" + (write_error ? ": " + write_error->message : std::string()));

    // File starts with the v2 magic.
    {
        std::FILE* f = std::fopen(path.string().c_str(), "rb");
        char magic[8] = {};
        check(f != nullptr && std::fread(magic, 1, 8, f) == 8, "v2 magic readable");
        if (f != nullptr) {
            std::fclose(f);
        }
        check(std::memcmp(magic, "AGBSCN02", 8) == 0, "written file has AGBSCN02 magic");
    }

    const auto result = agbot::render::read_scene_file(path);
    check(result.ok(), "v2 scene read ok" +
                           (result.error ? ": " + result.error->message : std::string()));
    if (!result.ok()) {
        return;
    }
    const RenderScene& loaded = result.scene;
    check(loaded.static_meshes.size() == original.static_meshes.size(),
          "v2 round-trip static mesh count");
    check(loaded.textured_meshes.size() == original.textured_meshes.size(),
          "v2 round-trip textured mesh count");
    for (std::size_t i = 0;
         i < loaded.textured_meshes.size() && i < original.textured_meshes.size(); ++i) {
        const TexturedMesh& a = original.textured_meshes[i];
        const TexturedMesh& b = loaded.textured_meshes[i];
        check(a.vertices.size() == b.vertices.size(),
              "v2 textured vertex count " + std::to_string(i));
        check(a.indices.size() == b.indices.size(),
              "v2 textured index count " + std::to_string(i));
        check(a.texture.width == b.texture.width && a.texture.height == b.texture.height,
              "v2 texture dims " + std::to_string(i));
        if (a.vertices.size() == b.vertices.size() && !a.vertices.empty()) {
            check(std::memcmp(a.vertices.data(), b.vertices.data(),
                              a.vertices.size() * sizeof(TexturedVertex)) == 0,
                  "v2 textured vertices byte-equal " + std::to_string(i));
        }
        if (a.indices.size() == b.indices.size() && !a.indices.empty()) {
            check(std::memcmp(a.indices.data(), b.indices.data(),
                              a.indices.size() * sizeof(std::uint32_t)) == 0,
                  "v2 textured indices byte-equal " + std::to_string(i));
        }
        check(a.texture.rgba == b.texture.rgba,
              "v2 texture rgba byte-equal " + std::to_string(i));
    }

    std::filesystem::remove(path);
}

void test_scene_file_v1_compat() {
    const RenderScene original = make_reference_scene();
    const std::filesystem::path path =
        std::filesystem::temp_directory_path() / "agbot_render_v1_compat.agbscn";

    check(write_v1_scene_file(path, original), "v1 fixture written");

    const auto result = agbot::render::read_scene_file(path);
    check(result.ok(), "v1 scene read ok" +
                           (result.error ? ": " + result.error->message : std::string()));
    if (!result.ok()) {
        return;
    }
    const RenderScene& loaded = result.scene;
    check(loaded.textured_meshes.empty(), "v1 scene has empty textured_meshes");
    check(loaded.static_meshes.size() == original.static_meshes.size(),
          "v1 compat mesh count");
    check(loaded.markers.size() == original.markers.size(), "v1 compat marker count");
    if (loaded.static_meshes.size() == original.static_meshes.size() &&
        !loaded.static_meshes.empty() &&
        loaded.static_meshes[0].vertices.size() == original.static_meshes[0].vertices.size()) {
        check(std::memcmp(loaded.static_meshes[0].vertices.data(),
                          original.static_meshes[0].vertices.data(),
                          original.static_meshes[0].vertices.size() * sizeof(RenderVertex)) == 0,
              "v1 compat vertices byte-equal");
    }
    check(std::memcmp(loaded.sun_dir, original.sun_dir, sizeof(loaded.sun_dir)) == 0,
          "v1 compat sun dir byte-equal");

    std::filesystem::remove(path);
}

void test_scene_file_bad_magic() {
    const std::filesystem::path path =
        std::filesystem::temp_directory_path() / "agbot_render_badmagic.agbscn";
    {
        std::FILE* f = std::fopen(path.string().c_str(), "wb");
        check(f != nullptr, "bad-magic fixture written");
        if (f != nullptr) {
            std::fputs("NOTASCENE_FILE", f);
            std::fclose(f);
        }
    }
    const auto result = agbot::render::read_scene_file(path);
    check(!result.ok(), "bad magic rejected");
    std::filesystem::remove(path);
}

// ---------------------------------------------------------------------------
// Demo scene sanity
// ---------------------------------------------------------------------------

void test_demo_scene() {
    const RenderScene scene = agbot::render::build_demo_scene();

    check(scene.static_meshes.size() >= 2, "demo scene has heightfield + city meshes");
    check(!scene.markers.empty(), "demo scene has markers");

    std::size_t total_vertices = 0;
    std::size_t total_indices = 0;
    for (const RenderMesh& mesh : scene.static_meshes) {
        check(!mesh.vertices.empty(), "demo mesh has vertices");
        check(!mesh.indices.empty(), "demo mesh has indices");
        check(mesh.indices.size() % 3 == 0, "demo mesh index count divisible by 3");
        total_vertices += mesh.vertices.size();
        total_indices += mesh.indices.size();

        for (std::uint32_t index : mesh.indices) {
            if (index >= mesh.vertices.size()) {
                check(false, "demo mesh index in range");
                break;
            }
        }
        for (const RenderVertex& v : mesh.vertices) {
            const float len = std::sqrt(v.nx * v.nx + v.ny * v.ny + v.nz * v.nz);
            if (!near_eq(len, 1.0F, 1e-3F)) {
                check(false, "demo mesh normal normalized (len=" + std::to_string(len) + ")");
                break;
            }
        }
    }

    // Heightfield is 200x200 vertices; city is ~200 boxes at 24 vertices each.
    check(scene.static_meshes[0].vertices.size() == 200U * 200U,
          "heightfield vertex count 200x200");
    check(scene.static_meshes[1].vertices.size() >= 190U * 24U, "city has ~200 boxes");
    check(total_vertices > 0 && total_indices > 0, "demo scene non-empty totals");

    const float sun_len = std::sqrt(scene.sun_dir[0] * scene.sun_dir[0] +
                                    scene.sun_dir[1] * scene.sun_dir[1] +
                                    scene.sun_dir[2] * scene.sun_dir[2]);
    check(sun_len > 0.1F, "demo sun dir non-degenerate");
    check(scene.sun_dir[1] < 0.0F, "demo sun points downward");

    // Textured pipeline: the demo carries at least one checkerboard mesh with
    // valid UVs in [0, 1] and a non-empty texture.
    check(!scene.textured_meshes.empty(), "demo scene has >= 1 textured mesh");
    for (const TexturedMesh& mesh : scene.textured_meshes) {
        check(!mesh.vertices.empty(), "demo textured mesh has vertices");
        check(!mesh.indices.empty(), "demo textured mesh has indices");
        check(mesh.indices.size() % 3 == 0, "demo textured index count divisible by 3");
        check(mesh.texture.width > 0 && mesh.texture.height > 0,
              "demo textured mesh texture dims > 0");
        check(mesh.texture.rgba.size() == static_cast<std::size_t>(mesh.texture.width) *
                                              static_cast<std::size_t>(mesh.texture.height) * 4U,
              "demo textured rgba payload matches dims");
        for (std::uint32_t index : mesh.indices) {
            if (index >= mesh.vertices.size()) {
                check(false, "demo textured mesh index in range");
                break;
            }
        }
        for (const TexturedVertex& v : mesh.vertices) {
            if (v.u < 0.0F || v.u > 1.0F || v.v < 0.0F || v.v > 1.0F) {
                check(false, "demo textured mesh UVs in [0,1]");
                break;
            }
        }
    }

    // Determinism: two builds are identical.
    const RenderScene again = agbot::render::build_demo_scene();
    check(again.static_meshes.size() == scene.static_meshes.size() &&
              again.static_meshes[0].vertices.size() == scene.static_meshes[0].vertices.size() &&
              std::memcmp(again.static_meshes[0].vertices.data(),
                          scene.static_meshes[0].vertices.data(),
                          scene.static_meshes[0].vertices.size() * sizeof(RenderVertex)) == 0,
          "demo scene deterministic");
    check(again.textured_meshes.size() == scene.textured_meshes.size() &&
              !again.textured_meshes.empty() &&
              again.textured_meshes[0].vertices.size() ==
                  scene.textured_meshes[0].vertices.size() &&
              std::memcmp(again.textured_meshes[0].vertices.data(),
                          scene.textured_meshes[0].vertices.data(),
                          scene.textured_meshes[0].vertices.size() * sizeof(TexturedVertex)) ==
                  0 &&
              again.textured_meshes[0].texture.rgba == scene.textured_meshes[0].texture.rgba,
          "demo textured mesh deterministic");
}

void test_value_noise() {
    // Deterministic, bounded, and continuous-ish.
    const float a = agbot::render::value_noise_2d(1.25F, 3.5F, 42U);
    const float b = agbot::render::value_noise_2d(1.25F, 3.5F, 42U);
    check(near_eq(a, b, 0.0F), "value noise deterministic");
    for (int i = 0; i < 100; ++i) {
        const float v = agbot::render::value_noise_2d(static_cast<float>(i) * 0.37F,
                                                      static_cast<float>(i) * -0.73F, 7U);
        if (v < 0.0F || v > 1.0F) {
            check(false, "value noise in [0,1]");
            break;
        }
    }
    const float c = agbot::render::value_noise_2d(10.0F, 10.0F, 42U);
    const float d = agbot::render::value_noise_2d(10.001F, 10.0F, 42U);
    check(std::fabs(c - d) < 0.05F, "value noise continuous");
}

agbot::render::RenderMesh quad_mesh(float z, float r, float g, float b, float half) {
    agbot::render::RenderMesh mesh;
    const auto vtx = [&](float x, float y) {
        agbot::render::RenderVertex v;
        v.px = x; v.py = y; v.pz = z;
        v.r = r; v.g = g; v.b = b; v.a = 1.0F;
        return v;
    };
    mesh.vertices = {vtx(-half, -half), vtx(half, -half), vtx(half, half), vtx(-half, half)};
    mesh.indices = {0, 1, 2, 0, 2, 3};
    return mesh;
}

void test_offscreen_rasterizer() {
    using namespace agbot::render;
    RenderScene scene;
    scene.static_meshes.push_back(quad_mesh(-10.0F, 1.0F, 0.0F, 0.0F, 2.0F));  // red at 10 m

    OffscreenCamera cam;  // at origin looking down -Z
    const int w = 64;
    const int h = 64;
    const SensorFrame frame = render_offscreen(scene, cam, w, h);

    const std::size_t center = static_cast<std::size_t>(h / 2) * w + (w / 2);
    check(frame.covered_pixels > 0 && frame.coverage_ratio() > 0.05 &&
              frame.coverage_ratio() < 0.9,
          "offscreen: quad covers a plausible central fraction");
    check(std::fabs(frame.depth[center] - 10.0F) < 0.1F, "offscreen: centre linear depth ~10 m");
    check(frame.semantic[center] == 1, "offscreen: centre carries mesh semantic id 1");
    check(frame.rgb[center * 3] > 200 && frame.rgb[center * 3 + 1] < 40,
          "offscreen: centre colour is red");
    // Corner is background sky: no hit, negative depth, semantic 0.
    check(frame.semantic[0] == 0 && frame.depth[0] < 0.0F && frame.rgb[2] == 60,
          "offscreen: corner is background sky");
    // Depth/semantic co-registration: every hit has finite positive depth.
    bool consistent = true;
    for (std::size_t i = 0; i < frame.semantic.size(); ++i) {
        if ((frame.semantic[i] != 0) != (frame.depth[i] > 0.0F)) {
            consistent = false;
            break;
        }
    }
    check(consistent, "offscreen: semantic and depth are co-registered");

    // Determinism: identical scene+camera => identical frame hash.
    const SensorFrame again = render_offscreen(scene, cam, w, h);
    check(frame_hash(frame) == frame_hash(again), "offscreen: frame hash is deterministic");

    // Occlusion: a nearer quad with a different id wins the z-test.
    scene.static_meshes.push_back(quad_mesh(-5.0F, 0.0F, 1.0F, 0.0F, 1.0F));  // green at 5 m
    const SensorFrame occluded = render_offscreen(scene, cam, w, h);
    check(std::fabs(occluded.depth[center] - 5.0F) < 0.1F, "offscreen: nearer quad wins depth");
    check(occluded.semantic[center] == 2, "offscreen: nearer quad wins semantic id");
    check(frame_hash(occluded) != frame_hash(frame), "offscreen: occlusion changes the frame");
}

// --- M7 batch 3: atmosphere & lighting -------------------------------------

void test_sun_direction() {
    // Sun due south (azimuth 180) at 30 deg elevation: points +south is -Z?
    // Repo north is +Z, azimuth clockwise from north, so az=180 => -Z.
    agbot::flight_sim::SolarPosition sp;
    sp.elevation_rad = 30.0 * 3.14159265358979323846 / 180.0;
    sp.azimuth_rad = 3.14159265358979323846; // south
    const Vec3f d = agbot::render::sun_direction(sp);
    check(d.y > 0.4F && d.y < 0.6F, "sun_direction: 30 deg elevation gives y ~ 0.5");
    check(d.z < -0.7F && std::fabs(d.x) < 1e-5F, "sun_direction: south azimuth points -Z");
    const float len = std::sqrt(d.x * d.x + d.y * d.y + d.z * d.z);
    check(std::fabs(len - 1.0F) < 1e-5F, "sun_direction: unit length");
}

void test_preetham_sky() {
    using agbot::render::preetham_sky;
    using agbot::render::Rgb;
    // Sun high in the south-east; a clear daytime sky.
    const Vec3f sun = agbot::render::sun_direction([] {
        agbot::flight_sim::SolarPosition s;
        s.elevation_rad = 1.1; // ~63 deg
        s.azimuth_rad = 2.6;
        return s;
    }());
    const Vec3f zenith{0.0F, 1.0F, 0.0F};
    const Rgb sky = preetham_sky(zenith, sun, 2.5);
    check(sky.b > sky.r && sky.b > 0.0F, "preetham: clear zenith sky is blue-dominant");

    // Brighter looking toward the sun than away from it.
    const Vec3f away{-sun.x, sun.y, -sun.z};
    const float lum_near = preetham_sky(sun, sun, 2.5).g;
    const float lum_away = preetham_sky(away, sun, 2.5).g;
    check(lum_near > lum_away, "preetham: sky is brighter toward the sun");

    // Determinism.
    check(preetham_sky(zenith, sun, 2.5).b == sky.b, "preetham: deterministic");

    // Night (sun below horizon) returns a dim sky.
    const Vec3f night_sun{0.3F, -0.5F, 0.2F};
    const Rgb night = preetham_sky(zenith, night_sun, 2.5);
    check(night.r < 0.1F && night.g < 0.1F && night.b < 0.2F, "preetham: night sky is dim");
}

void test_aerial_perspective() {
    using agbot::render::aerial_perspective;
    using agbot::render::Rgb;
    const Rgb surface{0.2F, 0.5F, 0.2F};
    const Rgb haze{0.7F, 0.75F, 0.8F};
    const Rgb close = aerial_perspective(surface, haze, 0.0, 10000.0);
    check(std::fabs(close.r - surface.r) < 1e-6F, "aerial: distance 0 keeps the surface colour");
    const Rgb mid = aerial_perspective(surface, haze, 5000.0, 10000.0);
    check(mid.r > surface.r && mid.r < haze.r, "aerial: mid distance blends toward haze");
    const Rgb far = aerial_perspective(surface, haze, 60000.0, 10000.0);
    check(std::fabs(far.b - haze.b) < 0.05F, "aerial: far beyond visibility saturates to haze");
}

void test_lighting_from_preset() {
    using agbot::render::lighting_from_preset;
    const auto day = lighting_from_preset(agbot::flight_sim::preset_clear_noon());
    check(day.sun_intensity > 0.5 && !day.artificial_lights_on,
          "lighting: clear noon is bright with lamps off");
    check(day.sun_dir.y > 0.5F, "lighting: noon sun is high");
    const auto night = lighting_from_preset(agbot::flight_sim::preset_clear_night());
    check(night.sun_intensity == 0.0 && night.artificial_lights_on,
          "lighting: clear night is dark with lamps on");
    // Hazier air raises the turbidity proxy.
    const auto hazy = lighting_from_preset(agbot::flight_sim::preset_hazy_afternoon());
    check(hazy.turbidity > day.turbidity, "lighting: hazy air has higher turbidity than clear");
}

void test_night_lights_from_roads() {
    using agbot::render::night_lights_from_roads;
    using agbot::render::NightLightingParams;
    // A 400 m straight road along +X at ground level.
    std::vector<std::vector<Vec3f>> roads = {
        {Vec3f{0.0F, 0.0F, 0.0F}, Vec3f{400.0F, 0.0F, 0.0F}},
        {Vec3f{0.0F, 0.0F, 50.0F}, Vec3f{400.0F, 0.0F, 50.0F}},
    };
    NightLightingParams p;
    p.spacing_m = 40.0;
    p.height_m = 8.0;
    // Road 0 major (importance 1.0), road 1 minor (0.0).
    const auto lights = night_lights_from_roads(roads, {1.0, 0.0}, p);
    check(!lights.empty(), "night lights: lamps placed along roads");
    // All lamps raised to the lamp height.
    bool raised = true;
    for (const auto& l : lights) {
        if (std::fabs(l.position.y - 8.0F) > 1e-4F) {
            raised = false;
        }
    }
    check(raised, "night lights: lamps sit at the configured height");
    // Major road is lit denser than the minor road.
    int major = 0;
    int minor = 0;
    for (const auto& l : lights) {
        if (std::fabs(l.position.z - 0.0F) < 1e-3F) {
            ++major;
        } else {
            ++minor;
        }
    }
    check(major > minor, "night lights: major roads are lit denser than minor roads");
    check(night_lights_from_roads(roads, {1.0, 0.0}, p).size() == lights.size(),
          "night lights: deterministic count");
}

} // namespace

int main() {
    test_mat4_identity_multiply();
    test_perspective_hand_values();
    test_view_matrix_orthonormal();
    test_look_at_known_point();
    test_camera_mvp_known_point();
    test_camera_axes_orthonormal();
    test_scene_file_round_trip();
    test_textured_types_packed();
    test_scene_file_v2_textured_round_trip();
    test_scene_file_v1_compat();
    test_scene_file_bad_magic();
    test_demo_scene();
    test_value_noise();
    test_offscreen_rasterizer();
    test_sun_direction();
    test_preetham_sky();
    test_aerial_perspective();
    test_lighting_from_preset();
    test_night_lights_from_roads();

    if (g_failures == 0) {
        std::printf("agbot_render_tests: all %d checks passed\n", g_checks);
        return 0;
    }
    std::fprintf(stderr, "agbot_render_tests: %d of %d checks FAILED\n", g_failures, g_checks);
    return 1;
}
