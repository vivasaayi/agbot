#!/usr/bin/env bash
set -euo pipefail

if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "error: sqlite3 is required in PATH" >&2
  exit 1
fi

DB_PATH=".codex/checkpoints/roadmap-run/checkpoint.sqlite"
RUN_ID="run-02-sim"
BATCH_SIZE=4
AGENT="codex"

usage() {
  cat <<'USAGE'
Usage:
  scripts/roadmap_batch_claim.sh [OPTIONS]

Options:
  --db PATH            Path to checkpoint sqlite (default: .codex/checkpoints/roadmap-run/checkpoint.sqlite)
  --run-id ID          Active run id (default: run-02-sim)
  --batch-size N       Number of items to claim (default: 4)
  --agent NAME         Agent marker for claimed/features/events (default: codex)
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --db)
      DB_PATH="$2"
      shift 2
      ;;
    --run-id)
      RUN_ID="$2"
      shift 2
      ;;
    --batch-size)
      BATCH_SIZE="$2"
      shift 2
      ;;
    --agent)
      AGENT="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if ! [[ "$BATCH_SIZE" =~ ^[1-9][0-9]*$ ]]; then
  echo "error: --batch-size must be a positive integer" >&2
  exit 1
fi

if [[ ! -f "$DB_PATH" ]]; then
  echo "error: checkpoint db not found at $DB_PATH" >&2
  exit 1
fi

run_exists=$(sqlite3 "$DB_PATH" "SELECT 1 FROM runs WHERE id = '${RUN_ID}' LIMIT 1;")
if [[ -z "$run_exists" ]]; then
  echo "error: run id '$RUN_ID' not present in $DB_PATH" >&2
  exit 1
fi

selected_ids=()
while IFS=$'\t' read -r feature_id priority module release_phase; do
  selected_ids+=("${feature_id}")
done < <(
  sqlite3 "$DB_PATH" -separator $'\t' -noheader <<SQL
SELECT feature_id, priority, module, release_phase
FROM feature_progress
WHERE status = 'pending'
ORDER BY
  CASE priority
    WHEN 'P0' THEN 0
    WHEN 'P1' THEN 1
    WHEN 'P2' THEN 2
    ELSE 3
  END,
  CAST(substr(feature_id, 1, 2) AS INTEGER),
  CAST(substr(feature_id, 4) AS INTEGER)
LIMIT ${BATCH_SIZE};
SQL
)

if [[ "${#selected_ids[@]}" -eq 0 ]]; then
  echo "No pending feature rows matched current selection."
  exit 0
fi

selected_feature_ids=("${selected_ids[@]}")

if [[ "${#selected_feature_ids[@]}" -eq 0 ]]; then
  echo "No eligible pending feature IDs returned by selection."
  exit 1
fi

now_utc="$(date -u +"%Y-%m-%d %H:%M:%S")"
batch_id="batch-$(date -u +%Y%m%d%H%M%S)"
next_action="Implement STORIES: $(printf '%s, ' "${selected_feature_ids[@]}" | sed 's/, $//')"

id_list_sql=$(printf "'%s'," "${selected_feature_ids[@]}" | sed "s/,$//")

read -r count_before < <(sqlite3 "$DB_PATH" -noheader <<SQL
SELECT COUNT(*)
FROM feature_progress
WHERE feature_id IN (${id_list_sql})
  AND status = 'pending';
SQL
)

if [[ "$count_before" -eq 0 ]]; then
  echo "All selected feature rows are no longer pending. Re-run for a fresh batch."
  exit 0
fi

sqlite3 "$DB_PATH" <<SQL
BEGIN IMMEDIATE;
INSERT INTO batches(
  id,
  status,
  selection_rule,
  agent,
  started_at,
  completed_at,
  commit_sha
) VALUES (
  '${batch_id}',
  'in_progress',
  'priority-order-batch',
  '${AGENT}',
  '${now_utc}',
  NULL,
  NULL
);

UPDATE runs
SET current_batch_id = '${batch_id}',
    next_action = '${next_action}',
    updated_at = '${now_utc}'
WHERE id = '${RUN_ID}';

UPDATE feature_progress
SET status = 'claimed',
    batch_id = '${batch_id}',
    agent = '${AGENT}',
    updated_at = '${now_utc}'
WHERE feature_id IN (${id_list_sql})
  AND status = 'pending';

INSERT INTO events (
  ts,
  event_type,
  batch_id,
  feature_id,
  agent,
  command,
  status,
  details
)
SELECT
  '${now_utc}',
  'batch_claim',
  '${batch_id}',
  feature_id,
  '${AGENT}',
  'scripts/roadmap_batch_claim.sh --run-id ${RUN_ID} --batch-size ${BATCH_SIZE} --agent ${AGENT}',
  'ok',
  'Selected by priority order and claimed as a batch'
FROM feature_progress
WHERE feature_id IN (${id_list_sql})
  AND status = 'claimed';

COMMIT;
SQL

claim_count=$(sqlite3 "$DB_PATH" -noheader <<SQL
SELECT COUNT(*)
FROM feature_progress
WHERE batch_id = '${batch_id}'
  AND status = 'claimed';
SQL
)

if [[ "$claim_count" -ne "$count_before" ]]; then
  echo "Warning: claimed ${claim_count} rows but expected ${count_before}; re-check selection concurrency."
fi

printf 'Batch %s claimed %s feature(s): %s\n' "$batch_id" "${#selected_feature_ids[@]}" "$next_action"
