#include "agbot_render/DemoScene.hpp"

#include "agbot_render/Mat4.hpp"

#include <cmath>
#include <cstdint>

namespace agbot::render {

namespace {

std::uint32_t hash_2d(std::int32_t ix, std::int32_t iy, std::uint32_t seed) {
    std::uint32_t h = seed;
    h ^= static_cast<std::uint32_t>(ix) * 0x9E3779B9U;
    h ^= static_cast<std::uint32_t>(iy) * 0x85EBCA6BU;
    h ^= h >> 16;
    h *= 0x7FEB352DU;
    h ^= h >> 15;
    h *= 0x846CA68BU;
    h ^= h >> 16;
    return h;
}

float lattice_value(std::int32_t ix, std::int32_t iy, std::uint32_t seed) {
    return static_cast<float>(hash_2d(ix, iy, seed) & 0x00FFFFFFU) /
           static_cast<float>(0x00FFFFFFU);
}

float smoothstep01(float t) {
    return t * t * (3.0F - 2.0F * t);
}

// Heightfield configuration: 200x200 vertices spanning 400m x 400m.
constexpr int kFieldVerts = 200;
constexpr float kFieldExtent = 400.0F;
constexpr float kHeightAmplitude = 14.0F;

float terrain_height(float x, float z) {
    // Two octaves of value noise, deterministic.
    const float n0 = value_noise_2d(x * 0.015F, z * 0.015F, 1337U);
    const float n1 = value_noise_2d(x * 0.06F, z * 0.06F, 7331U);
    return (n0 * 0.8F + n1 * 0.2F) * kHeightAmplitude;
}

RenderMesh build_heightfield_mesh() {
    RenderMesh mesh;
    mesh.vertices.reserve(static_cast<std::size_t>(kFieldVerts) * kFieldVerts);

    const float step = kFieldExtent / static_cast<float>(kFieldVerts - 1);
    const float half = kFieldExtent * 0.5F;

    for (int iz = 0; iz < kFieldVerts; ++iz) {
        for (int ix = 0; ix < kFieldVerts; ++ix) {
            const float x = -half + static_cast<float>(ix) * step;
            const float z = -half + static_cast<float>(iz) * step;
            const float y = terrain_height(x, z);

            // Normal via central differences on the height function.
            const float hl = terrain_height(x - step, z);
            const float hr = terrain_height(x + step, z);
            const float hd = terrain_height(x, z - step);
            const float hu = terrain_height(x, z + step);
            const Vec3f normal = vec3_normalize(Vec3f{hl - hr, 2.0F * step, hd - hu});

            // Color ramp: low = soil brown, high = grass green.
            const float t = y / kHeightAmplitude;
            const float r = 0.35F + 0.10F * (1.0F - t);
            const float g = 0.30F + 0.45F * t;
            const float b = 0.18F + 0.07F * (1.0F - t);

            RenderVertex v;
            v.px = x;
            v.py = y;
            v.pz = z;
            v.nx = normal.x;
            v.ny = normal.y;
            v.nz = normal.z;
            v.r = r;
            v.g = g;
            v.b = b;
            v.a = 1.0F;
            mesh.vertices.push_back(v);
        }
    }

    mesh.indices.reserve(static_cast<std::size_t>(kFieldVerts - 1) * (kFieldVerts - 1) * 6);
    for (int iz = 0; iz + 1 < kFieldVerts; ++iz) {
        for (int ix = 0; ix + 1 < kFieldVerts; ++ix) {
            const std::uint32_t i00 = static_cast<std::uint32_t>(iz * kFieldVerts + ix);
            const std::uint32_t i10 = i00 + 1;
            const std::uint32_t i01 = i00 + static_cast<std::uint32_t>(kFieldVerts);
            const std::uint32_t i11 = i01 + 1;
            mesh.indices.push_back(i00);
            mesh.indices.push_back(i01);
            mesh.indices.push_back(i10);
            mesh.indices.push_back(i10);
            mesh.indices.push_back(i01);
            mesh.indices.push_back(i11);
        }
    }
    return mesh;
}

void append_box(RenderMesh& mesh, float cx, float cz, float base_y, float half_x, float half_z,
                float height, float r, float g, float b) {
    // 6 faces, 4 unique vertices per face so each face carries a flat normal.
    struct Face {
        Vec3f normal;
        Vec3f corners[4];
    };

    const float x0 = cx - half_x;
    const float x1 = cx + half_x;
    const float y0 = base_y;
    const float y1 = base_y + height;
    const float z0 = cz - half_z;
    const float z1 = cz + half_z;

    const Face faces[6] = {
        {{0.0F, 1.0F, 0.0F}, {{x0, y1, z0}, {x0, y1, z1}, {x1, y1, z1}, {x1, y1, z0}}},   // top
        {{0.0F, -1.0F, 0.0F}, {{x0, y0, z0}, {x1, y0, z0}, {x1, y0, z1}, {x0, y0, z1}}},  // bottom
        {{1.0F, 0.0F, 0.0F}, {{x1, y0, z0}, {x1, y1, z0}, {x1, y1, z1}, {x1, y0, z1}}},   // +x
        {{-1.0F, 0.0F, 0.0F}, {{x0, y0, z1}, {x0, y1, z1}, {x0, y1, z0}, {x0, y0, z0}}},  // -x
        {{0.0F, 0.0F, 1.0F}, {{x1, y0, z1}, {x1, y1, z1}, {x0, y1, z1}, {x0, y0, z1}}},   // +z
        {{0.0F, 0.0F, -1.0F}, {{x0, y0, z0}, {x0, y1, z0}, {x1, y1, z0}, {x1, y0, z0}}},  // -z
    };

    for (const Face& face : faces) {
        const std::uint32_t base = static_cast<std::uint32_t>(mesh.vertices.size());
        for (const Vec3f& corner : face.corners) {
            RenderVertex v;
            v.px = corner.x;
            v.py = corner.y;
            v.pz = corner.z;
            v.nx = face.normal.x;
            v.ny = face.normal.y;
            v.nz = face.normal.z;
            v.r = r;
            v.g = g;
            v.b = b;
            v.a = 1.0F;
            mesh.vertices.push_back(v);
        }
        mesh.indices.push_back(base);
        mesh.indices.push_back(base + 1);
        mesh.indices.push_back(base + 2);
        mesh.indices.push_back(base);
        mesh.indices.push_back(base + 2);
        mesh.indices.push_back(base + 3);
    }
}

RenderMesh build_city_mesh() {
    RenderMesh mesh;
    constexpr int kBlocks = 14; // 14 x 14 = 196 boxes (~200)
    constexpr float kSpacing = 18.0F;
    const float origin = -static_cast<float>(kBlocks - 1) * kSpacing * 0.5F;

    for (int iz = 0; iz < kBlocks; ++iz) {
        for (int ix = 0; ix < kBlocks; ++ix) {
            const float cx = origin + static_cast<float>(ix) * kSpacing;
            const float cz = origin + static_cast<float>(iz) * kSpacing;

            const float n = lattice_value(ix, iz, 4242U);
            const float height = 6.0F + n * 34.0F;
            const float half = 3.5F + lattice_value(ix, iz, 9911U) * 3.0F;
            const float base_y = terrain_height(cx, cz);

            const float shade = 0.45F + 0.35F * lattice_value(ix, iz, 555U);
            append_box(mesh, cx, cz, base_y, half, half, height, shade, shade,
                       shade + 0.06F);
        }
    }
    return mesh;
}

} // namespace

float value_noise_2d(float x, float y, std::uint32_t seed) {
    const float fx = std::floor(x);
    const float fy = std::floor(y);
    const std::int32_t ix = static_cast<std::int32_t>(fx);
    const std::int32_t iy = static_cast<std::int32_t>(fy);
    const float tx = smoothstep01(x - fx);
    const float ty = smoothstep01(y - fy);

    const float v00 = lattice_value(ix, iy, seed);
    const float v10 = lattice_value(ix + 1, iy, seed);
    const float v01 = lattice_value(ix, iy + 1, seed);
    const float v11 = lattice_value(ix + 1, iy + 1, seed);

    const float a = v00 + (v10 - v00) * tx;
    const float b = v01 + (v11 - v01) * tx;
    return a + (b - a) * ty;
}

RenderScene build_demo_scene() {
    RenderScene scene;
    scene.static_meshes.push_back(build_heightfield_mesh());
    scene.static_meshes.push_back(build_city_mesh());

    scene.markers.push_back(RenderScene::Marker{0.0F, terrain_height(0.0F, 0.0F) + 30.0F, 0.0F,
                                                1.0F, 0.2F, 0.2F, 2.0F});
    scene.markers.push_back(RenderScene::Marker{60.0F, terrain_height(60.0F, -60.0F) + 45.0F,
                                                -60.0F, 0.2F, 1.0F, 0.3F, 2.0F});
    scene.markers.push_back(RenderScene::Marker{-80.0F, terrain_height(-80.0F, 40.0F) + 50.0F,
                                                40.0F, 0.3F, 0.5F, 1.0F, 2.0F});

    scene.sun_dir[0] = 0.35F;
    scene.sun_dir[1] = -0.80F;
    scene.sun_dir[2] = 0.45F;
    return scene;
}

} // namespace agbot::render
