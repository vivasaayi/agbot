#!/bin/sh
set -eu
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
test_tmp=$(mktemp -d)
trap 'rm -rf "$test_tmp"' EXIT HUP INT TERM
source_repo="$test_tmp/source"
mkdir -p "$source_repo/scripts" "$test_tmp/bin"
git init -q -b main "$source_repo"
git -C "$source_repo" config user.name 'Installer test'
git -C "$source_repo" config user.email 'installer-test@example.invalid'
cp "$repo_root/scripts/agbot" "$source_repo/scripts/agbot"
printf 'first\n' > "$source_repo/note.txt"
git -C "$source_repo" add scripts/agbot note.txt
git -C "$source_repo" commit -qm initial
export AGBOT_HOME="$test_tmp/install"
export AGBOT_REPO="file://$source_repo"
export AGBOT_REF=main
export BIN_DIR="$test_tmp/bin"
sh "$repo_root/scripts/install.sh" > "$test_tmp/install.log" 2>&1
installed_repo="$AGBOT_HOME/repo"
printf 'second\n' > "$source_repo/note.txt"
git -C "$source_repo" commit -qam update
sh "$repo_root/scripts/install.sh" >> "$test_tmp/install.log" 2>&1
[ "$(git -C "$installed_repo" rev-parse HEAD)" = "$(git -C "$source_repo" rev-parse HEAD)" ]
printf 'my changes\n' > "$installed_repo/note.txt"
if sh "$repo_root/scripts/install.sh" > "$test_tmp/dirty.log" 2>&1; then
    printf 'FAIL: installer accepted a dirty checkout\n' >&2; exit 1
fi
[ "$(cat "$installed_repo/note.txt")" = 'my changes' ]
grep -q 'local changes' "$test_tmp/dirty.log"
# Commit the local edit: a clean divergent history must also be preserved.
git -C "$installed_repo" -c user.name=Test -c user.email=test@example.invalid commit -qam local
local_commit=$(git -C "$installed_repo" rev-parse HEAD)
printf 'upstream changes\n' > "$source_repo/note.txt"
git -C "$source_repo" commit -qam upstream
if sh "$repo_root/scripts/install.sh" > "$test_tmp/diverged.log" 2>&1; then
    printf 'FAIL: installer accepted diverged history\n' >&2; exit 1
fi
[ "$(git -C "$installed_repo" rev-parse HEAD)" = "$local_commit" ]
[ "$(cat "$installed_repo/note.txt")" = 'my changes' ]
printf 'installer-test: clean update and local work preservation passed\n'
