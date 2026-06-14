#!/usr/bin/env bash
set -euo pipefail

target="${1:?usage: package-arm-artifacts.sh <target-triple> [output-dir]}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
out_dir="${2:-$repo_root/dist/arm/$target}"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"

if [[ "$target_dir" != /* ]]; then
    target_dir="$repo_root/$target_dir"
fi

release_dir="$target_dir/$target/release"
bin_list="$repo_root/scripts/edge-runtime-bins.txt"
staging_dir="$out_dir/package"
archive="$out_dir/agbot-$target.tar.gz"

case "$target" in
    aarch64-unknown-linux-gnu|armv7-unknown-linux-gnueabihf) ;;
    *)
        echo "arm-package: unsupported target: $target" >&2
        exit 1
        ;;
esac

if [[ ! -f "$bin_list" ]]; then
    echo "arm-package: missing binary list: $bin_list" >&2
    exit 1
fi

rm -rf "$staging_dir"
mkdir -p "$staging_dir/bin" "$out_dir"

bins=()
while IFS= read -r bin; do
    [[ -z "$bin" || "$bin" == \#* ]] && continue
    bins+=("$bin")
done < "$bin_list"

if [[ "${#bins[@]}" -eq 0 ]]; then
    echo "arm-package: no binaries listed in $bin_list" >&2
    exit 1
fi

for bin in "${bins[@]}"; do
    src="$release_dir/$bin"
    if [[ ! -f "$src" ]]; then
        echo "arm-package: missing cross-compiled binary: $src" >&2
        exit 1
    fi
    cp "$src" "$staging_dir/bin/$bin"
    chmod 0755 "$staging_dir/bin/$bin"
done

commit="${GITHUB_SHA:-$(git -C "$repo_root" rev-parse HEAD)}"
json_bins=""
separator=""
for bin in "${bins[@]}"; do
    json_bins="$json_bins$separator\"$bin\""
    separator=", "
done

cat > "$staging_dir/build-manifest.json" <<EOF
{
  "commit": "$commit",
  "target": "$target",
  "profile": "release",
  "source_command": "cross build --target $target --release",
  "smoke_boot_command": "scripts/smoke-arm-artifacts.sh $target",
  "binaries": [$json_bins]
}
EOF

(
    cd "$staging_dir"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum bin/* > SHA256SUMS
    else
        shasum -a 256 bin/* > SHA256SUMS
    fi
)

tar -C "$staging_dir" -czf "$archive" .
echo "arm-package: wrote $archive"
