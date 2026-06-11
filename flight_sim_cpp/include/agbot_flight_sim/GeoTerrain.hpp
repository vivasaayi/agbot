#pragma once

#include "agbot_flight_sim/Mission.hpp"
#include "agbot_flight_sim/Vec3.hpp"

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace agbot::flight_sim {

struct GeoBounds {
    double min_latitude = 0.0;
    double min_longitude = 0.0;
    double max_latitude = 0.0;
    double max_longitude = 0.0;

    [[nodiscard]] static GeoBounds from_center(const GeoCoordinate& center, double radius_m);
    [[nodiscard]] GeoCoordinate center() const;
    [[nodiscard]] double width_m() const;
    [[nodiscard]] double height_m() const;
};

struct TileCoordinate {
    int z = 0;
    int x = 0;
    int y = 0;

    [[nodiscard]] GeoBounds bounds() const;
};

enum class TerrainTileState {
    Available,
    Missing,
    Stale,
    Synthetic,
    FlatFallback,
};

struct TerrainTileStatus {
    TileCoordinate coordinate;
    TerrainTileState state = TerrainTileState::Available;
    std::string reason;
};

struct ElevationTile {
    TileCoordinate coordinate;
    TerrainTileState state = TerrainTileState::Available;
    std::string state_reason;
    int width = 0;
    int height = 0;
    std::vector<float> elevations_m;
    float min_elevation_m = 0.0f;
    float max_elevation_m = 0.0f;

    [[nodiscard]] float sample_bilinear(double u, double v) const;
};

struct ElevationComposite {
    std::vector<float> heightmap;
    std::vector<TerrainTileStatus> tile_states;

    [[nodiscard]] bool has_state(TerrainTileState state) const;
};

struct TerrainVertex {
    Vec3 position;
    Vec3 normal;
    double u = 0.0;
    double v = 0.0;
};

struct TerrainMesh {
    std::vector<TerrainVertex> vertices;
    std::vector<std::uint32_t> indices;
    float min_elevation_m = 0.0f;
    float max_elevation_m = 0.0f;
    bool has_elevation = false;
};

[[nodiscard]] double radius_m_for_area_km2(double area_km2);
[[nodiscard]] TileCoordinate tile_for_geo(const GeoCoordinate& coordinate, int zoom);
[[nodiscard]] int zoom_for_radius_m(double radius_m);
[[nodiscard]] std::vector<TileCoordinate> tiles_for_bounds(const GeoBounds& bounds, int zoom);

[[nodiscard]] std::optional<ElevationTile> elevation_tile_from_terrarium_rgba(
    TileCoordinate coordinate,
    int width,
    int height,
    const std::vector<std::uint8_t>& rgba_pixels);

[[nodiscard]] std::vector<float> composite_elevation(
    const std::vector<ElevationTile>& tiles,
    const GeoBounds& bounds,
    int resolution);

[[nodiscard]] ElevationComposite composite_elevation_with_state(
    const std::vector<ElevationTile>& tiles,
    const GeoBounds& bounds,
    int resolution,
    const std::vector<TileCoordinate>& expected_tiles);

[[nodiscard]] TerrainMesh build_terrain_mesh(
    const std::vector<float>& heightmap,
    int resolution,
    double width_m,
    double depth_m,
    double vertical_scale = 1.0);

[[nodiscard]] const char* to_string(TerrainTileState state);

} // namespace agbot::flight_sim
