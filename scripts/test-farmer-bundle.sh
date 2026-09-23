#!/bin/sh
set -eu
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
test_tmp=$(mktemp -d)
trap 'rm -rf "$test_tmp"' EXIT HUP INT TERM
sh "$repo_root/scripts/package-farmer-bundle.sh" v0.0.0-test stable "$test_tmp"
cd "$test_tmp"
shasum -a 256 -c agbot-v0.0.0-test.sha256
mkdir 'extracted bundle'
tar -xzf agbot-v0.0.0-test.tar.gz -C 'extracted bundle'
bundle_root="$test_tmp/extracted bundle/agbot-v0.0.0-test"
[ -x "$bundle_root/Start AGBot.command" ]
[ -x "$bundle_root/Stop AGBot.command" ]
[ -x "$bundle_root/start.sh" ]
[ -f "$bundle_root/README.md" ]
[ -f "$bundle_root/VERSION" ]
grep -q '^name: agbot$' "$bundle_root/docker-compose.prod.yml"
AGBOT_TEST_ROOT="$bundle_root" sh "$repo_root/scripts/test-agbot-cli.sh"
printf 'farmer-bundle-test: archive, checksum, and extracted launchers passed\n'
