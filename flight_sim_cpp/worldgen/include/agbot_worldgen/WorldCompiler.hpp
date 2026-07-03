#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_render/RenderScene.hpp"
#include "agbot_terrain/Raster.hpp"
#include "agbot_worldgen/Feature.hpp"
#include "agbot_worldgen/SceneMesh.hpp"

#include <cstdint>
#include <filesystem>
#include <string>
#include <vector>

namespace agbot::worldgen {

// Version tag stamped into every .agbworld manifest. Bump when the compiler
// changes in a way that legitimately alters output hashes.
inline constexpr const char* kWorldCompilerVersion = "agbworld-1";

// Provenance record for one pinned input dataset. content_hash is an FNV1a-64
// digest of the source file bytes (0 when the source is inline or absent), so a
// later run can prove it consumed the same snapshot.
struct SourceSnapshot {
    std::string source_id;
    std::string uri;
    std::string version;
    std::string license;
    std::string crs = "EPSG:4326";
    std::string vertical_datum;      // "" when not applicable
    std::uint64_t content_hash = 0;
};

// Which world layer a provenance entry describes.
enum class WorldLayerKind { Terrain, Buildings, Roads, Basemap };
[[nodiscard]] const char* to_string(WorldLayerKind kind);

// Per-tile terrain provenance status. Missing elevation is never coerced to
// zero: a tile that lacks authoritative coverage is labelled explicitly.
enum class ElevationState {
    Authoritative,   // compiled from an authoritative DEM (e.g. USGS 3DEP)
    Fallback,        // compiled from a non-authoritative source (e.g. Terrarium)
    MaskedWater,     // intentionally water-masked (reserved for M3 batch 3)
    Missing,         // no usable elevation source
};
[[nodiscard]] const char* to_string(ElevationState state);

// Ties one rendered layer of a tile back to its source dataset and the
// algorithm/params that produced it.
struct LayerProvenance {
    WorldLayerKind kind = WorldLayerKind::Terrain;
    std::string source_id;           // references a SourceSnapshot::source_id
    std::string algorithm_id;        // extractor / pipeline id
    std::uint64_t params_hash = 0;
};

// CRS/datum policy the compile ran under. M1 records the EPSG:4326 + local-ENU
// status quo; M2 populates the canonical metric frame.
struct CrsPolicy {
    std::string horizontal = "EPSG:4326";
    std::string vertical_datum;      // "" until M3 sources carry one
    std::string runtime_frame = "local_enu_m";
};

// One compiled tile of the world. M1 emits a single AOI-wide tile; the field
// shape already supports a tile grid for later milestones.
struct WorldTile {
    std::string tile_id;
    agbot::flight_sim::GeoBounds bounds;
    std::string scene_path;          // relative .agbscn payload ("" until written)
    std::uint64_t content_hash = 0;  // canonical geometry hash for the tile
    std::vector<LayerProvenance> provenance;
    ElevationState elevation_state = ElevationState::Missing;
    // Reason code when elevation is not fully authoritative, e.g.
    // "NO_AUTHORITATIVE_SOURCE", "NODATA_STRIP". Empty when authoritative+complete.
    std::string elevation_fallback_reason;
};

// Aggregate quality metrics carried into the manifest for the validation gates.
struct WorldQuality {
    std::size_t terrain_sample_count = 0;
    double terrain_rmse_m = 0.0;
    double terrain_mae_m = 0.0;
    double terrain_bias_m = 0.0;
    float terrain_min_m = 0.0f;
    float terrain_max_m = 0.0f;
    std::size_t terrain_cell_count = 0;          // total heightfield cells
    std::size_t terrain_authoritative_cells = 0; // cells backed by an authoritative DEM
    std::size_t terrain_nodata_cells = 0;        // cells with no elevation (kept nodata)
    std::size_t building_count = 0;
    double max_building_height_m = 0.0;
    std::size_t city_vertex_count = 0;
    std::size_t city_triangle_count = 0;
    std::size_t city_batch_count = 0;
};

// The .agbworld manifest-of-manifests: everything needed to reproduce and audit
// a compiled world. world_hash is a deterministic digest over the canonical
// body (sources, tiles, policy, seed, geometry hashes).
struct WorldManifest {
    std::string compiler_version;
    std::uint64_t seed = 0;
    agbot::flight_sim::GeoBounds aoi;
    CrsPolicy crs_policy;
    std::vector<SourceSnapshot> sources;
    std::vector<WorldTile> tiles;
    WorldQuality quality;
    std::uint64_t world_hash = 0;

