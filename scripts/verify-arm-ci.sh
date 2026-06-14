#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
workflow="$repo_root/.github/workflows/ci.yml"
bin_list="$repo_root/scripts/edge-runtime-bins.txt"
failures=0

fail() {
    echo "arm-ci-check: $*" >&2
    failures=$((failures + 1))
}

require_file() {
    if [[ ! -f "$1" ]]; then
        fail "missing file: $1"
    fi
}

require_file "$repo_root/Cross.toml"
require_file "$workflow"
require_file "$repo_root/justfile"
require_file "$bin_list"
require_file "$repo_root/scripts/smoke-arm-artifacts.sh"
require_file "$repo_root/scripts/package-arm-artifacts.sh"

for target in aarch64-unknown-linux-gnu armv7-unknown-linux-gnueabihf; do
    grep -q "\[target.$target\]" "$repo_root/Cross.toml" \
        || fail "Cross.toml is missing target section: $target"
    grep -q "$target" "$workflow" \
        || fail "CI workflow is missing target matrix entry: $target"
done

grep -q "docker/setup-qemu-action@v3" "$workflow" \
    || fail "CI workflow does not install QEMU for target smoke checks"
grep -q "cargo install cross --locked" "$workflow" \
    || fail "CI workflow does not install cross with a locked cargo install"
grep -q "cargo install just --locked" "$workflow" \
    || fail "CI workflow does not install just with a locked cargo install"
grep -q 'just \${{ matrix.just_recipe }}' "$workflow" \
    || fail "CI workflow does not run the matrix just recipe"
grep -q "scripts/smoke-arm-artifacts.sh" "$workflow" \
    || fail "CI workflow does not run ARM smoke boot checks"
grep -q "scripts/package-arm-artifacts.sh" "$workflow" \
    || fail "CI workflow does not package ARM artifacts"
grep -q "actions/upload-artifact@v4" "$workflow" \
    || fail "CI workflow does not publish ARM artifacts"

grep -q "^arm64:" "$repo_root/justfile" || fail "justfile is missing arm64 recipe"
grep -q "^arm:" "$repo_root/justfile" || fail "justfile is missing arm recipe"

for script in "$repo_root/scripts/smoke-arm-artifacts.sh" "$repo_root/scripts/package-arm-artifacts.sh"; do
    bash -n "$script"
done

bin_count=0
while IFS= read -r bin; do
    [[ -z "$bin" || "$bin" == \#* ]] && continue
    bin_count=$((bin_count + 1))
done < "$bin_list"

if [[ "$bin_count" -eq 0 ]]; then
    fail "edge runtime binary list is empty"
fi

if (( failures > 0 )); then
    exit 1
fi

echo "arm-ci-check: ARM cross CI contract validated"
