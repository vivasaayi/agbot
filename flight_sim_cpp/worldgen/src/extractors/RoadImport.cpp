#include "agbot_worldgen/extractors/RoadImport.hpp"

#include <nlohmann/json.hpp>

#include <algorithm>
#include <cstdint>
#include <fstream>
#include <set>
#include <sstream>
#include <string>
#include <vector>

namespace agbot::worldgen {
namespace {

using agbot::flight_sim::GeoBounds;
using agbot::flight_sim::GeoCoordinate;
using nlohmann::json;

const std::set<std::string> kDefaultHighwayFilter = {
    "motorway",       "trunk",        "primary", "secondary", "tertiary",
    "residential",    "unclassified", "living_street", "service",
};

struct RoadImportParams {
    std::string path;
    std::set<std::string> highway_filter = kDefaultHighwayFilter;
    std::int64_t min_points = 2;
    std::int64_t max_features = 0;
};

RoadImportParams read_params(const agbot::config::ParamTable& table) {
    namespace cfg = agbot::config;
    RoadImportParams params;
    params.path = cfg::string_or(table, "path", "");
    if (const cfg::ParamArray* filter = cfg::find_array(table, "highway_filter")) {
        params.highway_filter.clear();
        for (const cfg::ParamValue& entry : *filter) {
            if (entry.is_string()) {
                params.highway_filter.insert(entry.as_string());
            }
        }
    }
    params.min_points = std::max<std::int64_t>(2, cfg::integer_or(table, "min_points", 2));
    params.max_features = cfg::integer_or(table, "max_features", 0);
    return params;
}

bool polyline_intersects_aoi(const std::vector<GeoCoordinate>& polyline, const GeoBounds& aoi) {
    double min_lat = polyline.front().latitude;
    double max_lat = polyline.front().latitude;
    double min_lon = polyline.front().longitude;
    double max_lon = polyline.front().longitude;
    for (const GeoCoordinate& point : polyline) {
        min_lat = std::min(min_lat, point.latitude);
        max_lat = std::max(max_lat, point.latitude);
        min_lon = std::min(min_lon, point.longitude);
        max_lon = std::max(max_lon, point.longitude);
    }
    return !(max_lat < aoi.min_latitude || min_lat > aoi.max_latitude ||
             max_lon < aoi.min_longitude || min_lon > aoi.max_longitude);
}

// Normalizes an OSM oneway tag value onto "yes" | "-1" | "no".
std::string normalize_oneway(const std::string& raw) {
    if (raw == "yes" || raw == "true" || raw == "1") {
        return "yes";
    }
    if (raw == "-1" || raw == "reverse") {
        return "-1";
    }
    return "no";
}

std::string tag_string(const json& tags, const char* key) {
    if (!tags.is_object()) {
        return "";
    }
    const auto it = tags.find(key);
    if (it == tags.end()) {
        return "";
    }
    if (it->is_string()) {
        return it->get<std::string>();
    }
    if (it->is_number_integer()) {
        return std::to_string(it->get<std::int64_t>());
    }
    return "";
}

struct ParsedRoad {
    std::vector<GeoCoordinate> polyline;
    std::string highway;
    std::string name;
    std::string oneway;
    std::string lanes;
    std::string way_id;
};

void append_road(
    std::vector<ParsedRoad>& roads,
    std::vector<GeoCoordinate> polyline,
    const json& tags,
    const std::string& way_id) {
    ParsedRoad road;
    road.polyline = std::move(polyline);
    road.highway = tag_string(tags, "highway");
    road.name = tag_string(tags, "name");
    road.oneway = normalize_oneway(tag_string(tags, "oneway"));
    road.lanes = tag_string(tags, "lanes");
    road.way_id = way_id;
    roads.push_back(std::move(road));
}

// Overpass `out geom`: elements[] with type=="way", tags{}, geometry[{lat,lon}].
std::vector<ParsedRoad> parse_overpass(const json& document) {
    std::vector<ParsedRoad> roads;
    const json& elements = document["elements"];
    if (!elements.is_array()) {
        return roads;
    }
    for (const json& element : elements) {
        if (!element.is_object() || element.value("type", "") != "way" ||
            !element.contains("geometry") || !element["geometry"].is_array()) {
            continue;
        }
        std::vector<GeoCoordinate> polyline;
        polyline.reserve(element["geometry"].size());
        bool valid = true;
        for (const json& point : element["geometry"]) {
            if (!point.is_object() || !point.contains("lat") || !point.contains("lon") ||
                !point["lat"].is_number() || !point["lon"].is_number()) {
                valid = false;
                break;
            }
            polyline.push_back({point["lat"].get<double>(), point["lon"].get<double>(), 0.0});
        }
        if (!valid || polyline.empty()) {
            continue;
        }
        std::string way_id;
        if (element.contains("id") && element["id"].is_number_integer()) {
            way_id = std::to_string(element["id"].get<std::int64_t>());
        }
        const json tags = element.contains("tags") ? element["tags"] : json(nullptr);
        append_road(roads, std::move(polyline), tags, way_id);
    }
    return roads;
}

std::vector<GeoCoordinate> parse_geojson_line(const json& coordinates) {
    std::vector<GeoCoordinate> polyline;
    if (!coordinates.is_array()) {
        return polyline;
    }
    polyline.reserve(coordinates.size());
    for (const json& point : coordinates) {
        if (!point.is_array() || point.size() < 2 || !point[0].is_number() ||
            !point[1].is_number()) {
            return {};
        }
        polyline.push_back({point[1].get<double>(), point[0].get<double>(), 0.0});
    }
    return polyline;
}

// GeoJSON FeatureCollection with LineString/MultiLineString geometries;
// highway/name/oneway/lanes are read from feature properties.
std::vector<ParsedRoad> parse_geojson(const json& document) {
    std::vector<ParsedRoad> roads;
    const json& features = document["features"];
    if (!features.is_array()) {
        return roads;
    }
    std::size_t ordinal = 0;
    for (const json& feature : features) {
        const std::size_t feature_ordinal = ordinal++;
        if (!feature.is_object() || !feature.contains("geometry") ||
            !feature["geometry"].is_object()) {
            continue;
        }
        const json& geometry = feature["geometry"];
        const std::string type = geometry.value("type", "");
        if (!geometry.contains("coordinates")) {
            continue;
        }
        const json properties =
            feature.contains("properties") ? feature["properties"] : json(nullptr);
        std::string way_id = tag_string(properties, "way_id");
        if (way_id.empty()) {
            way_id = "g" + std::to_string(feature_ordinal);
        }
        if (type == "LineString") {
            std::vector<GeoCoordinate> polyline = parse_geojson_line(geometry["coordinates"]);
            if (!polyline.empty()) {
                append_road(roads, std::move(polyline), properties, way_id);
            }
        } else if (type == "MultiLineString" && geometry["coordinates"].is_array()) {
            std::size_t part = 0;
            for (const json& line : geometry["coordinates"]) {
                std::vector<GeoCoordinate> polyline = parse_geojson_line(line);
                if (!polyline.empty()) {
                    append_road(roads, std::move(polyline), properties,
                                way_id + ":l" + std::to_string(part));
                }
                ++part;
            }
        }
    }
    return roads;
}

} // namespace

std::string RoadImportExtractor::id() const {
    return kId;
}

std::vector<FeatureClass> RoadImportExtractor::produces() const {
    return {FeatureClass::Road};
}

ExtractionResult RoadImportExtractor::extract(const ExtractionContext& context) const {
    ExtractionResult result;
    result.algorithm_id = kId;
    result.params_hash = agbot::config::param_hash(context.params);

    const RoadImportParams params = read_params(context.params);
    if (params.path.empty()) {
        result.error_code = "params_missing_path";
        result.error_detail = "road_import requires a 'path' parameter";
        return result;
    }

    std::ifstream input(params.path, std::ios::binary);
    if (!input) {
        result.error_code = "file_not_found";
        result.error_detail = "cannot open road JSON file: " + params.path;
        return result;
    }
    std::ostringstream buffer;
    buffer << input.rdbuf();

    const json document = json::parse(buffer.str(), nullptr, /*allow_exceptions=*/false);
    if (document.is_discarded()) {
        result.error_code = "json_parse_error";
        result.error_detail = "malformed JSON in " + params.path;
        return result;
    }
    if (!document.is_object()) {
        result.error_code = "unsupported_road_format";
        result.error_detail = "expected an Overpass or GeoJSON object in " + params.path;
        return result;
    }

    std::vector<ParsedRoad> roads;
    if (document.contains("elements")) {
        roads = parse_overpass(document);
    } else if (document.contains("features")) {
        roads = parse_geojson(document);
    } else {
        result.error_code = "unsupported_road_format";
        result.error_detail =
            "expected 'elements' (Overpass) or 'features' (GeoJSON) in " + params.path;
        return result;
    }

    std::size_t ordinal = 0;
    for (ParsedRoad& road : roads) {
        if (params.max_features > 0 &&
            result.features.size() >= static_cast<std::size_t>(params.max_features)) {
            break;
        }
        const std::size_t road_ordinal = ordinal++;
        if (road.highway.empty() || params.highway_filter.count(road.highway) == 0) {
            continue;
        }
        if (road.polyline.size() < static_cast<std::size_t>(params.min_points)) {
            continue;
        }
        if (!polyline_intersects_aoi(road.polyline, context.aoi)) {
            continue;
        }

        ExtractedFeature feature;
        feature.cls = FeatureClass::Road;
        feature.class_name = "road";
        feature.exterior = std::move(road.polyline);
        feature.confidence = 1.0;
        feature.source_id =
            road.way_id.empty() ? "r" + std::to_string(road_ordinal) : "way:" + road.way_id;
        feature.attributes["geometry_type"] = "polyline";
        feature.attributes["highway"] = road.highway;
        feature.attributes["oneway"] = road.oneway;
        if (!road.lanes.empty()) {
            feature.attributes["lanes"] = road.lanes;
        }
        if (!road.name.empty()) {
            feature.attributes["name"] = road.name;
        }
        if (!road.way_id.empty()) {
            feature.attributes["way_id"] = road.way_id;
        }
        result.features.push_back(std::move(feature));
    }

    result.ok = true;
    return result;
}

} // namespace agbot::worldgen
