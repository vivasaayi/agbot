#pragma once

#include "agbot_worldgen/FeatureExtractor.hpp"

namespace agbot::worldgen {

// Imports road centerlines from an OSM Overpass JSON dump (`out geom`) or a
// GeoJSON FeatureCollection of LineString/MultiLineString features. The input
// format is detected by the top-level key: "elements" selects Overpass,
// "features" selects GeoJSON.
//
// Parameters (all read from ExtractionContext::params):
//   path            string  required; JSON file path
//   highway_filter  array   accepted highway classes (default: motorway,
//                           trunk, primary, secondary, tertiary, residential,
//                           unclassified, living_street, service)
//   min_points      int     drop polylines with fewer vertices (default 2)
//   max_features    int     0 = unlimited (default 0)
//
// Emitted features use cls = FeatureClass::Road with `exterior` holding an
// OPEN POLYLINE (ordered centerline vertices), not a closed ring; consumers
// must check attributes["geometry_type"] == "polyline" before treating
// `exterior` as a ring. Additional attributes: "highway", "oneway"
// (normalized to "yes" | "-1" | "no"), "lanes" (when tagged), "name" (when
// tagged), and "way_id". Polylines whose bounding box lies fully outside the
// AOI are dropped; no clipping happens.
class RoadImportExtractor final : public FeatureExtractor {
public:
    static constexpr const char* kId = "road_import";

    [[nodiscard]] std::string id() const override;
    [[nodiscard]] std::vector<FeatureClass> produces() const override;
    [[nodiscard]] ExtractionResult extract(const ExtractionContext& context) const override;
};

} // namespace agbot::worldgen
