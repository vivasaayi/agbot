#pragma once

#include "agbot_terrain/Raster.hpp"
#include "agbot_worldgen/Feature.hpp"

#include <cstddef>
#include <utility>
#include <vector>

// M3 batch 3 — authoritative surface-detail adapters.
//
// DSM residual: a highest-hit surface (DSM) minus the bare-earth ground gives a
// measured building height, the top tier of the ranked LoD1 height stack.
// Land-cover: a categorical raster sampled onto the terrain grid yields a
// per-class cell histogram (semantic mask evidence). Both are ingest adapters
// at the compiler boundary: they activate when the source raster is present and
// no-op cleanly when it is absent.
namespace agbot::worldgen {

struct DsmResidualParams {
    double min_height_m = 2.0;
    double max_height_m = 600.0;
};

struct DsmResidualStats {
    std::size_t applied = 0;  // buildings that received a measured height
    std::size_t rejected = 0; // residual nodata / out of range (fell through)
};

// For each building, sample the DSM and the bare-earth ground at the footprint
// centroid; the residual (DSM - ground) is the measured height. When finite and
// within [min,max], set feature.height_m and tag attributes["height_source"] =
// "measured" so the measured tier outranks the height attribute. Buildings whose
// centroid is outside either raster, or whose residual is implausible, are left
// untouched (their attribute/levels/default height stands).
DsmResidualStats apply_dsm_measured_heights(std::vector<ExtractedFeature>& buildings,
                                            const agbot::terrain::Raster& dsm,
                                            const agbot::terrain::Raster& ground,
                                            const DsmResidualParams& params = {});

// Class id -> covered cell count, sorted ascending by class id (deterministic).
using LandCoverHistogram = std::vector<std::pair<int, std::size_t>>;

// Sample the land-cover raster at every terrain-grid cell center and tally class
// ids. Cells outside the raster or nodata are counted under class -1 (unknown).
[[nodiscard]] LandCoverHistogram sample_landcover_histogram(
    const agbot::terrain::Raster& terrain, const agbot::terrain::Raster& landcover);

} // namespace agbot::worldgen
