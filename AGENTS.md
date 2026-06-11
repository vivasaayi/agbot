# AGBot Codex Instructions

Use this file as persistent project guidance for Codex runs in this repository. These instructions apply to the whole repo unless a nested `AGENTS.md` or `AGENTS.override.md` provides more specific guidance.

## Mission

AGBot (AgroDrone) is an autonomous agricultural drone platform: a Rust monorepo for real-time flight control, multi-sensor data acquisition, remote-sensing imagery analysis, LiDAR mapping, GIS serving, and ground-station visualization. Treat the product goal as a complete field-to-decision pipeline: fly missions safely, collect LiDAR and multispectral data, derive NDVI and other indices from evidence, build occupancy grids and 3D maps, serve geospatial layers and advisor recommendations, and drive both web and 3D visualization. Favor deterministic, evidence-backed processing over opaque heuristics.

The active Cargo workspace members are: `mission_planner`, `sensor_overlay_engine`, `multi_drone_control`, `data_collector`, `post_processor`, `shared`, `lidar_mapper`, `imagery_processor`, `geo_hub`, and `geo_viewer`. There is also a C++ `flight_sim_cpp` component, which is the canonical simulator for both interactive viewing and headless deterministic regression. The former Rust/Bevy `simulator` crate and Rust `drone_simulator` crate are retired; do not reintroduce a second simulator or mission-preview surface unless the user explicitly asks for that architectural change.

## Autonomous Operating Mode

- Work end-to-end when the user gives an implementation, review, roadmap, validation, or commit request.
- Do not stop at a plan when there is enough information to act.
- Ask the user only for destructive or irreversible actions, a scope change, or information that cannot be discovered locally.
- Before ending a response, check whether the last paragraph is only a plan, promise, question, or next-step list. If it is, do the work instead.
- Keep edits scoped to the requested outcome and the established project shape.
- Prefer existing repo patterns over new abstractions.
- Never revert unrelated user changes.

## Roadmap One-Shot Execution

When the task is to execute the AGBot roadmap, do not ask the user which item to start with.

- Start from `docs/product-roadmap/` including `README.md`, `product-doctrine.md`, `requirements-rigor.md`, `implementation-sequencing.md`, and the domain folders; also inspect `docs/reference/product-requirements.md` and the `prompts/` directory (`init.md`, `simulation.md`, `visualization.md`, `codex-one-shot-roadmap.md`) when present.
- Enumerate all milestone and backlog items before selecting work.
- Process the backlog in deterministic batches. Do not attempt to load all items into context at once.
- Prioritize P0, then P1, then P2. Within each priority, prefer foundational inventory and observable-foundation work before later milestones.
- Use a progress ledger or checkpoint so a later run can resume exactly.
- If subagents are available, use them for backlog triage, Rust crate work, GIS/visualization work, tests, and independent verification. If subagents are unavailable, run those passes sequentially.
- Commit each completed, verified batch when the task definition requires commits.
- Never claim the whole roadmap is complete unless every item has been processed and verified.

## Parallel Batch Execution

Speed matters, but parallelism must not corrupt the worktree.

- Prefer 2-4 parallel lanes when the runtime supports subagents or background agents.
- Use SQLite as the coordination source of truth. Every agent must atomically claim feature IDs before work begins.
- Parallelize only independent slices: different crates, different files, or analysis/test work that will not edit the same modules.
- Do not let two agents edit the same Rust crate, module, route, controller, migration, proto, or generated file at the same time.
- Keep one coordinator responsible for batch selection, conflict checks, final integration, validation, staging, commits, and checkpoint updates.
- Agents should return compact evidence: claimed feature IDs, files changed, tests run, failures, commit readiness, and exact next action.
- Run expensive validations in parallel only when they do not compete for the same build lock or mutate shared output. Otherwise serialize validation. The Rust `target/` build lock is shared across the workspace.
- Commits must be serialized. One committed, verified batch at a time.
- If parallel lanes conflict, pause the lower-priority lane, write an event to SQLite, and let the coordinator decide whether to rebase, merge manually, or requeue.
- If subagents are unavailable, simulate parallel roles sequentially but keep the same SQLite claim/checkpoint protocol.

## SQLite Checkpointing Protocol

For roadmap one-shot execution, use SQLite checkpointing. This is preferred over long prose checkpoints because a milestone backlog may have many items and may be processed by multiple agents.

Checkpoint location for Codex runs:

