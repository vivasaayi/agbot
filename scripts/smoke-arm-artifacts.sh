#!/usr/bin/env bash
set -euo pipefail

target="${1:?usage: smoke-arm-artifacts.sh <target-triple>}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
bin_list="$repo_root/scripts/edge-runtime-bins.txt"
timeout_seconds="${ARM_SMOKE_TIMEOUT_SECONDS:-30s}"

case "$target" in
    aarch64-unknown-linux-gnu|armv7-unknown-linux-gnueabihf) ;;
    *)
        echo "arm-smoke: unsupported target: $target" >&2
        exit 1
        ;;
esac

if ! command -v cross >/dev/null 2>&1; then
    echo "arm-smoke: cross is required for target smoke boot checks" >&2
    exit 1
fi

if [[ ! -f "$bin_list" ]]; then
    echo "arm-smoke: missing binary list: $bin_list" >&2
    exit 1
fi

bins=()
while IFS= read -r bin; do
    [[ -z "$bin" || "$bin" == \#* ]] && continue
    bins+=("$bin")
done < "$bin_list"

if [[ "${#bins[@]}" -eq 0 ]]; then
    echo "arm-smoke: no binaries listed in $bin_list" >&2
    exit 1
fi

for bin in "${bins[@]}"; do
    echo "arm-smoke: target=$target bin=$bin command=--help"
    cmd=(env RUNTIME_MODE=SIMULATION cross run --target "$target" --release --bin "$bin" -- --help)
    if command -v timeout >/dev/null 2>&1; then
        timeout "$timeout_seconds" "${cmd[@]}"
    else
        "${cmd[@]}"
    fi
done

echo "arm-smoke: all $target binaries reached their CLI help path"
