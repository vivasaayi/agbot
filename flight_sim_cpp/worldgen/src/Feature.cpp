#include "agbot_worldgen/Feature.hpp"

#include <cmath>

namespace agbot::worldgen {

const char* to_string(FeatureClass cls) {
    switch (cls) {
        case FeatureClass::Building:
            return "building";
        case FeatureClass::Road:
            return "road";
        case FeatureClass::Water:
            return "water";
        case FeatureClass::Vegetation:
            return "vegetation";
        case FeatureClass::Bare:
            return "bare";
        case FeatureClass::Unknown:
            return "unknown";
    }
    return "unknown";
}

FeatureClass feature_class_from_name(const std::string& class_name) {
    if (class_name == "building" || class_name == "buildings") {
        return FeatureClass::Building;
    }
    if (class_name == "road" || class_name == "highway" || class_name == "street") {
        return FeatureClass::Road;
    }
    if (class_name == "water" || class_name == "river" || class_name == "lake") {
        return FeatureClass::Water;
    }
    if (class_name == "vegetation" || class_name == "tree" || class_name == "forest" ||
        class_name == "grass" || class_name == "crop") {
        return FeatureClass::Vegetation;
    }
    if (class_name == "bare" || class_name == "bare_earth" || class_name == "soil") {
        return FeatureClass::Bare;
    }
    return FeatureClass::Unknown;
}

double ring_area_m2(
    const std::vector<agbot::flight_sim::GeoCoordinate>& ring,
    const agbot::flight_sim::GeoCoordinate& origin) {
    if (ring.size() < 3) {
        return 0.0;
    }
    double doubled_area = 0.0;
    agbot::flight_sim::Vec3 previous = agbot::flight_sim::local_from_geo(ring.back(), origin);
    for (const agbot::flight_sim::GeoCoordinate& coordinate : ring) {
        const agbot::flight_sim::Vec3 current = agbot::flight_sim::local_from_geo(coordinate, origin);
        doubled_area += previous.x * current.z - current.x * previous.z;
        previous = current;
    }
    return std::abs(doubled_area) * 0.5;
}

double feature_area_m2(
    const ExtractedFeature& feature,
    const agbot::flight_sim::GeoCoordinate& origin) {
    double area = ring_area_m2(feature.exterior, origin);
    for (const std::vector<agbot::flight_sim::GeoCoordinate>& hole : feature.holes) {
        area -= ring_area_m2(hole, origin);
    }
    return std::max(0.0, area);
}

} // namespace agbot::worldgen
