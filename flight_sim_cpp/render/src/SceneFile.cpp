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
    if (!in.good() || std::memcmp(magic, kSceneFileMagic, sizeof(magic)) != 0) {
        result.error = SceneFileError{"bad magic (expected AGBSCN01): " + path.string()};
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
    return result;
}

} // namespace agbot::render
