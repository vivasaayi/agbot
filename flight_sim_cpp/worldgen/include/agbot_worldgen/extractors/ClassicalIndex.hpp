#pragma once

#include "agbot_worldgen/FeatureExtractor.hpp"

namespace agbot::worldgen {

// Extracts Vegetation and Water polygons from RGB(A) imagery via classical
// pseudo-indices (no NIR band available). The imagery is assumed to span the
// context AOI exactly (row 0 = north edge).
//
// Parameters (all read from ExtractionContext::params):
//   imagery_path   string  required; PNG file georeferenced to the AOI
//   veg_method     string  "exg" (default) | "green_ratio"
//                          exg:          2g - r - b > veg_thresh over
//                                        chromatic coords r+g+b = 1
//                          green_ratio:  g/(r+g+b) > veg_thresh
//   veg_thresh     float   default 0.08 (exg); use ~0.40 for green_ratio
//   water_method   string  "blueness" (default)
//                          blueness: (B - max(R, G)) / 255 > water_thresh
//   water_thresh   float   default 0.05
//   morph_open_px  int     binary opening radius in pixels (erosion then
//                          dilation, square structuring element); 0 = off
//   min_area_m2    float   drop smaller blobs (default 10)
//   simplify_tol_m float   Douglas-Peucker tolerance, 0 = off (default 0)
//   confidence     float   confidence assigned to output features
//                          (default 0.6; classical masks carry no
//                          per-pixel probability)
//
// Vegetation wins where both tests fire. Output order is deterministic:
// vegetation blobs first, then water, each in scanline discovery order.
class ClassicalIndexExtractor final : public FeatureExtractor {
public:
    static constexpr const char* kId = "classical_index";

    [[nodiscard]] std::string id() const override;
    [[nodiscard]] std::vector<FeatureClass> produces() const override;
    [[nodiscard]] ExtractionResult extract(const ExtractionContext& context) const override;
};

} // namespace agbot::worldgen
