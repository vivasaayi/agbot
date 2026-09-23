#!/bin/sh
set -eu
bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec "$bundle_dir/scripts/agbot" down --channel "$(cat "$bundle_dir/CHANNEL")"
