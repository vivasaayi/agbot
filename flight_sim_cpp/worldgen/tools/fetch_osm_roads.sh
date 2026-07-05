#!/usr/bin/env bash
# Fetch OSM road centerlines (drivable highway classes) for a lat/lon AOI via
# the Overpass API and save the raw Overpass JSON (`out geom`, so each way
# carries an ordered lat/lon geometry array plus its tags).
#
# Usage:
#   fetch_osm_roads.sh [min_lat] [min_lon] [max_lat] [max_lon] [out_file]
# Defaults: Lower Manhattan AOI (40.700..40.740, -74.020..-73.980), matching
# fetch_nyc_buildings.sh, written to data/worldgen/manhattan_roads.json
# (data/ is gitignored).
#
# Retries on Overpass overload (HTTP 429/504) with a sleep, then falls back to
# the kumi.systems mirror.
set -euo pipefail

MIN_LAT="${1:-40.700}"
MIN_LON="${2:--74.020}"
MAX_LAT="${3:-40.740}"
MAX_LON="${4:--73.980}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_FILE="${5:-${SCRIPT_DIR}/../../data/worldgen/manhattan_roads.json}"

ENDPOINTS=(
    "https://overpass-api.de/api/interpreter"
    "https://overpass.kumi.systems/api/interpreter"
)

QUERY="[out:json][timeout:120]; way[\"highway\"~\"^(motorway|trunk|primary|secondary|tertiary|residential|unclassified|living_street|service)\$\"](${MIN_LAT},${MIN_LON},${MAX_LAT},${MAX_LON}); out geom;"

mkdir -p "$(dirname "${OUT_FILE}")"
TMP_FILE="$(mktemp)"
trap 'rm -f "${TMP_FILE}"' EXIT

fetch_ok=0
for endpoint in "${ENDPOINTS[@]}"; do
    for attempt in 1 2 3; do
        echo "fetching from ${endpoint} (attempt ${attempt})" >&2
        http_code="$(curl -s -o "${TMP_FILE}" -w '%{http_code}' \
            --data-urlencode "data=${QUERY}" "${endpoint}" || echo "000")"
        if [ "${http_code}" = "200" ]; then
            fetch_ok=1
            break 2
        fi
        echo "HTTP ${http_code} from ${endpoint}; retrying in $((attempt * 15))s" >&2
        sleep $((attempt * 15))
    done
done
if [ "${fetch_ok}" -ne 1 ]; then
    echo "error: all Overpass endpoints failed" >&2
    exit 1
fi

python3 - "${TMP_FILE}" "${OUT_FILE}" <<'PY'
import json
import sys

tmp_path, out_path = sys.argv[1], sys.argv[2]
with open(tmp_path) as handle:
    doc = json.load(handle)
ways = [e for e in doc.get("elements", []) if e.get("type") == "way"]
bad = [w for w in ways if "geometry" not in w or "highway" not in w.get("tags", {})]
if len(ways) < 500:
    print(f"error: only {len(ways)} ways fetched (expected > 500)", file=sys.stderr)
    sys.exit(1)
if bad:
    print(f"error: {len(bad)} ways missing geometry or highway tag", file=sys.stderr)
    sys.exit(1)
with open(out_path, "w") as handle:
    json.dump(doc, handle)
classes = {}
for way in ways:
    cls = way["tags"]["highway"]
    classes[cls] = classes.get(cls, 0) + 1
print(f"wrote {len(ways)} ways to {out_path}")
print("highway classes:", json.dumps(classes, sort_keys=True))
PY
