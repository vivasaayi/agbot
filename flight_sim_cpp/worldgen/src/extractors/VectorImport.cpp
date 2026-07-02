#include "agbot_worldgen/extractors/VectorImport.hpp"

#include "agbot_worldgen/HeightResolver.hpp"

#include <nlohmann/json.hpp>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

namespace agbot::worldgen {
namespace {

using agbot::flight_sim::GeoBounds;
using agbot::flight_sim::GeoCoordinate;
using agbot::flight_sim::Vec3;
using nlohmann::json;

struct ImportParams {
    std::string path;
    std::string height_attr;
    double height_unit_scale_m = 1.0;
    std::string base_elev_attr;
    double base_unit_scale_m = 1.0;
    std::string levels_attr;
    double default_level_height_m = 3.0;
    double default_height_m = 3.0;
    std::string class_attr;
    std::string default_class = "building";
    std::string id_attr;
    double min_area_m2 = 10.0;
    double simplify_tol_m = 0.0;
    std::int64_t max_features = 0;
};

double unit_scale_for(const std::string& units) {
    if (units == "feet" || units == "ft") {
        return 0.3048;
    }
    return 1.0;
}

ImportParams read_params(const agbot::config::ParamTable& table) {
    namespace cfg = agbot::config;
    ImportParams params;
    params.path = cfg::string_or(table, "path", "");
    params.height_attr = cfg::string_or(table, "height_attr", "");
    params.height_unit_scale_m = unit_scale_for(cfg::string_or(table, "height_units", "meters"));
    params.base_elev_attr = cfg::string_or(table, "base_elev_attr", "");
    params.base_unit_scale_m = unit_scale_for(cfg::string_or(table, "base_units", "meters"));
    params.levels_attr = cfg::string_or(table, "levels_attr", "");
    params.default_level_height_m = cfg::double_or(table, "default_level_height_m", 3.0);
    params.default_height_m = cfg::double_or(table, "default_height_m", 3.0);
    params.class_attr = cfg::string_or(table, "class_attr", "");
    params.default_class = cfg::string_or(table, "default_class", "building");
    params.id_attr = cfg::string_or(table, "id_attr", "");
    params.min_area_m2 = cfg::double_or(table, "min_area_m2", 10.0);
    params.simplify_tol_m = cfg::double_or(table, "simplify_tol_m", 0.0);
    params.max_features = cfg::integer_or(table, "max_features", 0);
    return params;
}

std::optional<double> numeric_property(const json& properties, const std::string& name) {
    if (name.empty() || !properties.is_object()) {
        return std::nullopt;
    }
    const auto it = properties.find(name);
    if (it == properties.end()) {
        return std::nullopt;
    }
    if (it->is_number()) {
        return it->get<double>();
    }
    if (it->is_string()) {
        return parse_numeric_attribute(it->get<std::string>());
    }
    return std::nullopt;
}

std::optional<std::string> string_property(const json& properties, const std::string& name) {
    if (name.empty() || !properties.is_object()) {
        return std::nullopt;
    }
    const auto it = properties.find(name);
    if (it == properties.end()) {
        return std::nullopt;
    }
    if (it->is_string()) {
        const std::string value = it->get<std::string>();
        if (value.empty()) {
            return std::nullopt;
        }
        return value;
    }
    if (it->is_number_integer()) {
        return std::to_string(it->get<std::int64_t>());
    }
    if (it->is_number()) {
        std::ostringstream formatted;
        formatted << it->get<double>();
        return formatted.str();
    }
    return std::nullopt;
}

// Parses a GeoJSON linear ring ([[lon, lat], ...]) into geodetic points,
// dropping the duplicated closing point. Returns an empty vector when the
// ring is malformed or degenerate.
std::vector<GeoCoordinate> parse_ring(const json& ring_json) {
    if (!ring_json.is_array()) {
        return {};
    }
    std::vector<GeoCoordinate> ring;
    ring.reserve(ring_json.size());
    for (const json& point : ring_json) {
        if (!point.is_array() || point.size() < 2 || !point[0].is_number() || !point[1].is_number()) {
            return {};
        }
        ring.push_back({point[1].get<double>(), point[0].get<double>(), 0.0});
    }
    if (ring.size() >= 2) {
        const GeoCoordinate& first = ring.front();
        const GeoCoordinate& last = ring.back();
        if (first.latitude == last.latitude && first.longitude == last.longitude) {
            ring.pop_back();
        }
    }
    if (ring.size() < 3) {
        return {};
    }
    return ring;
}

bool ring_intersects_aoi(const std::vector<GeoCoordinate>& ring, const GeoBounds& aoi) {
    double min_lat = ring.front().latitude;
    double max_lat = ring.front().latitude;
    double min_lon = ring.front().longitude;
    double max_lon = ring.front().longitude;
    for (const GeoCoordinate& point : ring) {
        min_lat = std::min(min_lat, point.latitude);
        max_lat = std::max(max_lat, point.latitude);
        min_lon = std::min(min_lon, point.longitude);
        max_lon = std::max(max_lon, point.longitude);
    }
    return !(max_lat < aoi.min_latitude || min_lat > aoi.max_latitude ||
             max_lon < aoi.min_longitude || min_lon > aoi.max_longitude);
}

double point_segment_distance(const Vec3& point, const Vec3& start, const Vec3& end) {
    const double dx = end.x - start.x;
    const double dz = end.z - start.z;
    const double length_sq = dx * dx + dz * dz;
    double t = 0.0;
    if (length_sq > 0.0) {
        t = ((point.x - start.x) * dx + (point.z - start.z) * dz) / length_sq;
        t = std::clamp(t, 0.0, 1.0);
    }
    const double px = start.x + t * dx - point.x;
    const double pz = start.z + t * dz - point.z;
    return std::sqrt(px * px + pz * pz);
}

void douglas_peucker(
    const std::vector<Vec3>& points,
    std::size_t first,
    std::size_t last,
    double tolerance_m,
    std::vector<bool>& keep) {
    if (last <= first + 1) {
        return;
    }
    double max_distance = -1.0;
    std::size_t max_index = first;
    for (std::size_t index = first + 1; index < last; ++index) {
        const double distance = point_segment_distance(points[index], points[first], points[last]);
        if (distance > max_distance) {
            max_distance = distance;
            max_index = index;
        }
    }
    if (max_distance > tolerance_m) {
        keep[max_index] = true;
        douglas_peucker(points, first, max_index, tolerance_m, keep);
        douglas_peucker(points, max_index, last, tolerance_m, keep);
    }
}

// Douglas-Peucker in local meters over a closed ring (closing edge included).
// Keeps the original ring when simplification would collapse it below a
// triangle.
std::vector<GeoCoordinate> simplify_ring(
    const std::vector<GeoCoordinate>& ring,
    const GeoCoordinate& origin,
    double tolerance_m) {
    if (tolerance_m <= 0.0 || ring.size() <= 3) {
        return ring;
    }
    std::vector<Vec3> local;
    local.reserve(ring.size() + 1);
    for (const GeoCoordinate& point : ring) {
        local.push_back(agbot::flight_sim::local_from_geo(point, origin));
    }
    local.push_back(local.front());

    std::vector<bool> keep(local.size(), false);
    keep.front() = true;
    keep.back() = true;
    douglas_peucker(local, 0, local.size() - 1, tolerance_m, keep);

    std::vector<GeoCoordinate> simplified;
    simplified.reserve(ring.size());
    for (std::size_t index = 0; index < ring.size(); ++index) {
        if (keep[index]) {
            simplified.push_back(ring[index]);
        }
    }
    if (simplified.size() < 3) {
        return ring;
    }
    return simplified;
}

struct PolygonRings {
    std::vector<GeoCoordinate> exterior;
    std::vector<std::vector<GeoCoordinate>> holes;
};

// Extracts each polygon of a Polygon/MultiPolygon geometry; invalid polygons
// are skipped, invalid holes dropped.
std::vector<PolygonRings> parse_polygons(const json& geometry) {
    std::vector<PolygonRings> polygons;
    if (!geometry.is_object()) {
        return polygons;
    }
    const auto type_it = geometry.find("type");
    const auto coords_it = geometry.find("coordinates");
    if (type_it == geometry.end() || coords_it == geometry.end() || !type_it->is_string() ||
        !coords_it->is_array()) {
        return polygons;
    }
    const std::string type = type_it->get<std::string>();

    const auto parse_polygon = [&polygons](const json& rings_json) {
        if (!rings_json.is_array() || rings_json.empty()) {
            return;
        }
        PolygonRings polygon;
        polygon.exterior = parse_ring(rings_json[0]);
        if (polygon.exterior.empty()) {
            return;
        }
        for (std::size_t ring_index = 1; ring_index < rings_json.size(); ++ring_index) {
            std::vector<GeoCoordinate> hole = parse_ring(rings_json[ring_index]);
            if (!hole.empty()) {
                polygon.holes.push_back(std::move(hole));
            }
        }
        polygons.push_back(std::move(polygon));
    };

    if (type == "Polygon") {
        parse_polygon(*coords_it);
    } else if (type == "MultiPolygon") {
        for (const json& polygon_json : *coords_it) {
            parse_polygon(polygon_json);
        }
    }
    return polygons;
}

} // namespace

std::string VectorImportExtractor::id() const {
    return kId;
}

std::vector<FeatureClass> VectorImportExtractor::produces() const {
    return {
        FeatureClass::Building,
        FeatureClass::Road,
        FeatureClass::Water,
        FeatureClass::Vegetation,
        FeatureClass::Bare,
        FeatureClass::Unknown,
    };
}

ExtractionResult VectorImportExtractor::extract(const ExtractionContext& context) const {
    ExtractionResult result;
    result.algorithm_id = kId;
    result.params_hash = agbot::config::param_hash(context.params);

    const ImportParams params = read_params(context.params);
    if (params.path.empty()) {
        result.error_code = "params_missing_path";
        result.error_detail = "vector_import requires a 'path' parameter";
        return result;
    }

    std::ifstream input(params.path, std::ios::binary);
    if (!input) {
        result.error_code = "file_not_found";
        result.error_detail = "cannot open GeoJSON file: " + params.path;
        return result;
    }
    std::ostringstream buffer;
    buffer << input.rdbuf();
    const std::string text = buffer.str();

    const json document = json::parse(text, nullptr, /*allow_exceptions=*/false);
    if (document.is_discarded()) {
        result.error_code = "json_parse_error";
        result.error_detail = "malformed JSON in " + params.path;
        return result;
    }
    if (!document.is_object() || document.value("type", "") != "FeatureCollection" ||
        !document.contains("features") || !document["features"].is_array()) {
        result.error_code = "not_feature_collection";
        result.error_detail = "expected a GeoJSON FeatureCollection in " + params.path;
        return result;
    }

    const GeoCoordinate origin = context.aoi.center();
    const HeightResolverParams height_params{
        params.height_unit_scale_m,
        params.default_level_height_m,
        params.default_height_m,
    };

    const json& features_json = document["features"];
    std::size_t feature_ordinal = 0;
    for (const json& feature_json : features_json) {
        if (params.max_features > 0 &&
            result.features.size() >= static_cast<std::size_t>(params.max_features)) {
            break;
        }
        const std::size_t ordinal = feature_ordinal++;
        if (!feature_json.is_object() || !feature_json.contains("geometry")) {
            continue;
        }
        const json properties =
            feature_json.contains("properties") ? feature_json["properties"] : json(nullptr);

        const std::vector<PolygonRings> polygons = parse_polygons(feature_json["geometry"]);
        if (polygons.empty()) {
            continue;
        }

        const std::string class_name =
            string_property(properties, params.class_attr).value_or(params.default_class);
        const std::string base_id =
            string_property(properties, params.id_attr).value_or("f" + std::to_string(ordinal));
        const std::optional<double> raw_height = numeric_property(properties, params.height_attr);
        const std::optional<double> raw_levels = numeric_property(properties, params.levels_attr);
        const std::optional<double> raw_base = numeric_property(properties, params.base_elev_attr);
        const HeightResolution height = resolve_height(raw_height, raw_levels, height_params);

        for (std::size_t polygon_index = 0; polygon_index < polygons.size(); ++polygon_index) {
            if (params.max_features > 0 &&
                result.features.size() >= static_cast<std::size_t>(params.max_features)) {
                break;
            }
            const PolygonRings& polygon = polygons[polygon_index];
            if (!ring_intersects_aoi(polygon.exterior, context.aoi)) {
                continue;
            }

            ExtractedFeature feature;
            feature.class_name = class_name;
            feature.cls = feature_class_from_name(class_name);
            feature.exterior = simplify_ring(polygon.exterior, origin, params.simplify_tol_m);
            for (const std::vector<GeoCoordinate>& hole : polygon.holes) {
                feature.holes.push_back(simplify_ring(hole, origin, params.simplify_tol_m));
            }
            if (feature_area_m2(feature, origin) < params.min_area_m2) {
                continue;
            }

            feature.source_id =
                polygons.size() > 1 ? base_id + ":p" + std::to_string(polygon_index) : base_id;
            feature.height_m = height.height_m;
            if (raw_base.has_value()) {
                feature.base_elev_m = *raw_base * params.base_unit_scale_m;
            }
            feature.confidence = 1.0;
            feature.attributes["height_source"] = to_string(height.source);
            result.features.push_back(std::move(feature));
        }
    }

    result.ok = true;
    return result;
}

} // namespace agbot::worldgen
