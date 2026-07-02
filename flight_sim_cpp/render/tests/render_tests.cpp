// Non-GL unit tests for agbot_render: Mat4 math, camera matrices,
// scene file round-trip, and demo scene sanity. No GL context required.

#include "agbot_render/Camera.hpp"
#include "agbot_render/DemoScene.hpp"
#include "agbot_render/Mat4.hpp"
#include "agbot_render/RenderScene.hpp"
#include "agbot_render/SceneFile.hpp"

#include <cmath>
#include <cstdio>
#include <cstring>
#include <filesystem>
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

    // Determinism: two builds are identical.
    const RenderScene again = agbot::render::build_demo_scene();
    check(again.static_meshes.size() == scene.static_meshes.size() &&
              again.static_meshes[0].vertices.size() == scene.static_meshes[0].vertices.size() &&
              std::memcmp(again.static_meshes[0].vertices.data(),
                          scene.static_meshes[0].vertices.data(),
                          scene.static_meshes[0].vertices.size() * sizeof(RenderVertex)) == 0,
          "demo scene deterministic");
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

} // namespace

int main() {
    test_mat4_identity_multiply();
    test_perspective_hand_values();
    test_view_matrix_orthonormal();
    test_look_at_known_point();
    test_camera_mvp_known_point();
    test_camera_axes_orthonormal();
    test_scene_file_round_trip();
    test_scene_file_bad_magic();
    test_demo_scene();
    test_value_noise();

    if (g_failures == 0) {
        std::printf("agbot_render_tests: all %d checks passed\n", g_checks);
        return 0;
    }
    std::fprintf(stderr, "agbot_render_tests: %d of %d checks FAILED\n", g_failures, g_checks);
    return 1;
}
