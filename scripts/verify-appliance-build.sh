#!/usr/bin/env bash
# Validate the geo_hub appliance stage of the Dockerfile. Static checks run
# with no dependencies; pass --build to additionally build the appliance image
# (and, when possible, smoke-boot it) using the local Docker daemon.
set -euo pipefail

dockerfile="Dockerfile"
do_build=0
for arg in "$@"; do
    case "$arg" in
        --build) do_build=1 ;;
        *) dockerfile="$arg" ;;
    esac
done

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
dockerfile="$repo_root/$dockerfile"
failures=0

fail() {
    echo "appliance-build-check: $*" >&2
    failures=$((failures + 1))
}

[[ -f "$dockerfile" ]] || { fail "Dockerfile not found: $dockerfile"; exit 1; }

# --- Static contract checks on the appliance stage -------------------------
grep -Eq '^FROM .* AS runtime-appliance$' "$dockerfile" \
    || fail "missing 'runtime-appliance' build stage"

grep -Eq -- '--bin[[:space:]]+geo_hub([[:space:]\\]|$)' "$dockerfile" \
    || fail "builder does not build the geo_hub binary"

grep -Eq 'COPY --from=builder .*/geo_hub[[:space:]]+/opt/agbot/bin/' "$dockerfile" \
    || fail "geo_hub binary is not copied into /opt/agbot/bin/"

grep -Eq 'COPY .*geo_hub/web[[:space:]]+/opt/agbot/web' "$dockerfile" \
    || fail "geo_hub/web assets are not copied into /opt/agbot/web"

for env_key in \
    'GEO_HUB__BIND_ADDRESS=' \
    'GEO_HUB__DATABASE_URL=' \
    'GEO_HUB__DATA_ROOT=' \
    'GEO_HUB__WORKSPACE_WEB_ROOT='; do
    grep -q "$env_key" "$dockerfile" \
        || fail "appliance stage does not pin $env_key"
done

grep -q 'USER agbot' "$dockerfile" \
    || fail "appliance stage does not switch to the non-root agbot user"

grep -Eq 'CMD \["geo_hub"\]' "$dockerfile" \
    || fail "appliance stage does not default to CMD [\"geo_hub\"]"

grep -q '/opt/agbot/build-manifest.json' "$dockerfile" \
    || fail "appliance stage does not write /opt/agbot/build-manifest.json"

# CWD-relative defaults must be overridden to absolute /opt/agbot paths.
grep -q 'GEO_HUB__DATABASE_URL="sqlite:///opt/agbot' "$dockerfile" \
    || fail "database_url override is not an absolute appliance path"
grep -q 'GEO_HUB__DATA_ROOT="/opt/agbot' "$dockerfile" \
    || fail "data_root override is not an absolute appliance path"
grep -q 'GEO_HUB__WORKSPACE_WEB_ROOT="/opt/agbot' "$dockerfile" \
    || fail "workspace_web_root override is not an absolute appliance path"

if (( failures > 0 )); then
    exit 1
fi
echo "appliance-build-check: static appliance contract validated"

# --- Optional real Docker build -------------------------------------------
if (( do_build )); then
    if ! command -v docker >/dev/null 2>&1; then
        echo "appliance-build-check: docker not available; skipping --build" >&2
        exit 0
    fi
    tag="geo-hub-appliance:verify"
    echo "appliance-build-check: building $tag (target runtime-appliance)"
    docker build \
        --target runtime-appliance \
        --build-arg AGRODRONE_COMMIT="$(git -C "$repo_root" rev-parse --short HEAD 2>/dev/null || echo unknown)" \
        -t "$tag" \
        "$repo_root"

    # Smoke-boot: the server should answer /health within a short window.
    cid="$(docker run -d -p 18080:8080 "$tag")"
    trap 'docker rm -f "$cid" >/dev/null 2>&1 || true' EXIT
    ok=0
    for _ in $(seq 1 30); do
        if curl -fsS http://127.0.0.1:18080/health >/dev/null 2>&1; then
            ok=1
            break
        fi
        sleep 1
    done
    if (( ok )); then
        echo "appliance-build-check: /health responded — appliance boots"
    else
        echo "appliance-build-check: appliance did not answer /health in time" >&2
        docker logs "$cid" >&2 || true
        exit 1
    fi
fi
