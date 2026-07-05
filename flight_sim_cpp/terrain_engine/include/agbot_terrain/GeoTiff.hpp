#pragma once

#include "agbot_terrain/Raster.hpp"

#include <filesystem>
#include <string>

namespace agbot::terrain {

// Reason-coded result of reading a GeoTIFF DEM into a Raster.
struct GeoTiffResult {
    bool ok = false;
    std::string error;   // e.g. "geotiff_not_found", "geotiff_unsupported_compression"
    Raster raster;       // row 0 = northernmost row; nodata cells hold Raster::nodata()
    bool has_georef = false;
};

// Reads a single-band floating-point GeoTIFF (BitsPerSample 32 or 64,
// SampleFormat = IEEE float, Compression = none). Supports both tiled and
// stripped layouts and little/big-endian byte order. Georeferencing is taken
// from ModelPixelScale (33550) + ModelTiepoint (33922); GDAL_NODATA (42113) or
// values below -1e30 become Raster::nodata().
//
// This is deliberately narrow: it targets the uncompressed F32 GeoTIFF that the
// USGS 3DEP ImageServer exportImage endpoint returns, not the full TIFF spec.
[[nodiscard]] GeoTiffResult read_geotiff_dem(const std::filesystem::path& path);

// As read_geotiff_dem, but also accepts single-band unsigned/signed integer
// sample formats (8/16/32-bit), decoding class ids into the float Raster. Used
// for categorical rasters such as a land-cover class grid delivered as a lon/lat
// integer GeoTIFF at the compiler boundary.
[[nodiscard]] GeoTiffResult read_geotiff_categorical(const std::filesystem::path& path);

} // namespace agbot::terrain
