#!/usr/bin/env bash
# Ingest the NYC 6-inch Land Cover as a single-band integer GeoTIFF in lon/lat
# (EPSG:4326), plus a provenance sidecar. Sampled onto the terrain grid it
# yields a per-class cell histogram (semantic-mask evidence) in the manifest.
#
# The NYC land cover is 6-inch, object-based-image-analysis classified, and
# referenced to NY State Plane (EPSG:2263). It is categorical, so this is a
# fetch+reproject adapter that REQUIRES GDAL (gdalwarp) with NEAREST resampling
# (never interpolate class ids) plus a source raster.
#
# Classes (NYC 2017 LC): 1 tree canopy, 2 grass/shrub, 3 bare soil, 4 water,
# 5 buildings, 6 roads, 7 other impervious, 8 railroads.
#
# Usage:
#   fetch_nyc_landcover.sh <source_landcover_raster_or_vrt> [min_lat min_lon max_lat max_lon] [out_file]
# Defaults: Lower Manhattan AOI (40.700..40.740, -74.020..-73.980).
set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "usage: fetch_nyc_landcover.sh <source_landcover_raster_or_vrt> [min_lat min_lon max_lat max_lon] [out_file]" >&2
    echo "  <source> is the NYC 6-inch land-cover raster/VRT (EPSG:2263, class ids)." >&2
    exit 2
fi
if ! command -v gdalwarp >/dev/null 2>&1; then
    echo "ERROR: gdalwarp not found. Install GDAL to reproject the NYC land cover." >&2
    exit 3
fi

SRC="$1"
MIN_LAT="${2:-40.700}"
MIN_LON="${3:--74.020}"
MAX_LAT="${4:-40.740}"
MAX_LON="${5:--73.980}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_FILE="${6:-${SCRIPT_DIR}/../../data/terrain/manhattan_nyc_landcover.tif}"
# Resolution in degrees; ~0.000018 deg ~= 2 m at this latitude (the classified
# raster is 6-inch native, downsampled here to a tractable grid).
RES="${AGBOT_LC_RES_DEG:-0.00002}"

mkdir -p "$(dirname "${OUT_FILE}")"
echo "Reprojecting NYC land cover -> EPSG:4326 uint8 (nearest) over ${MIN_LAT},${MIN_LON}..${MAX_LAT},${MAX_LON}" >&2
# NEAREST resampling preserves categorical class ids. Byte output, uncompressed
# (matches the narrow categorical GeoTIFF reader).
gdalwarp -overwrite -t_srs EPSG:4326 -r near -ot Byte -of GTiff \
    -co COMPRESS=NONE -tr "${RES}" "${RES}" \
    -te "${MIN_LON}" "${MIN_LAT}" "${MAX_LON}" "${MAX_LAT}" \
    "${SRC}" "${OUT_FILE}"

SHA256="$(shasum -a 256 "${OUT_FILE}" | awk '{print $1}')"
BYTES="$(wc -c < "${OUT_FILE}" | tr -d ' ')"
FETCHED_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

cat > "${OUT_FILE%.tif}.provenance.json" <<JSON
{
  "dataset": "NYC 2017 6-inch Land Cover (object-based classification)",
  "source_raster": "${SRC}",
  "reprojected_by": "gdalwarp -t_srs EPSG:4326 -r near -ot Byte",
  "aoi": {"min_lat": ${MIN_LAT}, "min_lon": ${MIN_LON}, "max_lat": ${MAX_LAT}, "max_lon": ${MAX_LON}},
  "horizontal_crs": "EPSG:4326",
  "resample": "nearest (categorical)",
  "classes": {"1": "tree_canopy", "2": "grass_shrub", "3": "bare_soil", "4": "water", "5": "buildings", "6": "roads", "7": "other_impervious", "8": "railroads"},
  "license": "NYC Open Data (public domain)",
  "fetched_utc": "${FETCHED_UTC}",
  "sha256": "${SHA256}",
  "bytes": ${BYTES}
}
JSON

echo "Wrote ${OUT_FILE} (${BYTES} bytes, sha256 ${SHA256})" >&2
echo "Wrote ${OUT_FILE%.tif}.provenance.json" >&2