- Database: `.codex/checkpoints/roadmap-run/checkpoint.sqlite`
- Human/model resume file: `.codex/checkpoints/roadmap-run/RESUME.md`

If resuming a legacy Claude-run roadmap task, first inspect `.claude/checkpoints/roadmap-run/RESUME.md` and `.claude/checkpoints/roadmap-run/checkpoint.sqlite`, then continue from the freshest valid checkpoint. Prefer `.codex` for new Codex runs; mirror a legacy `.claude` checkpoint into `.codex` only when doing so is safe and useful for future Codex resumes. If both locations exist, compare `runs.last_commit`, `current_batch_id`, and `next_action`, record the chosen source in `events`, and avoid overwriting a newer checkpoint with an older one.

Initialize SQLite with:

```sql
PRAGMA journal_mode=WAL;
PRAGMA busy_timeout=5000;

CREATE TABLE IF NOT EXISTS runs (
  id TEXT PRIMARY KEY,
  roadmap_hash TEXT NOT NULL,
  started_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_commit TEXT,
  current_batch_id TEXT,
  next_action TEXT
);

CREATE TABLE IF NOT EXISTS feature_progress (
  feature_id TEXT PRIMARY KEY,
  module TEXT NOT NULL,
  service_or_domain TEXT,
  priority TEXT,
  release_phase TEXT,
  status TEXT NOT NULL,
  batch_id TEXT,
  agent TEXT,
  commit_sha TEXT,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS batches (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL,
  selection_rule TEXT,
  agent TEXT,
  started_at TEXT,
  completed_at TEXT,
  commit_sha TEXT
);

CREATE TABLE IF NOT EXISTS events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ts TEXT NOT NULL,
  event_type TEXT NOT NULL,
  batch_id TEXT,
  feature_id TEXT,
  agent TEXT,
  command TEXT,
  status TEXT,
  details TEXT
);
```

Use these feature statuses:

- `pending`
- `claimed`
- `in_progress`
- `implemented`
- `tests_passed`
- `committed`
- `blocked`
- `skipped`

Rules:

- Hash roadmap inputs before selecting work: the `docs/product-roadmap/` domain folders, especially each `release-plan.md`, the `prompts/` files, and any backlog source you derive items from.
- Store one row per feature ID in `feature_progress`; do not store the full item body unless needed.
- Claim work atomically so agents do not duplicate work: update from `pending` to `claimed` only when the row is still pending.
- Append every important action to `events`: batch selection, claim, file edit, test command, failure, verifier result, commit, pause, resume, and blocker.
- Treat a git commit as the strongest checkpoint. After each verified batch commit, update `runs.last_commit`, `batches.commit_sha`, and all completed `feature_progress.commit_sha` values.
- Keep `RESUME.md` tiny: current run ID, roadmap hash, last commit, current batch, completed batch count, blocker if any, and exact next action.
- On resume, read `RESUME.md`, verify `checkpoint.sqlite`, verify the roadmap hash, verify `last_commit`, check `git status --short`, then continue from `runs.next_action`.

## Evidence Rules

- Ground claims in the current repo, command output, generated files, tests, or logs from the current run.
- Do not say work is complete until it is implemented and verified.
- If validation fails, report the exact command and the failing behavior.
- If the working tree is dirty, stage only files that belong to the requested change.

## Engineering Standards

- Use test-driven development for non-trivial behavior: write or update the smallest meaningful failing test first, implement the behavior, then refactor with tests passing.
- Use domain-driven design: keep domain rules in small, named Rust modules or pure functions; keep controllers/routes/handlers thin; keep persistence, sensor/hardware clients, and UI concerns outside core domain logic.
- Keep files small and cohesive. Split large files when a new behavior creates a clear domain boundary, but avoid broad refactors unrelated to the task.
- Prefer explicit types, reason-coded errors (`thiserror`/`anyhow`), deterministic evaluators, and evidence objects over stringly typed control flow.
- Add unit tests for domain logic and API/handler tests for route/controller behavior when backend behavior changes.
- For GIS and visualization changes, add or update crate tests in `geo_hub`, `geo_viewer`, and `shared`, and keep the GIS acceptance workflow passing.
- Do not mock AGBot's actual domain logic to make tests pass. Test real domain/service code with fixtures, test data, fake sensor inputs, or local adapters. Mock only true external boundaries such as flight hardware, MAVLink links, serial devices, cameras/LiDAR, network services, or clocks, and keep those mocks behind clear interfaces. Use `RUNTIME_MODE=SIMULATION` for hardware-free runs.
- Do not hide defects with snapshots, brittle assertions, or skipped tests. Fix the implementation or narrow the test to the actual contract.

