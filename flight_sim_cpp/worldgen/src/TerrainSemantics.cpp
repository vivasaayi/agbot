#include "agbot_worldgen/TerrainSemantics.hpp"

#include <cmath>
#include <map>

namespace agbot::worldgen {

namespace {

// Vertex-average centroid of a ring in geographic coordinates. Adequate as a
// footprint sample point for the compact, roughly convex NYC footprints; L- and
// U-shaped lots may sample slightly off-center but stay within the block.
agbot::flight_sim::GeoCoordinate ring_centroid(
    const std::vector<agbot::flight_sim::GeoCoordinate>& ring) {
    double lat = 0.0;
    double lon = 0.0;
    for (const auto& c : ring) {
        lat += c.latitude;
        lon += c.longitude;
    }
    const double n = static_cast<double>(ring.size());
    return {lat / n, lon / n, 0.0};
}

} // namespace

DsmResidualStats apply_dsm_measured_heights(std::vector<ExtractedFeature>& buildings,
                                            const agbot::terrain::Raster& dsm,
                                            const agbot::terrain::Raster& ground,
                                            const DsmResidualParams& params) {
    DsmResidualStats stats;
    for (ExtractedFeature& b : buildings) {
        if (b.exterior.size() < 3) {
            continue;
        }
        const auto centroid = ring_centroid(b.exterior);
        const std::optional<float> surf = dsm.sample_at(centroid.latitude, centroid.longitude);
        const std::optional<float> base = ground.sample_at(centroid.latitude, centroid.longitude);
        if (!surf.has_value() || !base.has_value()) {
            ++stats.rejected;
            continue;
        }
        const double residual = static_cast<double>(*surf) - static_cast<double>(*base);
        if (!std::isfinite(residual) || residual < params.min_height_m ||
            residual > params.max_height_m) {
            ++stats.rejected;
            continue;
        }
        b.height_m = residual;
        b.attributes["height_source"] = "measured";
        ++stats.applied;
    }
    return stats;
}

LandCoverHistogram sample_landcover_histogram(const agbot::terrain::Raster& terrain,
                                              const agbot::terrain::Raster& landcover) {
    std::map<int, std::size_t> tally;
    if (!terrain.valid() || !landcover.valid()) {
        return {};
    }
    const agbot::flight_sim::GeoBounds& tb = terrain.bounds;
    const agbot::flight_sim::GeoBounds& lb = landcover.bounds;
    const double lat_span = tb.max_latitude - tb.min_latitude;
    const double lon_span = tb.max_longitude - tb.min_longitude;
    const double lc_lat_span = lb.max_latitude - lb.min_latitude;
    const double lc_lon_span = lb.max_longitude - lb.min_longitude;

    for (int r = 0; r < terrain.height; ++r) {
        // Row 0 is the northernmost row (matches Raster orientation).
        const double lat =
            tb.max_latitude - (static_cast<double>(r) + 0.5) / terrain.height * lat_span;
        for (int c = 0; c < terrain.width; ++c) {
            const double lon =
                tb.min_longitude + (static_cast<double>(c) + 0.5) / terrain.width * lon_span;
            int lc_col = lc_lon_span > 0.0
                             ? static_cast<int>(std::floor((lon - lb.min_longitude) / lc_lon_span *
                                                           landcover.width))
                             : -1;
            int lc_row = lc_lat_span > 0.0
                             ? static_cast<int>(std::floor((lb.max_latitude - lat) / lc_lat_span *
                                                           landcover.height))
                             : -1;
            int cls = -1; // unknown / outside coverage
            if (lc_col >= 0 && lc_col < landcover.width && lc_row >= 0 &&
                lc_row < landcover.height) {
                const float v = landcover.at(lc_row, lc_col);
                if (!agbot::terrain::Raster::is_nodata(v)) {
                    cls = static_cast<int>(std::lround(v));
                }
            }
            ++tally[cls];
        }
    }
    return LandCoverHistogram(tally.begin(), tally.end());
}

} // namespace agbot::worldgen