    // Deterministic JSON (fixed key order, fixed float formatting).
    [[nodiscard]] std::string to_json() const;
};

// Inputs to a world compile. Terrain runs from an in-memory TOML config so the
// compile stays hermetic; building/road sources are GeoJSON/Overpass files.
struct WorldCompileSpec {
    agbot::flight_sim::GeoBounds aoi;
    std::uint64_t seed = 0;
    std::string compiler_version = kWorldCompilerVersion;

    std::string terrain_config_toml;                    // required
    std::string terrain_source_id = "terrain";
    std::string terrain_uri;
    std::string terrain_version;
    std::string terrain_license;
    std::string terrain_vertical_datum;
    // True when the terrain base layer is an authoritative DEM (e.g. USGS 3DEP)
    // rather than a fallback source. Drives the tile ElevationState.
    bool terrain_authoritative = false;

    std::string buildings_path;                         // required GeoJSON
    agbot::config::ParamTable building_params;
    std::string buildings_source_id = "buildings";
    std::string buildings_uri;
    std::string buildings_version;
    std::string buildings_license;
    // Native horizontal CRS of the building coordinates ("EPSG:4326",
    // "EPSG:2263", "EPSG:26918"). Projected inputs are normalized to WGS84 at
    // ingest; the native CRS is preserved in source provenance.
    std::string buildings_source_crs = "EPSG:4326";
    // Vertical datum the building base elevations reference (e.g. "NAVD88").
    // Must be compatible with terrain_vertical_datum when both are declared.
    std::string buildings_vertical_datum;

    std::string roads_path;                             // optional
    agbot::config::ParamTable road_params;
    std::string roads_source_id = "roads";
    std::string roads_uri;
    std::string roads_version;
    std::string roads_license;

    // Directory holding cached OSM basemap tiles (out/map_tiles) for draping.
    // When empty or tiles are missing, terrain falls back to a height-colored
    // mesh and no basemap provenance is recorded.
    std::filesystem::path basemap_source_dir;
    std::string basemap_source_id = "basemap";
    std::string basemap_license;

    // Spatial batching size for the city mesh.
    SceneMeshParams mesh_params;
};

// Result of compile_world. Carries the manifest plus the intermediate artifacts
// consumers (viewer/demo/tests) reuse without recompiling.
struct WorldCompileResult {
    bool ok = false;
    std::string error_code;
    std::string error_detail;

    WorldManifest manifest;

    agbot::terrain::HeightField terrain;
    agbot::flight_sim::GeoCoordinate origin;            // AOI center / ENU origin
    std::vector<ExtractedFeature> buildings;
    std::vector<ExtractedFeature> roads;
    CityMesh city;
    agbot::render::RenderScene scene;                   // base world (terrain + city)
    bool terrain_textured = false;
};

// Compiles the AOI into an in-memory world artifact. Pure with respect to the
// compiler's own outputs: it reads source files and runs the terrain +
// extraction + meshing pipeline, computes deterministic content hashes, but
// writes no compiler artifacts. Identical spec + identical source bytes =>
// identical manifest and world_hash.
[[nodiscard]] WorldCompileResult compile_world(const WorldCompileSpec& spec);

// Persists a compiled world: writes <output_dir>/<name>.agbscn (from
// result.scene, so consumer overlays are included) and
// <output_dir>/<name>.agbworld (manifest with tile.scene_path populated).
struct WorldWriteResult {
    bool ok = false;
    std::string error_code;
    std::string error_detail;
    std::filesystem::path scene_path;
    std::filesystem::path manifest_path;
};
[[nodiscard]] WorldWriteResult write_world_artifacts(
    const WorldCompileResult& result,
    const std::filesystem::path& output_dir,
    const std::string& name = "manhattan");

// FNV1a-64 over a file's bytes; returns 0 when the file cannot be read.
[[nodiscard]] std::uint64_t hash_file_bytes(const std::filesystem::path& path);

} // namespace agbot::worldgen