## Default Validation

Use the smallest validation set that matches the change. Run cargo commands from the workspace root unless a crate-local run is faster.

- Backend Rust: prefer incremental validation. Do not run `cargo clean`, delete `target/`, or force a full clean rebuild unless explicitly requested.
- Domain logic: run targeted tests first with `cargo test -p <crate> <test_name>`.
- Single crate check: run `cargo check -p <crate>` when the change is local to one crate.
- Workspace check: run `cargo check` after route, handler, dependency, proto, or cross-crate changes. This may take time, so reuse the existing incremental build cache.
- GIS regression: run `just gis-test` for the fast suite and `just gis-acceptance` for the advisor workflow: boundary import, layer serving, annotations, recommendations, reports, and exports. Narrow failures with `cargo test -p shared --lib`, `cargo test -p geo_hub --tests --lib`, or `cargo test -p geo_viewer`.
- Full workspace tests: `cargo test` or `just test`.
- Formatting and lint: `cargo fmt` and `cargo clippy -- -D warnings`, or `just fmt` and `just clippy`.
- C++ FlightSim: `just flight-sim-build` and `just flight-sim-test` when touching `flight_sim_cpp`.
- ARM cross-compile only when the change affects target builds: `just arm` or `cross build --target aarch64-unknown-linux-gnu --release`.

Warnings may already exist. Treat command exit codes and new failures as the signal.

## Git Discipline

- Commit only when the user asks for a commit or the one-shot task explicitly includes committing in its definition of done.
- Before committing, run `git status --short` and review the intended diff.
- Use clear, specific commit messages.
- Leave the working tree clean after a commit when possible.

## Sandbox-Aware Git Handling

- Read-only git commands such as `git status`, `git diff`, `git log`, `git show`, and `git branch --show-current` should run normally in the sandbox.
- Mutating git commands such as `git add`, `git commit`, `git tag`, `git merge`, `git rebase`, `git cherry-pick`, `git stash`, and `git reset` write to `.git` and may require escalated permissions in restricted workspaces.
- If a required mutating git command fails with a sandbox-style error such as `Permission denied`, `Operation not permitted`, or `Unable to create .git/index.lock`, rerun only the necessary git command with escalated permissions and a concise justification.
- Do not work around git sandbox failures by copying the repository, manually editing `.git`, using `sudo`, or running destructive cleanup.
- Stage explicitly with `git add <path>...`, review `git status --short`, then commit with a clear message.

## Context Budget and Resume Discipline

For long roadmap execution runs, context pressure is expected. Do not keep dragging stale context after a completed batch.

- After every committed batch, update `checkpoint.sqlite` and `RESUME.md`, then prefer ending the current Codex session so the next run can start from a compact checkpoint.
- If the visible token count is high, roughly above 120k tokens, or Codex enters a long context-compaction phase, stop doing new implementation work.
- Before stopping for context pressure, write a complete checkpoint: current batch, feature IDs, changed files, commands run, verification state, last commit, blocker if any, and exact next action.
- Do not use permissions changes as a fix for token/context pressure. Permissions affect tool access, not context size.
- After a context reset, resume by reading this `AGENTS.md`, `.codex/checkpoints/roadmap-run/RESUME.md`, and the SQLite checkpoint. Verify roadmap hash, last commit, and `git status --short`, then continue from `runs.next_action`.
- Do not re-enumerate the full roadmap after a context reset unless the roadmap hash changed or the checkpoint is invalid.
- Keep progress updates short. Put durable state in SQLite and `RESUME.md`, not in chat.

## Long-Running and Limit Behavior

Codex cannot restart itself after an API token, rate, or context limit. The external harness must catch the limit response and requeue the task after the reset window.

If a token/rate limit or context interruption happens, write a checkpoint before stopping:

- Objective.
- Completed work.
- Files changed.
- Commands run.
- Verification status.
- Current blocker.
- Exact next action.
- Resume instructions.

On resume, read the checkpoint and continue from the exact next action instead of restarting from scratch.

## Final Response

Lead with the outcome. Include changed files, validation commands, commits, and remaining blockers. Keep the answer concise and factual.
