#!/bin/sh
# Package the browser application's launchers; no Rust toolchain or Git needed.
set -eu
[ "$#" -eq 3 ] || { echo 'usage: package-farmer-bundle.sh <version> <stable|edge> <output-directory>' >&2; exit 1; }
bundle_version=$1
bundle_channel=$2
case "$bundle_version" in ''|*[!a-zA-Z0-9._-]*) echo 'invalid version' >&2; exit 1 ;; esac
case "$bundle_channel" in stable|edge) ;; *) echo 'invalid release channel' >&2; exit 1 ;; esac
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
mkdir -p "$3"
output_dir=$(CDPATH= cd -- "$3" && pwd -P)
bundle_tmp=$(mktemp -d)
trap 'rm -rf "$bundle_tmp"' EXIT HUP INT TERM
bundle_name="agbot-$bundle_version"
bundle_dir="$bundle_tmp/$bundle_name"
mkdir -p "$bundle_dir/scripts"
cp "$repo_root/scripts/agbot" "$bundle_dir/scripts/agbot"
# Keep data-volume names stable when a newer version extracts to a new folder.
printf 'name: agbot\n' > "$bundle_dir/docker-compose.prod.yml"
cat "$repo_root/docker-compose.prod.yml" >> "$bundle_dir/docker-compose.prod.yml"
cp "$repo_root/scripts/farmer-bundle/README.md" "$bundle_dir/"
cp "$repo_root/scripts/farmer-bundle/start.sh" "$bundle_dir/start.sh"
cp "$repo_root/scripts/farmer-bundle/stop.sh" "$bundle_dir/stop.sh"
cp "$bundle_dir/start.sh" "$bundle_dir/Start AGBot.command"
cp "$bundle_dir/stop.sh" "$bundle_dir/Stop AGBot.command"
printf '%s\n' "$bundle_version" > "$bundle_dir/VERSION"
printf '%s\n' "$bundle_channel" > "$bundle_dir/CHANNEL"
chmod +x "$bundle_dir/scripts/agbot" "$bundle_dir/"*.sh "$bundle_dir/"*.command
COPYFILE_DISABLE=1 tar -czf "$output_dir/$bundle_name.tar.gz" -C "$bundle_tmp" "$bundle_name"
cd "$output_dir"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$bundle_name.tar.gz" > "$bundle_name.sha256"
else
    shasum -a 256 "$bundle_name.tar.gz" > "$bundle_name.sha256"
fi
printf '%s\n' "$output_dir/$bundle_name.tar.gz" "$output_dir/$bundle_name.sha256"
