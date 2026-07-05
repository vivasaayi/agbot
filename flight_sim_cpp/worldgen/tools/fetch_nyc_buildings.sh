#!/usr/bin/env bash
# Fetch NYC building footprints (Socrata dataset 5zhs-2jue, "Building Footprints")
# for a lat/lon AOI and merge paginated GeoJSON pages into one FeatureCollection.
#
# Actual Socrata attribute names (verified 2026-07-01):
#   height_roof       roof height above ground, FEET, serialized as string
#   ground_elevation  ground elevation at base, FEET, serialized as string
#   bin               building identification number
#   doitt_id          DOITT feature id
#   base_bbl, mappluto_bbl, construction_year, feature_code, geom_source
#
# Usage:
#   fetch_nyc_buildings.sh [min_lat] [min_lon] [max_lat] [max_lon] [out_file]
# Defaults: Lower Manhattan AOI (40.700..40.740, -74.020..-73.980). The tighter
# financial-district-only box (40.700..40.725, -74.020..-73.995) holds only
# ~534 footprints (few, large towers), so the default extends north to
# Tribeca/Chinatown to cover ~2200 buildings.
set -euo pipefail

MIN_LAT="${1:-40.700}"
MIN_LON="${2:--74.020}"
MAX_LAT="${3:-40.740}"
MAX_LON="${4:--73.980}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_FILE="${5:-${SCRIPT_DIR}/../../data/worldgen/manhattan_buildings.geojson}"

BASE_URL="https://data.cityofnewyork.us/resource/5zhs-2jue.geojson"
# within_box(location_col, NW_lat, NW_lon, SE_lat, SE_lon)
WHERE="within_box(the_geom,${MAX_LAT},${MIN_LON},${MIN_LAT},${MAX_LON})"
PAGE_SIZE=10000

mkdir -p "$(dirname "${OUT_FILE}")"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "${WORK_DIR}"' EXIT

offset=0
page=0
while :; do
    page_file="${WORK_DIR}/page_${page}.geojson"
    curl -sf -G "${BASE_URL}" \
        --data-urlencode "\$where=${WHERE}" \
        --data-urlencode "\$limit=${PAGE_SIZE}" \
        --data-urlencode "\$offset=${offset}" \
        -o "${page_file}"
    count="$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["features"]))' "${page_file}")"
    echo "page ${page}: offset=${offset} features=${count}" >&2
    if [ "${count}" -eq 0 ]; then
        rm -f "${page_file}"
        break
    fi
    offset=$((offset + count))
    page=$((page + 1))
    if [ "${count}" -lt "${PAGE_SIZE}" ]; then
        break
    fi
done

python3 - "${OUT_FILE}" "${WORK_DIR}"/page_*.geojson <<'PY'
import json
import sys

out_path = sys.argv[1]
features = []
for path in sys.argv[2:]:
    with open(path) as handle:
        features.extend(json.load(handle)["features"])
with open(out_path, "w") as handle:
    json.dump({"type": "FeatureCollection", "features": features}, handle)
print(f"wrote {len(features)} features to {out_path}")
PY
