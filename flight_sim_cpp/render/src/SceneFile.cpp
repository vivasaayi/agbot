#include "agbot_render/SceneFile.hpp"

#include <cstring>
#include <fstream>
#include <limits>

namespace agbot::render {

namespace {

bool write_u32(std::ofstream& out, std::uint32_t value) {
    out.write(reinterpret_cast<const char*>(&value), sizeof(value));
    return out.good();
}

bool read_u32(std::ifstream& in, std::uint32_t& value) {
    in.read(reinterpret_cast<char*>(&value), sizeof(value));
    return in.good();
}

constexpr std::uint32_t kMaxCount = 64U * 1024U * 1024U; // sanity bound against corrupt files

} // namespace

std::optional<SceneFileError> write_scene_file(const std::filesystem::path& path,
                                               const RenderScene& scene) {
    std::ofstream out(path, std::ios::binary | std::ios::trunc);
    if (!out.is_open()) {
        return SceneFileError{"cannot open scene file for writing: " + path.string()};
    }

    out.write(kSceneFileMagic, sizeof(kSceneFileMagic));

    if (scene.static_meshes.size() > kMaxCount) {
        return SceneFileError{"too many meshes"};
    }
    write_u32(out, static_cast<std::uint32_t>(scene.static_meshes.size()));

    for (const RenderMesh& mesh : scene.static_meshes) {
        if (mesh.vertices.size() > kMaxCount || mesh.indices.size() > kMaxCount) {
            return SceneFileError{"mesh too large for scene file"};
        }
        write_u32(out, static_cast<std::uint32_t>(mesh.vertices.size()));
        write_u32(out, static_cast<std::uint32_t>(mesh.indices.size()));
        if (!mesh.vertices.empty()) {
            out.write(reinterpret_cast<const char*>(mesh.vertices.data()),
                      static_cast<std::streamsize>(mesh.vertices.size() * sizeof(RenderVertex)));
        }
        if (!mesh.indices.empty()) {
            out.write(reinterpret_cast<const char*>(mesh.indices.data()),
                      static_cast<std::streamsize>(mesh.indices.size() * sizeof(std::uint32_t)));
        }
    }

    if (scene.markers.size() > kMaxCount) {
        return SceneFileError{"too many markers"};
    }
    write_u32(out, static_cast<std::uint32_t>(scene.markers.size()));
    if (!scene.markers.empty()) {
        out.write(reinterpret_cast<const char*>(scene.markers.data()),
                  static_cast<std::streamsize>(scene.markers.size() * sizeof(RenderScene::Marker)));
    }

    out.write(reinterpret_cast<const char*>(scene.sun_dir), sizeof(scene.sun_dir));

    // --- v2 block: textured meshes ---
    if (scene.textured_meshes.size() > kMaxCount) {
        return SceneFileError{"too many textured meshes"};
    }
    write_u32(out, static_cast<std::uint32_t>(scene.textured_meshes.size()));
    for (const TexturedMesh& mesh : scene.textured_meshes) {
        if (mesh.vertices.size() > kMaxCount || mesh.indices.size() > kMaxCount) {
            return SceneFileError{"textured mesh too large for scene file"};
        }
        if (mesh.texture.width < 0 || mesh.texture.height < 0 ||
            mesh.texture.rgba.size() != static_cast<std::size_t>(mesh.texture.width) *
                                            static_cast<std::size_t>(mesh.texture.height) * 4U) {
            return SceneFileError{"textured mesh texture size does not match rgba payload"};
        }
        write_u32(out, static_cast<std::uint32_t>(mesh.vertices.size()));
        write_u32(out, static_cast<std::uint32_t>(mesh.indices.size()));
        write_u32(out, static_cast<std::uint32_t>(mesh.texture.width));
        write_u32(out, static_cast<std::uint32_t>(mesh.texture.height));
        if (!mesh.vertices.empty()) {
            out.write(reinterpret_cast<const char*>(mesh.vertices.data()),
                      static_cast<std::streamsize>(mesh.vertices.size() * sizeof(TexturedVertex)));
        }
        if (!mesh.indices.empty()) {
            out.write(reinterpret_cast<const char*>(mesh.indices.data()),
                      static_cast<std::streamsize>(mesh.indices.size() * sizeof(std::uint32_t)));
        }
        if (!mesh.texture.rgba.empty()) {
            out.write(reinterpret_cast<const char*>(mesh.texture.rgba.data()),
                      static_cast<std::streamsize>(mesh.texture.rgba.size()));
        }
    }

    if (!out.good()) {
        return SceneFileError{"write failure on scene file: " + path.string()};
    }
    return std::nullopt;
}

SceneFileResult read_scene_file(const std::filesystem::path& path) {
    SceneFileResult result;

    std::ifstream in(path, std::ios::binary);
    if (!in.is_open()) {
        result.error = SceneFileError{"cannot open scene file: " + path.string()};
        return result;
    }

    char magic[sizeof(kSceneFileMagic)] = {};
    in.read(magic, sizeof(magic));
    const bool is_v2 = in.good() && std::memcmp(magic, kSceneFileMagic, sizeof(magic)) == 0;
    const bool is_v1 = in.good() && std::memcmp(magic, kSceneFileMagicV1, sizeof(magic)) == 0;
    if (!is_v1 && !is_v2) {
        result.error =
            SceneFileError{"bad magic (expected AGBSCN01 or AGBSCN02): " + path.string()};
        return result;
    }

    std::uint32_t mesh_count = 0;
    if (!read_u32(in, mesh_count) || mesh_count > kMaxCount) {
        result.error = SceneFileError{"invalid mesh count"};
        return result;
    }

    result.scene.static_meshes.resize(mesh_count);
    for (std::uint32_t i = 0; i < mesh_count; ++i) {
        std::uint32_t vertex_count = 0;
        std::uint32_t index_count = 0;
        if (!read_u32(in, vertex_count) || !read_u32(in, index_count) ||
            vertex_count > kMaxCount || index_count > kMaxCount) {
            result.error = SceneFileError{"invalid mesh header"};
            return result;
        }
        RenderMesh& mesh = result.scene.static_meshes[i];
        mesh.vertices.resize(vertex_count);
        mesh.indices.resize(index_count);
        if (vertex_count > 0) {
            in.read(reinterpret_cast<char*>(mesh.vertices.data()),
                    static_cast<std::streamsize>(vertex_count * sizeof(RenderVertex)));
        }
        if (index_count > 0) {
            in.read(reinterpret_cast<char*>(mesh.indices.data()),
                    static_cast<std::streamsize>(index_count * sizeof(std::uint32_t)));
        }
        if (!in.good()) {
            result.error = SceneFileError{"truncated mesh data"};
            return result;
        }
    }

    std::uint32_t marker_count = 0;
    if (!read_u32(in, marker_count) || marker_count > kMaxCount) {
        result.error = SceneFileError{"invalid marker count"};
        return result;
    }
    result.scene.markers.resize(marker_count);
    if (marker_count > 0) {
        in.read(reinterpret_cast<char*>(result.scene.markers.data()),
                static_cast<std::streamsize>(marker_count * sizeof(RenderScene::Marker)));
    }

    in.read(reinterpret_cast<char*>(result.scene.sun_dir), sizeof(result.scene.sun_dir));

    if (!in.good()) {
        result.error = SceneFileError{"truncated scene file: " + path.string()};
        return result;
    }

    if (!is_v2) {
        // v1 file ends here; textured_meshes stays empty.
        return result;
    }

    // --- v2 block: textured meshes ---
    std::uint32_t textured_count = 0;
    if (!read_u32(in, textured_count) || textured_count > kMaxCount) {
        result.error = SceneFileError{"invalid textured mesh count"};
        return result;
    }
    result.scene.textured_meshes.resize(textured_count);
    for (std::uint32_t i = 0; i < textured_count; ++i) {
        std::uint32_t vertex_count = 0;
        std::uint32_t index_count = 0;
        std::uint32_t tex_width = 0;
        std::uint32_t tex_height = 0;
        if (!read_u32(in, vertex_count) || !read_u32(in, index_count) ||
            !read_u32(in, tex_width) || !read_u32(in, tex_height) ||
            vertex_count > kMaxCount || index_count > kMaxCount || tex_width > kMaxCount ||
            tex_height > kMaxCount ||
            static_cast<std::uint64_t>(tex_width) * tex_height * 4U > kMaxCount) {
            result.error = SceneFileError{"invalid textured mesh header"};
            return result;
        }
        TexturedMesh& mesh = result.scene.textured_meshes[i];
        mesh.vertices.resize(vertex_count);
        mesh.indices.resize(index_count);
        mesh.texture.width = static_cast<int>(tex_width);
        mesh.texture.height = static_cast<int>(tex_height);
        mesh.texture.rgba.resize(static_cast<std::size_t>(tex_width) * tex_height * 4U);
        if (vertex_count > 0) {
            in.read(reinterpret_cast<char*>(mesh.vertices.data()),
                    static_cast<std::streamsize>(vertex_count * sizeof(TexturedVertex)));
        }
        if (index_count > 0) {
            in.read(reinterpret_cast<char*>(mesh.indices.data()),
                    static_cast<std::streamsize>(index_count * sizeof(std::uint32_t)));
        }
        if (!mesh.texture.rgba.empty()) {
            in.read(reinterpret_cast<char*>(mesh.texture.rgba.data()),
                    static_cast<std::streamsize>(mesh.texture.rgba.size()));
        }
        if (!in.good()) {
            result.error = SceneFileError{"truncated textured mesh data"};
            return result;
        }
    }
    return result;
}

} // namespace agbot::render
