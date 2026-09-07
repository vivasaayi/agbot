#!/usr/bin/env bash
# Static contract checks for the delivery path. These guard the release gates
# without requiring Docker, a registry login, or a GitHub Actions runner.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
ci_workflow="$repo_root/.github/workflows/ci.yml"
release_workflow="$repo_root/.github/workflows/release.yml"
compose_file="$repo_root/docker-compose.prod.yml"
cli="$repo_root/scripts/agbot"
failures=0

fail() {
    echo "delivery-pipeline-check: $*" >&2
    failures=$((failures + 1))
}

require_pattern() {
    local file=$1
    local pattern=$2
    local description=$3
    grep -Eq "$pattern" "$file" || fail "$description"
}

for file in "$ci_workflow" "$release_workflow" "$compose_file" "$cli"; do
    [[ -f "$file" ]] || { fail "required file missing: $file"; }
done

if (( failures > 0 )); then
    exit 1
fi

require_pattern "$ci_workflow" 'workflow_call:' \
    "CI is not reusable by the release workflow"
require_pattern "$ci_workflow" 'name: Flight simulator regression' \
    "CI does not name a C++ flight simulator gate"
require_pattern "$ci_workflow" 'ctest --test-dir flight_sim_cpp/build --output-on-failure' \
    "CI does not execute flight simulator tests"
require_pattern "$ci_workflow" 'name: GIS acceptance' \
    "CI does not include the GIS regression gate"
require_pattern "$ci_workflow" 'cargo test --locked -p geo_hub acceptance_' \
    "CI does not execute the GIS acceptance workflow"
require_pattern "$ci_workflow" 'name: Dependency security' \
    "CI does not audit Rust dependency advisories"
require_pattern "$ci_workflow" 'target: runtime-appliance' \
    "CI does not build the shipped appliance target"
require_pattern "$ci_workflow" 'Smoke boot appliance health endpoint' \
    "CI does not smoke-test the appliance health endpoint"
require_pattern "$ci_workflow" 'Scan appliance vulnerabilities' \
    "CI does not scan the shipped appliance image"

require_pattern "$release_workflow" 'uses: \./\.github/workflows/ci\.yml' \
    "release does not call the reusable CI gates"
require_pattern "$release_workflow" 'needs: verify' \
    "release publication is not gated on CI"
require_pattern "$release_workflow" 'id-token: write' \
    "release lacks OIDC permission for keyless signing"
require_pattern "$release_workflow" 'provenance: mode=max' \
    "release does not publish build provenance"
require_pattern "$release_workflow" 'sbom: true' \
    "release does not publish an SBOM"
require_pattern "$release_workflow" 'cosign sign --yes' \
    "release does not sign the published image digest"
require_pattern "$release_workflow" 'Stable release version must be a semver tag' \
    "release does not validate stable version input"
require_pattern "$release_workflow" 'Manual stable releases must run from main' \
    "release permits manual stable releases from arbitrary refs"
require_pattern "$release_workflow" 'Stable release tag must point to a commit reachable from main' \
    "release permits stable tags outside main history"

require_pattern "$compose_file" 'AGBOT_REQUIRE_SESSION:-true' \
    "production compose does not require sessions by default"
require_pattern "$compose_file" 'AGBOT_RATE_LIMIT_PER_MIN:-120' \
    "production compose does not enable a safe default rate limit"
require_pattern "$cli" 'pin_pulled_image\(\)' \
    "operator CLI does not resolve channel tags to a digest"
require_pattern "$cli" 'rolling back to' \
    "operator CLI does not roll back unhealthy upgrades"
require_pattern "$cli" 'wait_for_health\(\)' \
    "operator CLI does not wait for appliance health"

if (( failures > 0 )); then
    exit 1
fi
echo "delivery-pipeline-check: release gates, provenance, and rollback contract validated"
