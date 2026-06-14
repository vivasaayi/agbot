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
- Process large or cross-domain roadmap runs through the Roadmap MapReduce Execution Model below. Do not attempt to load all items into context at once.
- Prioritize P0, then P1, then P2. Within each priority, prefer foundational inventory and observable-foundation work before later milestones.
- Use a progress ledger or checkpoint so a later run can resume exactly.
- If subagents are available, use them for backlog triage, Rust crate work, GIS/visualization work, tests, and independent verification. If subagents are unavailable, run those passes sequentially.
- Commit each completed, verified batch when the task definition requires commits.
- Never claim the whole roadmap is complete unless every item has been processed and verified.

## Roadmap MapReduce Execution Model

Use this model for large roadmap runs. The goal is to reason globally, execute independently, integrate centrally, and verify continuously.

### Map Phase: Roadmap Decomposition

Before implementation, the coordinator reads the roadmap, checkpoint, and current repository state, then maps the roadmap into candidate macro-batches. This phase is non-mutating unless a checkpoint must be initialized.

Identify:

- Product capabilities and feature IDs.
- Dependency relationships and critical-path blockers.
- Shared contracts, domain foundations, and acceptance gates.
- Affected crates, modules, routes, schemas, migrations, generated outputs, and tests.
- Candidate macro-batches and safe parallel lanes.
- Conflict zones that must be serialized.

The Map phase must not produce implementation code unless the roadmap graph and candidate macro-batches are already known or valid in the checkpoint.

### Shuffle Phase: Claiming and Coordination

Group dependency-ready work into macro-batches and assign lanes through SQLite.

Rules:

- Every lane must atomically claim feature IDs before implementation.
- Do not assign two lanes to the same crate, module, route, controller, migration, generated file, or shared contract at the same time.
- Prefer independent lanes: shared contracts, GIS/backend, imagery/analytics, viewer/UI, simulator/flight, tests, and verification.
- If work overlaps, serialize it through the coordinator.
- If a conflict appears, pause the lower-priority lane, record the event in SQLite, and requeue, merge, or split the work.

### Worker Phase: Independent Implementation

Each worker lane implements only its claimed macro-batch.

Workers must return compact evidence:

- Claimed feature IDs and batch ID.
- Files changed.
- Contracts or APIs changed.
- Tests added or updated.
- Commands run and results.
- Failures encountered.
- Product outcome achieved.
- Commit readiness.
- Exact next action.

Workers must avoid broad speculative scaffolding. Code is valuable only when it supports a verified workflow, shared foundation, acceptance gate, simulator behavior, test fixture, or critical-path dependency.

### Reduce Phase: Integration

The coordinator integrates worker outputs.

The coordinator must:

- Review worker evidence.
- Resolve integration conflicts.
- Remove duplicate or incompatible models.
- Ensure shared contracts are authoritative.
- Prune speculative code not required by the accepted workflow.
- Run required validation commands.
- Stage only files that belong to the verified batch.
- Commit one verified batch at a time.
- Update SQLite and `RESUME.md`.

Commits remain serialized even when implementation work is parallel.

### Verify Phase: Acceptance and Metrics

A MapReduce cycle is complete only when the integrated batch is verified.

Record:

- Accepted workflow or critical-path foundation completed.
- Validation commands and results.
- Files changed.
- Net lines added/deleted when available.
- Downstream items unblocked.
- Product progress score.
- Last commit SHA.
- Exact next action.

A cycle is not successful because it generated code. A cycle is successful only when it increases accepted product capability.

Operating principle: Map the roadmap globally. Shuffle work safely. Implement in independent lanes. Reduce through one coordinator. Verify before commit. Checkpoint after every accepted product slice. Maximize accepted product capability per token, not code volume per hour.

## Roadmap Batch Loop Mode

Use this mode only when the user or one-shot prompt explicitly asks the agent to continue through multiple roadmap batches in a loop. Do not impose a discretionary batch-count limit.

- Continue selecting and executing deterministic batches until every roadmap item is processed and verified, or until a hard stop condition is reached.
- After each verified commit, update `checkpoint.sqlite` and `RESUME.md`, then re-read the active checkpoint before selecting the next batch.
- Before starting each next batch, verify `git status --short`, `runs.last_commit`, `current_batch_id`, `next_action`, and the roadmap hash. If the roadmap hash changed, re-evaluate selection before claiming work.
- Select, claim, implement, validate, commit, and checkpoint the next batch using the Roadmap MapReduce Execution Model and the same P0 -> P1 -> P2 priority rules.
- Do not stop merely because a batch completed, the next batch is larger, or the worktree is clean after a checkpoint.
- Stop the loop only when there are no pending items, validation fails and cannot be fixed within the current batch, unrelated worktree changes create a conflict, a real blocker prevents progress, the user explicitly stops the run, or context/token/rate-limit/timeout pressure makes it impossible to continue safely.
- Treat context, token, rate-limit, and timeout pressure as hard stop conditions only when there is concrete evidence that continuing another batch would likely lose work or prevent a checkpoint. Do not stop speculatively.
- Before stopping, write a durable checkpoint with the current batch, feature IDs, changed files, commands run, verification state, last commit, blocker if any, and exact next action.
- Codex cannot restart itself after API token, context, or rate limits. If an external runtime reset is required, SQLite and `RESUME.md` are the handoff contract for the harness to relaunch Codex and continue from the exact next action.

## Parallel Batch Execution

Speed matters, but parallelism must not corrupt the worktree.

- Prefer 2-4 parallel lanes when the runtime supports subagents or background agents.
- Use SQLite as the coordination source of truth. Every agent must atomically claim feature IDs before work begins.
- Parallelize only independent MapReduce lanes: different crates, different files, or analysis/test work that will not edit the same modules.
- Do not let two agents edit the same Rust crate, module, route, controller, migration, proto, or generated file at the same time.
- Keep one coordinator responsible for Map, Shuffle, Reduce, Verify, conflict checks, staging, commits, and checkpoint updates.
- Agents should return compact worker evidence: claimed feature IDs, batch ID, files changed, contracts or APIs changed, tests run, failures, product outcome, commit readiness, and exact next action.
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
- Keep `RESUME.md` tiny: current run ID, roadmap hash, last implementation commit, latest checkpoint commit when different, current batch, completed batch count, blocker if any, and exact next action.
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

For long roadmap execution runs, context pressure is expected. Checkpoint after each completed batch so an external runtime reset can resume exactly; do not voluntarily stop solely to get a clean context.

- After every committed batch, update `checkpoint.sqlite` and `RESUME.md`, then select and claim the next deterministic batch unless a hard stop condition applies.
- Do not stop solely because the context is large, a batch boundary is clean, or a context compaction may occur. Stop starting new implementation work only when the remaining context is insufficient to complete, validate, and checkpoint the next batch safely, or the runtime is about to hit a hard token/rate/timeout limit.
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
