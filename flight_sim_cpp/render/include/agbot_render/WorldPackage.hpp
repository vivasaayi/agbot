#pragma once

#include "agbot_flight_sim/GeoTerrain.hpp"

#include <cstdint>
#include <filesystem>
#include <optional>
#include <string>

namespace agbot::render {

// Resolved simulator handoff from an `.agbworld` manifest to its first tile's
// `.agbscn` payload. The manifest path remains the authoritative package
// identity; consumers load the resolved scene without guessing sibling names.
struct WorldPackage {
    std::filesystem::path manifest_path;
    std::filesystem::path scene_path;
    std::uint64_t world_hash = 0;
    std::string horizontal_crs;
    std::string vertical_datum;
    std::string runtime_frame;
    std::string elevation_state;
    agbot::flight_sim::GeoBounds aoi;
    bool has_aoi = false;
    std::string terrain_source_id;
    std::size_t terrain_cell_count = 0;
    std::size_t terrain_nodata_cells = 0;
};

struct WorldPackageResult {
    WorldPackage package;
    std::optional<std::string> error;

    [[nodiscard]] bool ok() const { return !error.has_value(); }
};

// Reads the deterministic `.agbworld` JSON and resolves its relative scene
// payload. Absolute/traversing scene paths and missing payloads are rejected.
[[nodiscard]] WorldPackageResult read_world_package(
    const std::filesystem::path& manifest_path);

struct WorldTerrainResult {
    agbot::flight_sim::RuntimeTerrain terrain;
    std::optional<std::string> error;

    [[nodiscard]] bool ok() const { return !error.has_value(); }
};

// Loads the grid terrain embedded in an L3 `.agbworld` package and shifts its
// local X/Z frame from the package AOI center to the supplied mission origin.
// Elevation remains in the manifest's declared vertical datum.
[[nodiscard]] WorldTerrainResult load_world_terrain(
    const std::filesystem::path& manifest_path,
    const agbot::flight_sim::GeoCoordinate& runtime_origin);

} // namespace agbot::render
