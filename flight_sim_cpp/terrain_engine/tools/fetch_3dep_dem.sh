#!/usr/bin/env bash
# Fetch an authoritative USGS 3DEP bare-earth DEM for a lat/lon AOI as an
# uncompressed float32 GeoTIFF, plus a provenance sidecar. This is the Gate 2
# ground-truth elevation source.
#
# Source: USGS 3DEPElevation ImageServer (public domain). Horizontal EPSG:4326,
# vertical NAVD88 metres. The exportImage endpoint resamples the best-available
# 3DEP product (1 m where available, e.g. New York) onto the requested grid.
#
# Usage:
#   fetch_3dep_dem.sh [min_lat] [min_lon] [max_lat] [max_lon] [size] [out_file]
# Defaults: Lower Manhattan AOI (40.700..40.740, -74.020..-73.980), 512x512.
set -euo pipefail

MIN_LAT="${1:-40.700}"
MIN_LON="${2:--74.020}"
MAX_LAT="${3:-40.740}"
MAX_LON="${4:--73.980}"
SIZE="${5:-512}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_FILE="${6:-${SCRIPT_DIR}/../../data/terrain/manhattan_3dep_dem.tif}"

SERVICE="https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer/exportImage"
# exportImage bbox order is xmin,ymin,xmax,ymax = min_lon,min_lat,max_lon,max_lat.
REQUEST_URL="${SERVICE}?bbox=${MIN_LON},${MIN_LAT},${MAX_LON},${MAX_LAT}&bboxSR=4326&imageSR=4326&size=${SIZE},${SIZE}&format=tiff&pixelType=F32&interpolation=RSP_BilinearInterpolation&f=image"

mkdir -p "$(dirname "${OUT_FILE}")"
echo "Fetching 3DEP DEM ${MIN_LAT},${MIN_LON}..${MAX_LAT},${MAX_LON} @ ${SIZE}x${SIZE}" >&2
curl -sf --max-time 90 -o "${OUT_FILE}" "${REQUEST_URL}"

# Validate it is a TIFF, not an error JSON payload.
MAGIC="$(head -c 2 "${OUT_FILE}")"
if [[ "${MAGIC}" != "II" && "${MAGIC}" != "MM" ]]; then
    echo "ERROR: response is not a TIFF (got: $(head -c 200 "${OUT_FILE}"))" >&2
    exit 1
fi

SHA256="$(shasum -a 256 "${OUT_FILE}" | awk '{print $1}')"
BYTES="$(wc -c < "${OUT_FILE}" | tr -d ' ')"
FETCHED_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

cat > "${OUT_FILE%.tif}.provenance.json" <<JSON
{
  "dataset": "USGS 3DEP bare-earth DEM",
  "service": "3DEPElevation/ImageServer/exportImage",
  "request_url": "${REQUEST_URL}",
  "aoi": {"min_lat": ${MIN_LAT}, "min_lon": ${MIN_LON}, "max_lat": ${MAX_LAT}, "max_lon": ${MAX_LON}},
  "size_px": ${SIZE},
  "horizontal_crs": "EPSG:4326",
  "vertical_datum": "NAVD88",
  "units": "metres",
  "license": "USGS 3DEP (public domain)",
  "fetched_utc": "${FETCHED_UTC}",
  "sha256": "${SHA256}",
  "bytes": ${BYTES}
}
JSON

echo "Wrote ${OUT_FILE} (${BYTES} bytes, sha256 ${SHA256})" >&2
echo "Wrote ${OUT_FILE%.tif}.provenance.json" >&2
