#pragma once

#include "agbot_render/RenderScene.hpp"

#include <filesystem>
#include <optional>
#include <string>

namespace agbot::render {

// Minimal binary scene format, version 2 "AGBSCN02" (little-endian, native
// IEEE-754 floats). The v2 layout is the full v1 layout with a textured-mesh
// block appended:
//
//   char     magic[8]        "AGBSCN02" (v2) or "AGBSCN01" (v1)
//   uint32   mesh_count
//   per mesh (mesh_count times):
//     uint32 vertex_count
//     uint32 index_count
//     RenderVertex[vertex_count]   (10 floats each: px py pz nx ny nz r g b a)
//     uint32[index_count]
//   uint32   marker_count
//   Marker[marker_count]           (7 floats each: x y z r g b size_m)
//   float    sun_dir[3]
//   --- v2 only (appended; a v1 file ends above) ---
//   uint32   textured_mesh_count
//   per textured mesh (textured_mesh_count times):
//     uint32 vertex_count
//     uint32 index_count
//     uint32 texture_width
//     uint32 texture_height
//     TexturedVertex[vertex_count] (8 floats each: px py pz nx ny nz u v)
//     uint32[index_count]
//     uint8  rgba[texture_width * texture_height * 4]
//
// write_scene_file always writes v2. read_scene_file accepts both v1
// (textured_meshes left empty) and v2.
//
// This is the integration handoff format: other modules write .agbscn files
// which agbot_world_viewer loads via argv[1].

inline constexpr char kSceneFileMagic[8] = {'A', 'G', 'B', 'S', 'C', 'N', '0', '2'};
inline constexpr char kSceneFileMagicV1[8] = {'A', 'G', 'B', 'S', 'C', 'N', '0', '1'};

struct SceneFileError {
    std::string message;
};

// Returns std::nullopt on success, otherwise a description of the failure.
std::optional<SceneFileError> write_scene_file(const std::filesystem::path& path,
                                               const RenderScene& scene);

struct SceneFileResult {
    RenderScene scene;
    std::optional<SceneFileError> error;

    bool ok() const { return !error.has_value(); }
};

SceneFileResult read_scene_file(const std::filesystem::path& path);

} // namespace agbot::render
