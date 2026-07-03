#pragma once

#include "agbot_flight_sim/Mission.hpp"

#include <string>

namespace agbot::worldgen {

using agbot::flight_sim::GeoCoordinate;

// Horizontal CRSs the compiler can ingest and normalize to WGS84 lon/lat.
enum class HorizontalCrs {
    Wgs84Lonlat,                // EPSG:4326  (lon/lat degrees)
    StatePlaneNyLongIslandFt,   // EPSG:2263  (NAD83 NY Long Island, US survey feet)
    Utm18N,                     // EPSG:26918 (NAD83 / UTM zone 18N, metres)
    Unknown,
};

// Parses an EPSG code ("EPSG:2263", "2263", case-insensitive "epsg:2263") into
// a HorizontalCrs. Returns Unknown for unrecognised codes.
[[nodiscard]] HorizontalCrs horizontal_crs_from_epsg(const std::string& code);

// Canonical EPSG string for a HorizontalCrs ("EPSG:4326" for Unknown).
[[nodiscard]] const char* epsg_for(HorizontalCrs crs);

// Vertical datum discipline. Orthometric (NAVD88 family) heights and
// ellipsoidal (NAD83) heights must never be mixed silently.
enum class VerticalDatum {
    Unknown,          // undeclared
    None,             // source carries no z
    Navd88,           // NAVD88 orthometric
    Navd88Geoid18,    // NAVD88 realised via GEOID18 (NOAA republished LiDAR)
    Ellipsoidal,      // NAD83 ellipsoidal height
};

[[nodiscard]] VerticalDatum vertical_datum_from_name(const std::string& name);
[[nodiscard]] const char* to_string(VerticalDatum datum);

// True when two vertical datums may be combined without an explicit transform:
// either side Unknown/None, an exact match, or two NAVD88-family realisations.
// Orthometric vs ellipsoidal is always incompatible.
[[nodiscard]] bool vertical_datums_compatible(VerticalDatum a, VerticalDatum b);

// A projected planar coordinate in metres (x = easting, y = northing).
struct ProjXY {
    double x = 0.0;
    double y = 0.0;
};

// EPSG:2263 — NAD83 / New York Long Island (US survey feet), Lambert Conformal
// Conic (2SP) on the GRS80 ellipsoid.
[[nodiscard]] GeoCoordinate wgs84_from_state_plane_li_ft(double easting_ft, double northing_ft);
void state_plane_li_ft_from_wgs84(const GeoCoordinate& coordinate,
                                  double& easting_ft, double& northing_ft);

// EPSG:26918 — NAD83 / UTM zone 18N (metres), Transverse Mercator on GRS80.
[[nodiscard]] GeoCoordinate wgs84_from_utm18n(const ProjXY& metres);
[[nodiscard]] ProjXY utm18n_from_wgs84(const GeoCoordinate& coordinate);

// Normalises a source coordinate whose components are (component0, component1)
// as stored by GeoJSON ([x, y]) in `source` CRS into WGS84 lon/lat. For
// Wgs84Lonlat this is (lon, lat) -> GeoCoordinate; for projected CRSs it is
// (easting, northing) -> inverse projection. Unknown falls back to lon/lat.
[[nodiscard]] GeoCoordinate wgs84_from_source(HorizontalCrs source,
                                              double component0, double component1);

} // namespace agbot::worldgen
