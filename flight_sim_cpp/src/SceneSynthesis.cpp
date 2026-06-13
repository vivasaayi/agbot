#include "agbot_flight_sim/SceneSynthesis.hpp"

#include "agbot_flight_sim/DeterministicRunner.hpp"
#include "agbot_flight_sim/TwinContractV1.hpp"

#include <algorithm>
#include <cmath>
#include <iomanip>
#include <sstream>
#include <string_view>

namespace agbot::flight_sim {
namespace {

std::string escape_json(std::string_view value) {
    std::ostringstream output;
    for (const char c : value) {
        switch (c) {
            case '"':
                output << "\\\"";
                break;
            case '\\':
                output << "\\\\";
                break;
            case '\n':
                output << "\\n";
                break;
            case '\r':
                output << "\\r";
                break;
            case '\t':
                output << "\\t";
                break;
            default:
                output << c;
                break;
        }
    }
    return output.str();
}

void write_double(std::ostringstream& output, double value, int precision = 6) {
    output << std::fixed << std::setprecision(precision) << value;
}

void write_bounds_json(std::ostringstream& output, const GeoBounds& bounds) {
    output << "{\"min_latitude\":";
    write_double(output, bounds.min_latitude, 7);
    output << ",\"min_longitude\":";
    write_double(output, bounds.min_longitude, 7);
    output << ",\"max_latitude\":";
    write_double(output, bounds.max_latitude, 7);
    output << ",\"max_longitude\":";
    write_double(output, bounds.max_longitude, 7);
    output << "}";
}

void write_geo_json(std::ostringstream& output, const GeoCoordinate& coordinate) {
    output << "{\"latitude\":";
    write_double(output, coordinate.latitude, 7);
    output << ",\"longitude\":";
    write_double(output, coordinate.longitude, 7);
    output << ",\"altitude_m\":";
    write_double(output, coordinate.altitude_m, 3);
    output << "}";
}

void write_vec3_json(std::ostringstream& output, const Vec3& value) {
    output << "{\"x\":";
    write_double(output, value.x, 3);
    output << ",\"y\":";
    write_double(output, value.y, 3);
    output << ",\"z\":";
    write_double(output, value.z, 3);
    output << "}";
}

bool footprint_overlaps_profile(
    const std::vector<GeoCoordinate>& footprint,
    const TerrainProfile& profile) {
    return std::any_of(footprint.begin(), footprint.end(), [&](const GeoCoordinate& coordinate) {
        return profile.contains(coordinate);
    });
}

std::uint64_t placement_seed_for(
    std::uint64_t scene_seed,
    std::string_view source_kind,
    std::string_view source_id,
    std::string_view class_name) {
    std::ostringstream input;
    input << scene_seed << "|" << source_kind << "|" << source_id << "|" << class_name;
    return fnv1a64(input.str());
}

std::string object_id_for(std::uint64_t placement_seed) {
    return "scene_object:" + to_hex(placement_seed);
}

std::vector<Vec3> local_footprint(
    const std::vector<GeoCoordinate>& footprint,
    const TerrainProfile& profile) {
    const GeoCoordinate origin = profile.bounds.center();
    std::vector<Vec3> local;
    local.reserve(footprint.size());
    for (const GeoCoordinate& coordinate : footprint) {
        local.push_back(local_from_geo(coordinate, origin));
    }
    return local;
}

double vegetation_height(double canopy_height_m, std::uint64_t placement_seed) {
    const double jitter = static_cast<double>(placement_seed % 1000ULL) / 1000.0;
    return std::max(0.1, canopy_height_m) * (1.0 + jitter * 0.15);
}

SceneObject building_object(
    const BuildingFootprintFeature& building,
    const TerrainProfile& profile,
    std::uint64_t seed) {
    const std::uint64_t placement_seed =
        placement_seed_for(seed, "building", building.source_id, building.class_name);
    return {
        object_id_for(placement_seed),
        building.source_id,
        "building",
        building.class_name.empty() ? "building" : building.class_name,
        "",
        building.footprint,
        local_footprint(building.footprint, profile),
        std::max(0.1, building.height_m),
        placement_seed,
    };
}

SceneObject vegetation_object(
    const VegetationClassFeature& vegetation,
    const TerrainProfile& profile,
    std::uint64_t seed) {
    const std::string class_name =
        vegetation.class_name.empty() ? "vegetation" : vegetation.class_name;
    const std::uint64_t placement_seed =
        placement_seed_for(seed, "vegetation", vegetation.source_id, class_name);
    return {
        object_id_for(placement_seed),
        vegetation.source_id,
        "vegetation",
        class_name,
        vegetation.crop,
        vegetation.footprint,
        local_footprint(vegetation.footprint, profile),
        vegetation_height(vegetation.canopy_height_m, placement_seed),
        placement_seed,
    };
}

bool object_sort_key(const SceneObject& left, const SceneObject& right) {
    if (left.source_kind != right.source_kind) {
        return left.source_kind < right.source_kind;
    }
    return left.source_id < right.source_id;
}

std::string manifest_json_without_hash(const SceneSynthesisManifest& manifest) {
    std::ostringstream output;
    output << "{"
           << "\"contract_version\":\"" << escape_json(manifest.contract_version) << "\""
           << ",\"status\":\"" << to_string(manifest.status) << "\""
           << ",\"seed\":" << manifest.seed
           << ",\"terrain_profile\":{"
           << "\"crs\":\"" << escape_json(manifest.profile.crs) << "\""
           << ",\"asserted\":" << (manifest.profile.asserted ? "true" : "false")
           << ",\"resolution\":" << manifest.profile.resolution
           << ",\"resolution_x_m\":";
    write_double(output, manifest.profile.resolution_x_m, 3);
    output << ",\"resolution_y_m\":";
    write_double(output, manifest.profile.resolution_y_m, 3);
    output << ",\"bounds\":";
    write_bounds_json(output, manifest.profile.bounds);
    output << "}"
           << ",\"object_count\":" << manifest.object_count
           << ",\"unpopulated_area_count\":" << manifest.unpopulated_area_count
           << ",\"objects\":[";
    for (std::size_t object_index = 0; object_index < manifest.objects.size(); ++object_index) {
        if (object_index > 0) {
            output << ",";
        }
        const SceneObject& object = manifest.objects[object_index];
        output << "{\"object_id\":\"" << escape_json(object.object_id) << "\""
               << ",\"source_id\":\"" << escape_json(object.source_id) << "\""
               << ",\"source_kind\":\"" << escape_json(object.source_kind) << "\""
               << ",\"class_name\":\"" << escape_json(object.class_name) << "\""
               << ",\"crop\":\"" << escape_json(object.crop) << "\""
               << ",\"height_m\":";
        write_double(output, object.height_m, 3);
        output << ",\"placement_seed\":" << object.placement_seed
               << ",\"footprint_geo\":[";
        for (std::size_t point_index = 0; point_index < object.footprint_geo.size(); ++point_index) {
            if (point_index > 0) {
                output << ",";
            }
            write_geo_json(output, object.footprint_geo[point_index]);
        }
        output << "],\"footprint_local_m\":[";
        for (std::size_t point_index = 0; point_index < object.footprint_local_m.size(); ++point_index) {
            if (point_index > 0) {
                output << ",";
            }
            write_vec3_json(output, object.footprint_local_m[point_index]);
        }
        output << "]}";
    }
    output << "],\"unpopulated_areas\":[";
    for (std::size_t area_index = 0; area_index < manifest.unpopulated_areas.size(); ++area_index) {
        if (area_index > 0) {
            output << ",";
        }
        const UnpopulatedArea& area = manifest.unpopulated_areas[area_index];
        output << "{\"reason\":\"" << escape_json(area.reason) << "\""
               << ",\"bounds\":";
        write_bounds_json(output, area.bounds);
        output << "}";
    }
    output << "]}";
    return output.str();
}

} // namespace

const char* to_string(SceneSynthesisStatus status) {
    switch (status) {
        case SceneSynthesisStatus::Ready:
            return "ready";
        case SceneSynthesisStatus::Unpopulated:
            return "unpopulated";
        case SceneSynthesisStatus::Invalid:
            return "invalid";
    }
    return "unknown";
}

std::string SceneSynthesisManifest::to_json() const {
    std::string json = manifest_json_without_hash(*this);
    json.pop_back();
    json += ",\"scene_hash\":\"" + escape_json(scene_hash) + "\"}";
    return json;
}

SceneSynthesisManifest synthesize_scene_manifest(const SceneSynthesisInput& input) {
    SceneSynthesisManifest manifest;
    manifest.contract_version = kTwinContractVersion;
    manifest.seed = input.seed;
    manifest.profile = input.profile;

    if (!input.profile.asserted || input.profile.crs.empty()) {
        manifest.status = SceneSynthesisStatus::Invalid;
        manifest.unpopulated_areas.push_back({input.profile.bounds, "terrain_profile_not_asserted"});
    } else {
        for (const BuildingFootprintFeature& building : input.buildings) {
            if (!footprint_overlaps_profile(building.footprint, input.profile)) {
                continue;
            }
            manifest.objects.push_back(building_object(building, input.profile, input.seed));
        }
        for (const VegetationClassFeature& vegetation : input.vegetation) {
            if (!footprint_overlaps_profile(vegetation.footprint, input.profile)) {
                continue;
            }
            manifest.objects.push_back(vegetation_object(vegetation, input.profile, input.seed));
        }

        std::sort(manifest.objects.begin(), manifest.objects.end(), object_sort_key);

        if (manifest.objects.empty()) {
            manifest.status = SceneSynthesisStatus::Unpopulated;
            manifest.unpopulated_areas.push_back({input.profile.bounds, "missing_source_coverage"});
        } else {
            manifest.status = SceneSynthesisStatus::Ready;
        }
    }

    manifest.object_count = manifest.objects.size();
    manifest.unpopulated_area_count = manifest.unpopulated_areas.size();
    manifest.scene_hash = sha256_hex(manifest_json_without_hash(manifest));
    return manifest;
}

} // namespace agbot::flight_sim
