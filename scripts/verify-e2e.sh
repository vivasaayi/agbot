#!/usr/bin/env bash
# Hardware-free acceptance. All runtime state stays in a fresh run directory.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
cargo_bin=${CARGO_BIN:-cargo}
cmake_bin=${CMAKE_BIN:-cmake}
jobs=${AGBOT_BUILD_JOBS:-2}
command -v "$cargo_bin" >/dev/null
command -v "$cmake_bin" >/dev/null
ctest_bin=${CTEST_BIN:-$(dirname "$(command -v "$cmake_bin")")/ctest}
command -v "$ctest_bin" >/dev/null
command -v node >/dev/null
command -v python3 >/dev/null
node -e 'require(process.env.PLAYWRIGHT_MODULE || "playwright")'
run_dir=$(mktemp -d "${TMPDIR:-/tmp}/agbot-e2e.XXXXXX")
echo "Acceptance artifacts: $run_dir"
"$cmake_bin" -S flight_sim_cpp -B "$run_dir/sim-build"
"$cmake_bin" --build "$run_dir/sim-build" --parallel "$jobs"
"$ctest_bin" --test-dir "$run_dir/sim-build" --output-on-failure --parallel "$jobs"
"$run_dir/sim-build/agbot_flight_sim_headless" --seed 42 \
  --mission flight_sim_cpp/samples/sample_field_loop.json --output "$run_dir/flight.jsonl"
"$cargo_bin" run --locked -j "$jobs" -p data_collector --bin sim_lidar_export -- \
  --input "$run_dir/flight.lidar.jsonl" --output-dir "$run_dir/scans"
"$cargo_bin" run --locked -j "$jobs" -p lidar_mapper -- \
  --input-dir "$run_dir/scans" --output-dir "$run_dir/local-map"
origin_lat=$(python3 -c 'import json;print(json.load(open("flight_sim_cpp/samples/sample_field_loop.json"))["home"]["latitude"])')
origin_lon=$(python3 -c 'import json;print(json.load(open("flight_sim_cpp/samples/sample_field_loop.json"))["home"]["longitude"])')
"$cargo_bin" run --locked -j "$jobs" -p lidar_mapper --bin sim_world_map -- \
  --input "$run_dir/flight.lidar.jsonl" --output-dir "$run_dir/world-map" \
  --origin-latitude="$origin_lat" --origin-longitude="$origin_lon"
for directory in local-map world-map; do
  GEO_HUB__DATABASE_URL="sqlite://$run_dir/hub.db?mode=rwc" \
  GEO_HUB__DATA_ROOT="$run_dir/hub-data" \
  GEO_HUB__BOOTSTRAP__ENABLED=false \
  "$cargo_bin" run --locked -j "$jobs" -p geo_hub -- catalog register "$run_dir/$directory"
done
hub_target=$("$cargo_bin" metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
AGBOT_TEST_DISPOSABLE=1 AGBOT_TEST_STATE="$run_dir" AGBOT_HUB_BINARY="$hub_target/debug/geo_hub" \
  node scripts/test-workspace-products.cjs
AGBOT_BROWSER_ACCEPTANCE=1 "$cargo_bin" test --locked -j "$jobs" -p geo_hub \
  --test lidar_catalog_pipeline --test landsat_advisor_pipeline \
  --test portal_onboarding --test backup_restore --test workspace_static -- --nocapture
echo "Acceptance passed. Artifacts retained at $run_dir"
