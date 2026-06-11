#include "agbot_flight_sim/GeoTerrain.hpp"

#include <algorithm>
#include <cmath>
#include <limits>
#include <stdexcept>

namespace agbot::flight_sim {
namespace {

constexpr double kPi = 3.14159265358979323846;
constexpr double kEarthMetersPerDegreeLat = 111'320.0;
constexpr double kMercatorLatitudeLimit = 85.05112878;

double deg_to_rad(double degrees) {
    return degrees * kPi / 180.0;
}

double rad_to_deg(double radians) {
    return radians * 180.0 / kPi;
}

double longitude_degrees_per_meter(double latitude_degrees) {
    return 1.0 / (kEarthMetersPerDegreeLat * std::max(0.01, std::abs(std::cos(deg_to_rad(latitude_degrees)))));
}

float sample_heightmap(const std::vector<float>& heightmap, int resolution, int x, int z) {
    x = std::clamp(x, 0, resolution - 1);
    z = std::clamp(z, 0, resolution - 1);
    return heightmap[static_cast<std::size_t>(z * resolution + x)];
}

bool same_tile(TileCoordinate a, TileCoordinate b) {
    return a.z == b.z && a.x == b.x && a.y == b.y;
}

} // namespace

GeoBounds GeoBounds::from_center(const GeoCoordinate& center, double radius_m) {
    const double lat_delta = radius_m / kEarthMetersPerDegreeLat;
    const double lon_delta = radius_m * longitude_degrees_per_meter(center.latitude);
    return {
        center.latitude - lat_delta,
        center.longitude - lon_delta,
        center.latitude + lat_delta,
        center.longitude + lon_delta,
    };
}

GeoCoordinate GeoBounds::center() const {
    return {
        (min_latitude + max_latitude) * 0.5,
        (min_longitude + max_longitude) * 0.5,
        0.0,
    };
}

double GeoBounds::width_m() const {
    const GeoCoordinate middle = center();
    return (max_longitude - min_longitude) / longitude_degrees_per_meter(middle.latitude);
}

double GeoBounds::height_m() const {
    return (max_latitude - min_latitude) * kEarthMetersPerDegreeLat;
}

GeoBounds TileCoordinate::bounds() const {
    const double n = std::pow(2.0, z);
    const auto latitude_for_y = [n](int tile_y) {
        const double mercator = kPi * (1.0 - 2.0 * static_cast<double>(tile_y) / n);
        return rad_to_deg(std::atan(std::sinh(mercator)));
    };

    return {
        latitude_for_y(y + 1),
        (static_cast<double>(x) / n * 360.0) - 180.0,
        latitude_for_y(y),
        (static_cast<double>(x + 1) / n * 360.0) - 180.0,
    };
}

float ElevationTile::sample_bilinear(double u, double v) const {
    if (width <= 0 || height <= 0 || elevations_m.empty()) {
        return 0.0f;
    }

    u = std::clamp(u, 0.0, 1.0);
    v = std::clamp(v, 0.0, 1.0);

    const double fx = u * static_cast<double>(width - 1);
    const double fy = v * static_cast<double>(height - 1);
    const int x0 = static_cast<int>(std::floor(fx));
    const int y0 = static_cast<int>(std::floor(fy));
    const int x1 = std::min(x0 + 1, width - 1);
    const int y1 = std::min(y0 + 1, height - 1);
    const double tx = fx - static_cast<double>(x0);
    const double ty = fy - static_cast<double>(y0);

    const auto at = [this](int x, int y) {
        return elevations_m[static_cast<std::size_t>(y * width + x)];
    };

    const double top = static_cast<double>(at(x0, y0)) * (1.0 - tx) + static_cast<double>(at(x1, y0)) * tx;
    const double bottom = static_cast<double>(at(x0, y1)) * (1.0 - tx) + static_cast<double>(at(x1, y1)) * tx;
    return static_cast<float>(top * (1.0 - ty) + bottom * ty);
}

bool ElevationComposite::has_state(TerrainTileState state) const {
    return std::any_of(tile_states.begin(), tile_states.end(), [state](const TerrainTileStatus& status) {
        return status.state == state;
    });
}

const char* to_string(TerrainTileState state) {
    switch (state) {
        case TerrainTileState::Available:
            return "available";
        case TerrainTileState::Missing:
            return "missing";
        case TerrainTileState::Stale:
            return "stale";
        case TerrainTileState::Synthetic:
            return "synthetic";
        case TerrainTileState::FlatFallback:
            return "flat_fallback";
    }
    return "unknown";
}

double radius_m_for_area_km2(double area_km2) {
    const double clamped_area = std::max(0.0, area_km2);
    return std::sqrt((clamped_area * 1'000'000.0) / kPi);
}

TileCoordinate tile_for_geo(const GeoCoordinate& coordinate, int zoom) {
    const double n = std::pow(2.0, zoom);
    const double latitude = std::clamp(coordinate.latitude, -kMercatorLatitudeLimit, kMercatorLatitudeLimit);
    const double lat_rad = deg_to_rad(latitude);
    const int max_tile = static_cast<int>(n) - 1;
    return {
        zoom,
        std::clamp(static_cast<int>(std::floor((coordinate.longitude + 180.0) / 360.0 * n)), 0, max_tile),
        std::clamp(static_cast<int>(std::floor((1.0 - std::asinh(std::tan(lat_rad)) / kPi) * 0.5 * n)), 0, max_tile),
    };
}

int zoom_for_radius_m(double radius_m) {
    if (radius_m > 10'000.0) {
        return 10;
    }
    if (radius_m > 5'000.0) {
        return 11;
    }
    if (radius_m > 2'000.0) {
        return 12;
    }
    if (radius_m > 1'000.0) {
        return 13;
    }
    return 14;
}

std::vector<TileCoordinate> tiles_for_bounds(const GeoBounds& bounds, int zoom) {
    const TileCoordinate min_tile = tile_for_geo({bounds.max_latitude, bounds.min_longitude, 0.0}, zoom);
    const TileCoordinate max_tile = tile_for_geo({bounds.min_latitude, bounds.max_longitude, 0.0}, zoom);

    std::vector<TileCoordinate> tiles;
    for (int x = min_tile.x; x <= max_tile.x; ++x) {
        for (int y = min_tile.y; y <= max_tile.y; ++y) {
            tiles.push_back({zoom, x, y});
        }
    }
    return tiles;
}

std::optional<ElevationTile> elevation_tile_from_terrarium_rgba(
    TileCoordinate coordinate,
    int width,
    int height,
    const std::vector<std::uint8_t>& rgba_pixels) {
    if (width <= 0 || height <= 0 || rgba_pixels.size() != static_cast<std::size_t>(width * height * 4)) {
        return std::nullopt;
    }

    ElevationTile tile;
    tile.coordinate = coordinate;
    tile.state = TerrainTileState::Available;
    tile.state_reason = "terrarium_rgba_decoded";
    tile.width = width;
    tile.height = height;
    tile.elevations_m.reserve(static_cast<std::size_t>(width * height));
    tile.min_elevation_m = std::numeric_limits<float>::max();
    tile.max_elevation_m = std::numeric_limits<float>::lowest();

    for (int index = 0; index < width * height; ++index) {
        const std::size_t offset = static_cast<std::size_t>(index * 4);
        const float r = static_cast<float>(rgba_pixels[offset]);
        const float g = static_cast<float>(rgba_pixels[offset + 1]);
        const float b = static_cast<float>(rgba_pixels[offset + 2]);
        const float elevation = (r * 256.0f + g + b / 256.0f) - 32768.0f;
        tile.elevations_m.push_back(elevation);
        tile.min_elevation_m = std::min(tile.min_elevation_m, elevation);
        tile.max_elevation_m = std::max(tile.max_elevation_m, elevation);
    }

    return tile;
}

std::vector<float> composite_elevation(
    const std::vector<ElevationTile>& tiles,
    const GeoBounds& bounds,
    int resolution) {
    if (resolution <= 0) {
        return {};
    }

    std::vector<float> heightmap(static_cast<std::size_t>(resolution * resolution), 0.0f);
    if (tiles.empty()) {
        return heightmap;
    }

    const double lat_span = bounds.max_latitude - bounds.min_latitude;
    const double lon_span = bounds.max_longitude - bounds.min_longitude;

    for (int z = 0; z < resolution; ++z) {
        for (int x = 0; x < resolution; ++x) {
            const double u = resolution == 1 ? 0.0 : static_cast<double>(x) / static_cast<double>(resolution - 1);
            const double v = resolution == 1 ? 0.0 : static_cast<double>(z) / static_cast<double>(resolution - 1);
            const double latitude = bounds.max_latitude - v * lat_span;
            const double longitude = bounds.min_longitude + u * lon_span;

            for (const ElevationTile& tile : tiles) {
                const GeoBounds tile_bounds = tile.coordinate.bounds();
                if (latitude < tile_bounds.min_latitude || latitude > tile_bounds.max_latitude ||
                    longitude < tile_bounds.min_longitude || longitude > tile_bounds.max_longitude) {
                    continue;
                }

                const double tile_u = (longitude - tile_bounds.min_longitude) /
                    (tile_bounds.max_longitude - tile_bounds.min_longitude);
                const double tile_v = 1.0 - ((latitude - tile_bounds.min_latitude) /
                    (tile_bounds.max_latitude - tile_bounds.min_latitude));
                heightmap[static_cast<std::size_t>(z * resolution + x)] = tile.sample_bilinear(tile_u, tile_v);
                break;
            }
        }
    }

    return heightmap;
}

ElevationComposite composite_elevation_with_state(
    const std::vector<ElevationTile>& tiles,
    const GeoBounds& bounds,
    int resolution,
    const std::vector<TileCoordinate>& expected_tiles) {
    ElevationComposite composite;
    composite.heightmap = composite_elevation(tiles, bounds, resolution);

    for (const ElevationTile& tile : tiles) {
        composite.tile_states.push_back({
            tile.coordinate,
            tile.state,
            tile.state_reason.empty() ? std::string("elevation tile available") : tile.state_reason,
        });
    }

    for (const TileCoordinate& expected : expected_tiles) {
        const auto found = std::find_if(tiles.begin(), tiles.end(), [expected](const ElevationTile& tile) {
            return same_tile(tile.coordinate, expected);
        });
        if (found == tiles.end()) {
            composite.tile_states.push_back({
                expected,
                TerrainTileState::FlatFallback,
                "missing elevation tile; using flat fallback heightmap",
            });
        }
    }

    return composite;
}

TerrainMesh build_terrain_mesh(
    const std::vector<float>& heightmap,
    int resolution,
    double width_m,
    double depth_m,
    double vertical_scale) {
    if (resolution < 2 || heightmap.size() != static_cast<std::size_t>(resolution * resolution)) {
        throw std::invalid_argument("heightmap size does not match terrain resolution");
    }

    TerrainMesh mesh;
    mesh.vertices.reserve(static_cast<std::size_t>(resolution * resolution));
    mesh.min_elevation_m = *std::min_element(heightmap.begin(), heightmap.end());
    mesh.max_elevation_m = *std::max_element(heightmap.begin(), heightmap.end());
    mesh.has_elevation = mesh.max_elevation_m > mesh.min_elevation_m;

    const double half_width = width_m * 0.5;
    const double half_depth = depth_m * 0.5;
    const double step_x = width_m / static_cast<double>(resolution - 1);
    const double step_z = depth_m / static_cast<double>(resolution - 1);

    for (int z = 0; z < resolution; ++z) {
        for (int x = 0; x < resolution; ++x) {
            const std::size_t index = static_cast<std::size_t>(z * resolution + x);
            const double u = static_cast<double>(x) / static_cast<double>(resolution - 1);
            const double v = static_cast<double>(z) / static_cast<double>(resolution - 1);
            const double height = (static_cast<double>(heightmap[index]) - mesh.min_elevation_m) * vertical_scale;

            mesh.vertices.push_back({
                Vec3(u * width_m - half_width, height, v * depth_m - half_depth),
                Vec3(0.0, 1.0, 0.0),
                u,
                1.0 - v,
            });
        }
    }

    for (int z = 0; z < resolution; ++z) {
        for (int x = 0; x < resolution; ++x) {
            const std::size_t index = static_cast<std::size_t>(z * resolution + x);
            const float h_l = sample_heightmap(heightmap, resolution, x - 1, z);
            const float h_r = sample_heightmap(heightmap, resolution, x + 1, z);
            const float h_d = sample_heightmap(heightmap, resolution, x, z - 1);
            const float h_u = sample_heightmap(heightmap, resolution, x, z + 1);
            const double dx = ((h_r - h_l) * vertical_scale) / (2.0 * step_x);
            const double dz = ((h_u - h_d) * vertical_scale) / (2.0 * step_z);
            mesh.vertices[index].normal = Vec3(-dx, 1.0, -dz).normalized();
        }
    }

    for (int z = 0; z < resolution - 1; ++z) {
        for (int x = 0; x < resolution - 1; ++x) {
            const std::uint32_t top_left = static_cast<std::uint32_t>(z * resolution + x);
            const std::uint32_t top_right = top_left + 1;
            const std::uint32_t bottom_left = static_cast<std::uint32_t>((z + 1) * resolution + x);
            const std::uint32_t bottom_right = bottom_left + 1;
            mesh.indices.push_back(top_left);
            mesh.indices.push_back(bottom_left);
            mesh.indices.push_back(top_right);
            mesh.indices.push_back(top_right);
            mesh.indices.push_back(bottom_left);
            mesh.indices.push_back(bottom_right);
        }
    }

    return mesh;
}

} // namespace agbot::flight_sim
