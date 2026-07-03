#include "agbot_worldgen/WorldCompiler.hpp"

#include "agbot_config/Toml.hpp"
#include "agbot_flight_sim/Mission.hpp"
#include "agbot_worldgen/Crs.hpp"
#include "agbot_render/SceneFile.hpp"
#include "agbot_terrain/Png.hpp"
#include "agbot_terrain/TerrainPipeline.hpp"
#include "agbot_worldgen/extractors/RoadImport.hpp"
#include "agbot_worldgen/extractors/VectorImport.hpp"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iomanip>
#include <limits>
#include <optional>
#include <sstream>

namespace agbot::worldgen {

namespace {

namespace cfg = agbot::config;
namespace fs = agbot::flight_sim;

// ---- FNV1a-64 folding -----------------------------------------------------

constexpr std::uint64_t kFnvOffset = 1469598103934665603ULL;
constexpr std::uint64_t kFnvPrime = 1099511628211ULL;

[[nodiscard]] std::uint64_t fold_bytes(std::uint64_t acc, const void* data, std::size_t len) {
    const auto* bytes = static_cast<const unsigned char*>(data);
    for (std::size_t i = 0; i < len; ++i) {
        acc ^= static_cast<std::uint64_t>(bytes[i]);
        acc *= kFnvPrime;
    }
    return acc;
}

[[nodiscard]] std::uint64_t fold_u64(std::uint64_t acc, std::uint64_t value) {
    return fold_bytes(acc, &value, sizeof(value));
}

[[nodiscard]] std::uint64_t fold_str(std::uint64_t acc, const std::string& value) {
    return fold_bytes(acc, value.data(), value.size());
}

// Fold a double by its IEEE-754 bit pattern (same-platform determinism).
[[nodiscard]] std::uint64_t fold_double(std::uint64_t acc, double value) {
    std::uint64_t bits = 0;
    std::memcpy(&bits, &value, sizeof(bits));
    return fold_u64(acc, bits);
}

[[nodiscard]] std::uint64_t fold_bounds(std::uint64_t acc, const fs::GeoBounds& b) {
    acc = fold_double(acc, b.min_latitude);
    acc = fold_double(acc, b.min_longitude);
    acc = fold_double(acc, b.max_latitude);
    acc = fold_double(acc, b.max_longitude);
    return acc;
}

// Global content hash over the canonical manifest body. Excludes world_hash
// itself and write-time details (tile.scene_path) so it is stable across
// compile-then-write.
[[nodiscard]] std::uint64_t fold_manifest(const WorldManifest& m) {
    std::uint64_t acc = kFnvOffset;
    acc = fold_str(acc, m.compiler_version);
    acc = fold_u64(acc, m.seed);
    acc = fold_bounds(acc, m.aoi);
    acc = fold_str(acc, m.crs_policy.horizontal);
    acc = fold_str(acc, m.crs_policy.vertical_datum);
    acc = fold_str(acc, m.crs_policy.runtime_frame);
    for (const SourceSnapshot& s : m.sources) {
        acc = fold_str(acc, s.source_id);
        acc = fold_str(acc, s.version);
        acc = fold_str(acc, s.license);
        acc = fold_str(acc, s.crs);
        acc = fold_str(acc, s.vertical_datum);
        acc = fold_u64(acc, s.content_hash);
    }
    for (const WorldTile& t : m.tiles) {
        acc = fold_str(acc, t.tile_id);
        acc = fold_bounds(acc, t.bounds);
        acc = fold_u64(acc, t.content_hash);
        for (const LayerProvenance& p : t.provenance) {
            acc = fold_u64(acc, static_cast<std::uint64_t>(p.kind));
            acc = fold_str(acc, p.source_id);
            acc = fold_str(acc, p.algorithm_id);
            acc = fold_u64(acc, p.params_hash);
        }
        acc = fold_u64(acc, static_cast<std::uint64_t>(t.elevation_state));
        acc = fold_str(acc, t.elevation_fallback_reason);
    }
    return acc;
}

// ---- Deterministic JSON helpers -------------------------------------------

[[nodiscard]] std::string escape_json(const std::string& value) {
    std::string out;
    out.reserve(value.size() + 2);
    for (const char c : value) {
        switch (c) {
            case '"': out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\n': out += "\\n"; break;
            case '\r': out += "\\r"; break;
            case '\t': out += "\\t"; break;
            default: out += c; break;
        }
    }
    return out;
}

[[nodiscard]] std::string fmt_double(double value, int precision) {
    std::ostringstream stream;
    stream << std::fixed << std::setprecision(precision) << value;
    return stream.str();
}

// ---- Scene assembly (moved out of the demo main) --------------------------

void terrain_color(float elevation_m, float& r, float& g, float& b) {
    const float t = std::clamp(elevation_m / 60.0f, 0.0f, 1.0f);
    r = 0.30f + 0.35f * t;
    g = 0.46f + 0.16f * t;
    b = 0.26f + 0.10f * t;
    if (elevation_m < 0.5f) {
        r = 0.22f;
        g = 0.34f;
        b = 0.42f;
    }
}

agbot::render::RenderMesh terrain_render_mesh(const agbot::terrain::HeightField& field,
                                              const fs::GeoCoordinate& origin) {
    const agbot::terrain::Raster& elevation = field.elevation;
    agbot::render::RenderMesh mesh;
    const int width = elevation.width;
    const int height = elevation.height;
    mesh.vertices.reserve(static_cast<std::size_t>(width) * static_cast<std::size_t>(height));

    const double lat_span = elevation.bounds.max_latitude - elevation.bounds.min_latitude;
    const double lon_span = elevation.bounds.max_longitude - elevation.bounds.min_longitude;

    auto elevation_at = [&](int row, int col) -> float {
        const float value = elevation.at(std::clamp(row, 0, height - 1),
                                         std::clamp(col, 0, width - 1));
        return agbot::terrain::Raster::is_nodata(value) ? 0.0f : value;
    };

    for (int row = 0; row < height; ++row) {
        const double latitude = elevation.bounds.max_latitude -
            lat_span * static_cast<double>(row) / static_cast<double>(height - 1);
        for (int col = 0; col < width; ++col) {
            const double longitude = elevation.bounds.min_longitude +
                lon_span * static_cast<double>(col) / static_cast<double>(width - 1);
            const float elev = elevation_at(row, col);
            const fs::Vec3 local =
                fs::local_from_geo({latitude, longitude, static_cast<double>(elev)}, origin);

            const fs::Vec3 east_step = fs::local_from_geo(
                {latitude, longitude + lon_span / (width - 1), 0.0}, origin);
            const fs::Vec3 north_step = fs::local_from_geo(
                {latitude + lat_span / (height - 1), longitude, 0.0}, origin);
            const float dx = static_cast<float>(east_step.x - local.x);
            const float dz = static_cast<float>(north_step.z - local.z);
            const float dedx = (elevation_at(row, col + 1) - elevation_at(row, col - 1)) /
                (2.0f * std::max(dx, 1.0f));
            const float dedz = (elevation_at(row - 1, col) - elevation_at(row + 1, col)) /
                (2.0f * std::max(dz, 1.0f));
            float nx = -dedx;
            float ny = 1.0f;
            float nz = -dedz;
            const float norm = std::sqrt(nx * nx + ny * ny + nz * nz);
            nx /= norm;
            ny /= norm;
            nz /= norm;

            agbot::render::RenderVertex vertex;
            vertex.px = static_cast<float>(local.x);
            vertex.py = elev;
            vertex.pz = static_cast<float>(local.z);
            vertex.nx = nx;
            vertex.ny = ny;
            vertex.nz = nz;
            terrain_color(elev, vertex.r, vertex.g, vertex.b);
            vertex.a = 1.0f;
            mesh.vertices.push_back(vertex);
        }
    }

    for (int row = 0; row + 1 < height; ++row) {
        for (int col = 0; col + 1 < width; ++col) {
            const std::uint32_t i00 = static_cast<std::uint32_t>(row * width + col);
            const std::uint32_t i01 = i00 + 1;
            const std::uint32_t i10 = i00 + static_cast<std::uint32_t>(width);
            const std::uint32_t i11 = i10 + 1;
            mesh.indices.insert(mesh.indices.end(), {i00, i11, i10, i00, i01, i11});
        }
    }
    return mesh;
}

void mercator_tile_fraction(double latitude, double longitude, int zoom,
                            double& x_fraction, double& y_fraction) {
    const double n = static_cast<double>(1 << zoom);
    x_fraction = (longitude + 180.0) / 360.0 * n;
    const double lat_rad = latitude * 3.14159265358979323846 / 180.0;
    y_fraction = (1.0 - std::log(std::tan(lat_rad) + 1.0 / std::cos(lat_rad)) /
                            3.14159265358979323846) /
        2.0 * n;
}

// Drape the cached OSM basemap (under <base_dir>/out/map_tiles) over the
// heightfield. Returns nullopt (caller falls back to the height-colored mesh)
// when any covering tile is missing from the on-disk cache.
std::optional<agbot::render::TexturedMesh> textured_terrain_mesh(
    const agbot::terrain::HeightField& field,
    const fs::GeoCoordinate& origin,
    const std::filesystem::path& base_dir) {
    constexpr int kZoom = 15;
    constexpr int kTilePx = 256;
    const agbot::terrain::Raster& elevation = field.elevation;

    const std::vector<fs::TileCoordinate> tiles =
        fs::tiles_for_bounds(elevation.bounds, kZoom);
    if (tiles.empty()) {
        return std::nullopt;
    }
    int min_x = tiles.front().x;
    int max_x = tiles.front().x;
    int min_y = tiles.front().y;
    int max_y = tiles.front().y;
    for (const fs::TileCoordinate& tile : tiles) {
        min_x = std::min(min_x, tile.x);
        max_x = std::max(max_x, tile.x);
        min_y = std::min(min_y, tile.y);
        max_y = std::max(max_y, tile.y);
    }
    const int tiles_x = max_x - min_x + 1;
    const int tiles_y = max_y - min_y + 1;

    agbot::render::TextureImage texture;
    texture.width = tiles_x * kTilePx;
    texture.height = tiles_y * kTilePx;
    texture.rgba.assign(
        static_cast<std::size_t>(texture.width) * texture.height * 4, 0);
    for (int tile_y = min_y; tile_y <= max_y; ++tile_y) {
        for (int tile_x = min_x; tile_x <= max_x; ++tile_x) {
            const std::filesystem::path tile_path = base_dir / "out/map_tiles" /
                std::to_string(kZoom) / std::to_string(tile_x) /
                (std::to_string(tile_y) + ".png");
            const agbot::terrain::PngImage tile =
                agbot::terrain::decode_png_rgba_file(tile_path);
            if (!tile.ok || tile.width != kTilePx || tile.height != kTilePx) {
                return std::nullopt;
            }
            const int dest_x0 = (tile_x - min_x) * kTilePx;
            const int dest_y0 = (tile_y - min_y) * kTilePx;
            for (int row = 0; row < kTilePx; ++row) {
                const std::size_t dest_offset =
                    (static_cast<std::size_t>(dest_y0 + row) * texture.width + dest_x0) * 4;
                const std::size_t src_offset =
                    static_cast<std::size_t>(row) * kTilePx * 4;
                std::copy_n(tile.rgba.begin() + static_cast<std::ptrdiff_t>(src_offset),
                            static_cast<std::size_t>(kTilePx) * 4,
                            texture.rgba.begin() + static_cast<std::ptrdiff_t>(dest_offset));
            }
        }
    }

    agbot::render::TexturedMesh mesh;
    mesh.texture = std::move(texture);
    const int width = elevation.width;
    const int height = elevation.height;
    const double lat_span = elevation.bounds.max_latitude - elevation.bounds.min_latitude;
    const double lon_span = elevation.bounds.max_longitude - elevation.bounds.min_longitude;
    mesh.vertices.reserve(static_cast<std::size_t>(width) * height);

    auto elevation_at = [&](int row, int col) -> float {
        const float value = elevation.at(std::clamp(row, 0, height - 1),
                                         std::clamp(col, 0, width - 1));
        return agbot::terrain::Raster::is_nodata(value) ? 0.0f : value;
    };

    for (int row = 0; row < height; ++row) {
        const double latitude = elevation.bounds.max_latitude -
            lat_span * static_cast<double>(row) / static_cast<double>(height - 1);
        for (int col = 0; col < width; ++col) {
            const double longitude = elevation.bounds.min_longitude +
                lon_span * static_cast<double>(col) / static_cast<double>(width - 1);
            const float elev = elevation_at(row, col);
            const fs::Vec3 local =
                fs::local_from_geo({latitude, longitude, static_cast<double>(elev)}, origin);

            double x_fraction = 0.0;
            double y_fraction = 0.0;
            mercator_tile_fraction(latitude, longitude, kZoom, x_fraction, y_fraction);

            agbot::render::TexturedVertex vertex;
            vertex.px = static_cast<float>(local.x);
            vertex.py = elev;
            vertex.pz = static_cast<float>(local.z);
            vertex.nx = 0.0f;
            vertex.ny = 1.0f;
            vertex.nz = 0.0f;
            vertex.u = static_cast<float>((x_fraction - min_x) / tiles_x);
            vertex.v = static_cast<float>((y_fraction - min_y) / tiles_y);
            mesh.vertices.push_back(vertex);
        }
    }
    for (int row = 0; row + 1 < height; ++row) {
        for (int col = 0; col + 1 < width; ++col) {
            const std::uint32_t i00 = static_cast<std::uint32_t>(row * width + col);
            const std::uint32_t i01 = i00 + 1;
            const std::uint32_t i10 = i00 + static_cast<std::uint32_t>(width);
            const std::uint32_t i11 = i10 + 1;
            mesh.indices.insert(mesh.indices.end(), {i00, i11, i10, i00, i01, i11});
        }
    }
    return mesh;
}

agbot::render::RenderMesh city_render_mesh(const CityMesh& city) {
    agbot::render::RenderMesh mesh;
    mesh.vertices.reserve(city.vertices.size());
    for (const CityVertex& vertex : city.vertices) {
        agbot::render::RenderVertex out;
        out.px = vertex.position[0];
        out.py = vertex.position[1];
        out.pz = vertex.position[2];
        out.nx = vertex.normal[0];
        out.ny = vertex.normal[1];
        out.nz = vertex.normal[2];
        const float variation =
            static_cast<float>((vertex.object_ordinal * 2654435761u) % 1000u) / 1000.0f;
        const float base = 0.62f + 0.24f * variation;
        out.r = base;
        out.g = base;
        out.b = std::min(1.0f, base + 0.05f);
        out.a = 1.0f;
        mesh.vertices.push_back(out);
    }
    mesh.indices = city.indices;
    return mesh;
}

} // namespace

// ---- Public API -----------------------------------------------------------

const char* to_string(WorldLayerKind kind) {
    switch (kind) {
        case WorldLayerKind::Terrain: return "terrain";
        case WorldLayerKind::Buildings: return "buildings";
        case WorldLayerKind::Roads: return "roads";
        case WorldLayerKind::Basemap: return "basemap";
    }
    return "unknown";
}

const char* to_string(ElevationState state) {
    switch (state) {
        case ElevationState::Authoritative: return "authoritative";
        case ElevationState::Fallback: return "fallback";
        case ElevationState::MaskedWater: return "masked_water";
        case ElevationState::Missing: return "missing";
    }
    return "missing";
}

std::uint64_t hash_file_bytes(const std::filesystem::path& path) {
    std::ifstream in(path, std::ios::binary);
    if (!in) {
        return 0;
    }
    std::uint64_t acc = kFnvOffset;
    char buffer[65536];
    while (in) {
        in.read(buffer, sizeof(buffer));
        const std::streamsize got = in.gcount();
        if (got > 0) {
            acc = fold_bytes(acc, buffer, static_cast<std::size_t>(got));
        }
    }
    return acc;
}

std::string WorldManifest::to_json() const {
    std::ostringstream out;
    out << "{\n";
    out << "  \"compiler_version\": \"" << escape_json(compiler_version) << "\",\n";
    out << "  \"seed\": " << seed << ",\n";
    out << "  \"aoi\": {\"min_lat\": " << fmt_double(aoi.min_latitude, 6)
        << ", \"min_lon\": " << fmt_double(aoi.min_longitude, 6)
        << ", \"max_lat\": " << fmt_double(aoi.max_latitude, 6)
        << ", \"max_lon\": " << fmt_double(aoi.max_longitude, 6) << "},\n";
    out << "  \"crs_policy\": {\"horizontal\": \"" << escape_json(crs_policy.horizontal)
        << "\", \"vertical_datum\": \"" << escape_json(crs_policy.vertical_datum)
        << "\", \"runtime_frame\": \"" << escape_json(crs_policy.runtime_frame) << "\"},\n";

    out << "  \"sources\": [";
    for (std::size_t i = 0; i < sources.size(); ++i) {
        const SourceSnapshot& s = sources[i];
        out << (i == 0 ? "\n" : ",\n");
        out << "    {\"source_id\": \"" << escape_json(s.source_id)
            << "\", \"uri\": \"" << escape_json(s.uri)
            << "\", \"version\": \"" << escape_json(s.version)
            << "\", \"license\": \"" << escape_json(s.license)
            << "\", \"crs\": \"" << escape_json(s.crs)
            << "\", \"vertical_datum\": \"" << escape_json(s.vertical_datum)
            << "\", \"content_hash\": " << s.content_hash << "}";
    }
    out << (sources.empty() ? "" : "\n  ") << "],\n";

    out << "  \"tiles\": [";
    for (std::size_t i = 0; i < tiles.size(); ++i) {
        const WorldTile& t = tiles[i];
        out << (i == 0 ? "\n" : ",\n");
        out << "    {\"tile_id\": \"" << escape_json(t.tile_id)
            << "\", \"bounds\": {\"min_lat\": " << fmt_double(t.bounds.min_latitude, 6)
            << ", \"min_lon\": " << fmt_double(t.bounds.min_longitude, 6)
            << ", \"max_lat\": " << fmt_double(t.bounds.max_latitude, 6)
            << ", \"max_lon\": " << fmt_double(t.bounds.max_longitude, 6) << "}"
            << ", \"scene_path\": \"" << escape_json(t.scene_path)
            << "\", \"content_hash\": " << t.content_hash << ", \"provenance\": [";
        for (std::size_t j = 0; j < t.provenance.size(); ++j) {
            const LayerProvenance& p = t.provenance[j];
            out << (j == 0 ? "" : ", ");
            out << "{\"kind\": \"" << to_string(p.kind)
                << "\", \"source_id\": \"" << escape_json(p.source_id)
                << "\", \"algorithm_id\": \"" << escape_json(p.algorithm_id)
                << "\", \"params_hash\": " << p.params_hash << "}";
        }
        out << "], \"elevation_state\": \"" << to_string(t.elevation_state)
            << "\", \"elevation_fallback_reason\": \"" << escape_json(t.elevation_fallback_reason)
            << "\"}";
    }
    out << (tiles.empty() ? "" : "\n  ") << "],\n";

    out << "  \"quality\": {\"terrain_sample_count\": " << quality.terrain_sample_count
        << ", \"terrain_rmse_m\": " << fmt_double(quality.terrain_rmse_m, 4)
        << ", \"terrain_mae_m\": " << fmt_double(quality.terrain_mae_m, 4)
        << ", \"terrain_bias_m\": " << fmt_double(quality.terrain_bias_m, 4)
        << ", \"terrain_min_m\": " << fmt_double(quality.terrain_min_m, 3)
        << ", \"terrain_max_m\": " << fmt_double(quality.terrain_max_m, 3)
        << ", \"terrain_cell_count\": " << quality.terrain_cell_count
        << ", \"terrain_authoritative_cells\": " << quality.terrain_authoritative_cells
        << ", \"terrain_nodata_cells\": " << quality.terrain_nodata_cells
        << ", \"building_count\": " << quality.building_count
        << ", \"max_building_height_m\": " << fmt_double(quality.max_building_height_m, 3)
        << ", \"city_vertex_count\": " << quality.city_vertex_count
        << ", \"city_triangle_count\": " << quality.city_triangle_count
        << ", \"city_batch_count\": " << quality.city_batch_count << "},\n";

    out << "  \"world_hash\": " << world_hash << "\n";
    out << "}\n";
    return out.str();
}

WorldCompileResult compile_world(const WorldCompileSpec& spec) {
    WorldCompileResult result;
    result.origin = spec.aoi.center();

    // 1. Terrain -------------------------------------------------------------
    const cfg::TomlParseResult terrain_config = cfg::parse_toml(spec.terrain_config_toml);
    if (!terrain_config.ok) {
        result.error_code = "terrain_config_parse_failed";
        result.error_detail = terrain_config.error;
        return result;
    }
    const agbot::terrain::PipelineResult terrain =
        agbot::terrain::run_terrain_pipeline(terrain_config.root);
    if (!terrain.ok) {
        result.error_code = "terrain_pipeline_failed";
        result.error_detail = terrain.error;
        return result;
    }
    result.terrain = terrain.fused;
    const fs::GeoBounds aoi = terrain.fused.elevation.bounds;
    result.origin = aoi.center();

    // 2. Buildings (required) ------------------------------------------------
    if (spec.buildings_path.empty() || !std::filesystem::exists(spec.buildings_path)) {
        result.error_code = "buildings_file_missing";
        result.error_detail = spec.buildings_path;
        return result;
    }
    cfg::ParamTable building_params = spec.building_params;
    building_params["path"] = cfg::ParamValue(spec.buildings_path);
    building_params["source_crs"] = cfg::ParamValue(spec.buildings_source_crs);
    const VectorImportExtractor building_extractor;
    const ExtractionResult buildings = building_extractor.extract({aoi, building_params});
    if (!buildings.ok) {
        result.error_code = "building_extraction_failed:" + buildings.error_code;
        result.error_detail = buildings.error_detail;
        return result;
    }
    result.buildings = buildings.features;

    // Datum discipline: when the buildings contribute base elevations, their
    // vertical datum must be compatible with the terrain's. Reject silent
    // mixing of orthometric and ellipsoidal heights.
    const VerticalDatum terrain_datum = vertical_datum_from_name(spec.terrain_vertical_datum);
    const VerticalDatum building_datum = vertical_datum_from_name(spec.buildings_vertical_datum);
    const bool buildings_carry_z = std::any_of(
        result.buildings.begin(), result.buildings.end(),
        [](const ExtractedFeature& feature) { return feature.base_elev_m.has_value(); });
    if (buildings_carry_z && !vertical_datums_compatible(terrain_datum, building_datum)) {
        result.error_code = "mixed_vertical_datum";
        result.error_detail = std::string("terrain=") + to_string(terrain_datum) +
            " buildings=" + to_string(building_datum);
        return result;
    }

    // 3. City mesh -----------------------------------------------------------
    result.city = build_city_mesh(buildings.features, result.origin, spec.mesh_params);

    // 4. Roads (optional) ----------------------------------------------------
    ExtractionResult roads;
    bool have_roads = false;
    if (!spec.roads_path.empty() && std::filesystem::exists(spec.roads_path)) {
        cfg::ParamTable road_params = spec.road_params;
        road_params["path"] = cfg::ParamValue(spec.roads_path);
        const RoadImportExtractor road_extractor;
        roads = road_extractor.extract({aoi, road_params});
        if (roads.ok) {
            result.roads = roads.features;
            have_roads = true;
        }
    }

    // 5. Scene assembly ------------------------------------------------------
    std::optional<agbot::render::TexturedMesh> draped;
    if (!spec.basemap_source_dir.empty()) {
        draped = textured_terrain_mesh(terrain.fused, result.origin, spec.basemap_source_dir);
    }
    result.terrain_textured = draped.has_value();
    if (result.terrain_textured) {
        result.scene.textured_meshes.push_back(std::move(*draped));
    } else {
        result.scene.static_meshes.push_back(terrain_render_mesh(terrain.fused, result.origin));
    }
    result.scene.static_meshes.push_back(city_render_mesh(result.city));
    result.scene.sun_dir[0] = 0.4f;
    result.scene.sun_dir[1] = -0.75f;
    result.scene.sun_dir[2] = 0.53f;

    // 6. Quality metrics -----------------------------------------------------
    WorldManifest manifest;
    manifest.quality.terrain_sample_count = terrain.validation.metrics.sample_count;
    manifest.quality.terrain_rmse_m = terrain.validation.metrics.rmse;
    manifest.quality.terrain_mae_m = terrain.validation.metrics.mae;
    manifest.quality.terrain_bias_m = terrain.validation.metrics.bias;
    float terrain_min = std::numeric_limits<float>::max();
    float terrain_max = std::numeric_limits<float>::lowest();
    std::size_t nodata_cells = 0;
    for (const float value : terrain.fused.elevation.values) {
        if (agbot::terrain::Raster::is_nodata(value)) {
            ++nodata_cells;
        } else {
            terrain_min = std::min(terrain_min, value);
            terrain_max = std::max(terrain_max, value);
        }
    }
    if (terrain_min <= terrain_max) {
        manifest.quality.terrain_min_m = terrain_min;
        manifest.quality.terrain_max_m = terrain_max;
    }
    const std::size_t terrain_cells = terrain.fused.elevation.values.size();
    const std::size_t data_cells = terrain_cells - nodata_cells;
    manifest.quality.terrain_cell_count = terrain_cells;
    manifest.quality.terrain_nodata_cells = nodata_cells;
    manifest.quality.terrain_authoritative_cells = spec.terrain_authoritative ? data_cells : 0;

    // No-silent-zero: classify the tile's elevation provenance explicitly.
    ElevationState elevation_state = ElevationState::Missing;
    std::string elevation_reason;
    if (!terrain.fused.elevation.valid() || data_cells == 0) {
        elevation_state = ElevationState::Missing;
        elevation_reason = "NO_TERRAIN";
    } else if (spec.terrain_authoritative) {
        elevation_state = ElevationState::Authoritative;
        if (nodata_cells > 0) {
            elevation_reason = "NODATA_STRIP";
        }
    } else {
        elevation_state = ElevationState::Fallback;
        elevation_reason = "NO_AUTHORITATIVE_SOURCE";
    }
    manifest.quality.building_count = result.buildings.size();
    for (const ExtractedFeature& feature : result.buildings) {
        if (feature.height_m.has_value()) {
            manifest.quality.max_building_height_m =
                std::max(manifest.quality.max_building_height_m, *feature.height_m);
        }
    }
    manifest.quality.city_vertex_count = result.city.vertices.size();
    manifest.quality.city_triangle_count = result.city.indices.size() / 3;
    manifest.quality.city_batch_count = result.city.batches.size();

    // 7. Sources (provenance) ------------------------------------------------
    SourceSnapshot terrain_source;
    terrain_source.source_id = spec.terrain_source_id;
    terrain_source.uri = spec.terrain_uri;
    terrain_source.version = spec.terrain_version;
    terrain_source.license = spec.terrain_license;
    terrain_source.crs = "EPSG:4326";
    terrain_source.vertical_datum =
        terrain_datum == VerticalDatum::Unknown ? "" : to_string(terrain_datum);
    terrain_source.content_hash = fold_str(kFnvOffset, spec.terrain_config_toml);
    manifest.sources.push_back(terrain_source);

    SourceSnapshot building_source;
    building_source.source_id = spec.buildings_source_id;
    building_source.uri = spec.buildings_uri;
    building_source.version = spec.buildings_version;
    building_source.license = spec.buildings_license;
    building_source.crs = epsg_for(horizontal_crs_from_epsg(spec.buildings_source_crs));
    building_source.vertical_datum =
        building_datum == VerticalDatum::Unknown ? "" : to_string(building_datum);
    building_source.content_hash = hash_file_bytes(spec.buildings_path);
    manifest.sources.push_back(building_source);

    if (have_roads) {
        SourceSnapshot road_source;
        road_source.source_id = spec.roads_source_id;
        road_source.uri = spec.roads_uri;
        road_source.version = spec.roads_version;
        road_source.license = spec.roads_license;
        road_source.crs = "EPSG:4326";
        road_source.content_hash = hash_file_bytes(spec.roads_path);
        manifest.sources.push_back(road_source);
    }
    if (result.terrain_textured) {
        SourceSnapshot basemap_source;
        basemap_source.source_id = spec.basemap_source_id;
        basemap_source.license = spec.basemap_license;
        basemap_source.crs = "EPSG:3857";
        manifest.sources.push_back(basemap_source);
    }

    // 8. Tile (single AOI-wide tile for M1) ----------------------------------
    WorldTile tile;
    tile.tile_id = "t_0_0";
    tile.bounds = aoi;
    tile.elevation_state = elevation_state;
    tile.elevation_fallback_reason = elevation_reason;
    tile.content_hash =
        fold_u64(fold_u64(kFnvOffset, terrain.validation.fused_raster_hash),
                 city_mesh_vertex_hash(result.city));
    tile.provenance.push_back({WorldLayerKind::Terrain, spec.terrain_source_id,
                               terrain.fused.source_algorithm, terrain.param_hash});
    tile.provenance.push_back({WorldLayerKind::Buildings, spec.buildings_source_id,
                               buildings.algorithm_id, buildings.params_hash});
    if (have_roads) {
        tile.provenance.push_back({WorldLayerKind::Roads, spec.roads_source_id,
                                   roads.algorithm_id, roads.params_hash});
    }
    if (result.terrain_textured) {
        tile.provenance.push_back(
            {WorldLayerKind::Basemap, spec.basemap_source_id, "osm_drape", 0});
    }

    // 9. Manifest assembly ---------------------------------------------------
    manifest.compiler_version = spec.compiler_version;
    manifest.seed = spec.seed;
    manifest.aoi = aoi;
    manifest.crs_policy.horizontal = "EPSG:4326";
    manifest.crs_policy.vertical_datum =
        terrain_datum == VerticalDatum::Unknown ? "" : to_string(terrain_datum);
    manifest.crs_policy.runtime_frame = "local_enu_m";
    manifest.tiles.push_back(std::move(tile));
    manifest.world_hash = fold_manifest(manifest);

    result.manifest = std::move(manifest);
    result.ok = true;
    return result;
}

WorldWriteResult write_world_artifacts(const WorldCompileResult& result,
                                       const std::filesystem::path& output_dir,
                                       const std::string& name) {
    WorldWriteResult out;
    if (!result.ok) {
        out.error_code = "compile_not_ok";
        out.error_detail = result.error_code;
        return out;
    }
    std::error_code ec;
    std::filesystem::create_directories(output_dir, ec);

    const std::filesystem::path scene_path = output_dir / (name + ".agbscn");
    if (const auto error = agbot::render::write_scene_file(scene_path, result.scene)) {
        out.error_code = "scene_write_failed";
        out.error_detail = error->message;
        return out;
    }

    WorldManifest manifest = result.manifest;
    for (WorldTile& tile : manifest.tiles) {
        tile.scene_path = name + ".agbscn";
    }
    const std::filesystem::path manifest_path = output_dir / (name + ".agbworld");
    std::ofstream manifest_out(manifest_path, std::ios::binary);
    if (!manifest_out) {
        out.error_code = "manifest_write_failed";
        out.error_detail = manifest_path.string();
        return out;
    }
    manifest_out << manifest.to_json();
    if (!manifest_out) {
        out.error_code = "manifest_write_failed";
        out.error_detail = manifest_path.string();
        return out;
    }

    out.ok = true;
    out.scene_path = scene_path;
    out.manifest_path = manifest_path;
    return out;
}

} // namespace agbot::worldgen
