#!/usr/bin/env bash
# Ingest the NYC highest-hit DSM (Digital Surface Model) as an uncompressed
# float32 GeoTIFF in lon/lat (EPSG:4326), plus a provenance sidecar. The DSM
# feeds the ranked LoD1 height stack: DSM - bare-earth DEM at a footprint
# centroid is the "measured" building height (the top tier).
#
# Unlike the 3DEP DEM, the NYC DSM is not served as a clean lon/lat exportImage:
# it ships as LAS-derived / tiled rasters referenced to NAD83(2011) State Plane
# or UTM 18N + NAVD88. This is therefore a fetch+reproject adapter at the
# compiler boundary and REQUIRES GDAL (gdalwarp) plus a source raster.
#
# Source: NYC 2017 Topobathymetric LiDAR highest-hit DSM (NYC Open Data / NYS
# GIS Clearinghouse). Vertical datum NAVD88 (GEOID18), matching the 3DEP DEM so
# the residual is taken in a single vertical frame.
#
# Usage:
#   fetch_nyc_dsm.sh <source_dsm_raster_or_vrt> [min_lat] [min_lon] [max_lat] [max_lon] [out_file]
# Defaults: Lower Manhattan AOI (40.700..40.740, -74.020..-73.980).
set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "usage: fetch_nyc_dsm.sh <source_dsm_raster_or_vrt> [min_lat min_lon max_lat max_lon] [out_file]" >&2
    echo "  <source> is the NYC DSM raster/VRT (State Plane or UTM 18N, NAVD88)." >&2
    exit 2
fi
if ! command -v gdalwarp >/dev/null 2>&1; then
    echo "ERROR: gdalwarp not found. Install GDAL to reproject the NYC DSM." >&2
    exit 3
fi

SRC="$1"
MIN_LAT="${2:-40.700}"
MIN_LON="${3:--74.020}"
MAX_LAT="${4:-40.740}"
MAX_LON="${5:--73.980}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_FILE="${6:-${SCRIPT_DIR}/../../data/terrain/manhattan_nyc_dsm.tif}"

mkdir -p "$(dirname "${OUT_FILE}")"
echo "Reprojecting NYC DSM -> EPSG:4326 float32 over ${MIN_LAT},${MIN_LON}..${MAX_LAT},${MAX_LON}" >&2
# -te is xmin ymin xmax ymax in the target CRS (lon/lat). Bilinear resampling,
# float32 output, uncompressed (matches the narrow GeoTIFF reader).
gdalwarp -overwrite -t_srs EPSG:4326 -r bilinear -ot Float32 -of GTiff \
    -co COMPRESS=NONE -te "${MIN_LON}" "${MIN_LAT}" "${MAX_LON}" "${MAX_LAT}" \
    "${SRC}" "${OUT_FILE}"

SHA256="$(shasum -a 256 "${OUT_FILE}" | awk '{print $1}')"
BYTES="$(wc -c < "${OUT_FILE}" | tr -d ' ')"
FETCHED_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

cat > "${OUT_FILE%.tif}.provenance.json" <<JSON
{
  "dataset": "NYC 2017 highest-hit DSM (LiDAR-derived surface)",
  "source_raster": "${SRC}",
  "reprojected_by": "gdalwarp -t_srs EPSG:4326 -r bilinear -ot Float32",
  "aoi": {"min_lat": ${MIN_LAT}, "min_lon": ${MIN_LON}, "max_lat": ${MAX_LAT}, "max_lon": ${MAX_LON}},
  "horizontal_crs": "EPSG:4326",
  "vertical_datum": "NAVD88",
  "units": "metres",
  "license": "NYC Open Data (public domain)",
  "fetched_utc": "${FETCHED_UTC}",
  "sha256": "${SHA256}",
  "bytes": ${BYTES}
}
JSON

echo "Wrote ${OUT_FILE} (${BYTES} bytes, sha256 ${SHA256})" >&2
echo "Wrote ${OUT_FILE%.tif}.provenance.json" >&2
