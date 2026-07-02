#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_terrain/Raster.hpp"
#include "agbot_terrain/Validation.hpp"

#include <filesystem>
#include <string>

namespace agbot::terrain {

struct PipelineResult {
    bool ok = false;
    std::string error;
    HeightField fused;
    ValidationReport validation;
    std::uint64_t param_hash = 0;
};

// Runs the configured estimator layers, fuses per [fusion], validates per
// [validation] (against the referenced layer), and records the config
// param_hash. Config shape matches configs/default_terrain.toml.
[[nodiscard]] PipelineResult run_terrain_pipeline(const agbot::config::ParamTable& config);
[[nodiscard]] PipelineResult run_terrain_pipeline_file(const std::filesystem::path& toml_path);

struct MeshResult {
    bool ok = false;
    std::string error;
    agbot::flight_sim::TerrainMesh mesh;
};

// Bridges a heightfield to the existing renderer mesh builder. Requires a
// square grid of resolution >= 2; nodata cells render at 0 m.
[[nodiscard]] MeshResult mesh_from_heightfield(
    const HeightField& field,
    double vertical_scale = 1.0);

} // namespace agbot::terrain
