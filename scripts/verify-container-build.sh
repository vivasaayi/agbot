#!/usr/bin/env bash
set -euo pipefail

dockerfile="${1:-Dockerfile}"
failures=0

fail() {
    echo "container-build-check: $*" >&2
    failures=$((failures + 1))
}

if [[ ! -f "$dockerfile" ]]; then
    fail "Dockerfile not found: $dockerfile"
    exit 1
fi

while read -r image; do
    if [[ "$image" != *:* ]]; then
        fail "base image is missing an explicit tag: $image"
        continue
    fi
    if [[ "$image" =~ :(latest|stable|bookworm-slim|bullseye-slim|slim)$ ]]; then
        fail "base image uses a floating tag: $image"
    fi
done < <(awk 'toupper($1) == "FROM" { print $2 }' "$dockerfile")

required_bins=(
    mission_control
    sensor_collector
    imagery_processor
    lidar_mapper
    ground_station_ui
)

for bin in "${required_bins[@]}"; do
    if ! grep -Eq -- "--bin[[:space:]]+$bin([[:space:]\\]|$)" "$dockerfile"; then
        fail "Dockerfile does not build required binary: $bin"
    fi
    if ! grep -Eq -- "COPY --from=builder .*/$bin[[:space:]]+/opt/agrodrone/bin/" "$dockerfile"; then
        fail "Dockerfile does not copy required binary into runtime image: $bin"
    fi
done

if grep -Eq '^[[:space:]]*COPY[[:space:]]+\.env[[:space:]]' "$dockerfile"; then
    fail "Dockerfile copies .env into the runtime image"
fi

if ! grep -q "build-manifest.json" "$dockerfile"; then
    fail "Dockerfile does not write /opt/agrodrone/build-manifest.json"
fi

if ! grep -q "USER agrodrone" "$dockerfile"; then
    fail "Dockerfile does not switch to the non-root agrodrone user"
fi

if (( failures > 0 )); then
    exit 1
fi

echo "container-build-check: Dockerfile pins and runtime manifest validated"
