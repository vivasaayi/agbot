#pragma once

#include "agbot_worldgen/FeatureExtractor.hpp"

namespace agbot::worldgen {

// Imports vector features from a GeoJSON FeatureCollection on disk.
//
// Parameters (all read from ExtractionContext::params):
//   path                    string  required; GeoJSON file path
//   height_attr             string  property carrying roof height ("" = off)
//   height_units            string  "feet" | "meters" (default "meters")
//   base_elev_attr          string  property carrying base elevation ("" = off)
//   base_units              string  "feet" | "meters" (default "meters")
//   levels_attr             string  property carrying storey count ("" = off)
//   default_level_height_m  float   metres per storey (default 3.0)
//   default_height_m        float   height when attr and levels absent (default 3.0)
//   class_attr              string  property carrying the class name ("" = off)
//   default_class           string  class when class_attr absent (default "building")
//   id_attr                 string  property used as source_id ("" = ordinal ids)
//   min_area_m2             float   drop smaller footprints (default 10)
//   simplify_tol_m          float   Douglas-Peucker tolerance, 0 = off (default 0)
//   max_features            int     0 = unlimited (default 0)
//
// Polygon and MultiPolygon geometries are supported; each polygon of a
// MultiPolygon becomes its own feature with holes preserved. Features whose
// bounding box lies fully outside the AOI are dropped; no clipping happens.
class VectorImportExtractor final : public FeatureExtractor {
public:
    static constexpr const char* kId = "vector_import";

    [[nodiscard]] std::string id() const override;
    [[nodiscard]] std::vector<FeatureClass> produces() const override;
    [[nodiscard]] ExtractionResult extract(const ExtractionContext& context) const override;
};

} // namespace agbot::worldgen
