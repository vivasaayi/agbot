#pragma once

#include "agbot_config/Params.hpp"
#include "agbot_flight_sim/GeoTerrain.hpp"
#include "agbot_flight_sim/Mission.hpp"

#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <vector>

namespace agbot::worldgen {

// Semantic class of an extracted world feature.
enum class FeatureClass {
    Building,
    Road,
    Water,
    Vegetation,
    Bare,
    Unknown,
};

[[nodiscard]] const char* to_string(FeatureClass cls);

// Maps a free-form class name ("building", "water", ...) onto a FeatureClass.
[[nodiscard]] FeatureClass feature_class_from_name(const std::string& class_name);

// One vector feature extracted from a source layer. Rings are geodetic
// (lat/lon); the exterior ring is stored without a duplicated closing point.
struct ExtractedFeature {
    FeatureClass cls = FeatureClass::Unknown;
    std::string class_name;
    std::vector<agbot::flight_sim::GeoCoordinate> exterior;
    std::vector<std::vector<agbot::flight_sim::GeoCoordinate>> holes;
    std::optional<double> height_m;
    std::optional<double> base_elev_m;
    double confidence = 1.0;
    std::string source_id;
    std::map<std::string, std::string> attributes;
};

// Input to a feature extractor: area of interest plus strategy parameters.
struct ExtractionContext {
    agbot::flight_sim::GeoBounds aoi;
    const agbot::config::ParamTable& params;
};

// Reason-coded extraction outcome. `ok == false` carries a stable machine
// readable `error_code` plus a human readable `error_detail`; no exceptions
// cross the extractor API boundary.
struct ExtractionResult {
    bool ok = false;
    std::string error_code;
    std::string error_detail;
    std::vector<ExtractedFeature> features;
    std::string algorithm_id;
    std::uint64_t params_hash = 0;
};

// Planar shoelace area (m^2, holes subtracted) of a feature, computed in
// local meters around `origin` via local_from_geo. Free of spherical excess.
[[nodiscard]] double feature_area_m2(
    const ExtractedFeature& feature,
    const agbot::flight_sim::GeoCoordinate& origin);

[[nodiscard]] double ring_area_m2(
    const std::vector<agbot::flight_sim::GeoCoordinate>& ring,
    const agbot::flight_sim::GeoCoordinate& origin);

} // namespace agbot::worldgen
