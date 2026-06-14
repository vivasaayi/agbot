# Codex One-Shot Roadmap Execution Prompt

Use this from the AGBot repo root. If Codex supports Goal mode, paste it after `/goal`; otherwise paste it as a normal prompt.

```text
Read and follow AGENTS.md first.

Objective: pick up the next executable AGBot roadmap task batch and carry it through implementation, validation, checkpointing, and commit.

Work mode:
- Do not stop at planning if there is enough local context to act.
- Resume from an existing checkpoint if present:
  - Prefer `.codex/checkpoints/roadmap-run/RESUME.md` and `checkpoint.sqlite`.
  - If `.codex` is missing but `.claude/checkpoints/roadmap-run` exists, inspect it and continue from that checkpoint. Mirror it into `.codex` only when doing so is safe and useful for future Codex resumes.
  - If both checkpoint locations exist, compare `runs.last_commit`, `current_batch_id`, and `next_action`; continue from the freshest valid checkpoint and record the choice in `events`.
- If no valid checkpoint exists, initialize the SQLite checkpoint protocol from AGENTS.md.
- If the working tree is dirty, identify which changes are unrelated user/agent work and do not revert them.

Task selection:
- Start from `docs/product-roadmap/` including `README.md`, `product-doctrine.md`, `requirements-rigor.md`, `implementation-sequencing.md`, and every domain folder.
- Also inspect `docs/reference/product-requirements.md` and the `prompts/` directory (`init.md`, `simulation.md`, `visualization.md`, and this prompt) when present.
- Enumerate roadmap domains and all milestone/backlog/story items before choosing work.
- Process the backlog in deterministic batches. Do not attempt to load all items into context at once.
- Select a small, coherent batch:
  - P0 before P1 before P2.
  - Within each priority, prefer foundational inventory, versioned contracts, safety parity, observability, and other observable-foundation work before later milestone work.
  - Choose related items that can be implemented and verified together without broad refactors.
- Atomically claim selected feature IDs in SQLite before editing.

Batch loop:
- Continue through successive completed and committed batches until every roadmap item is processed and verified, or until a hard stop condition is reached. Do not stop merely because a batch was completed, a checkpoint was written, a commit succeeded, or a concise status update could be reported.
- After each verified commit:
  - Update `checkpoint.sqlite` and `RESUME.md`.
  - Re-read the active checkpoint.
  - Verify `git status --short`, `runs.last_commit`, `current_batch_id`, `next_action`, and the roadmap hash.
  - Select and atomically claim the next deterministic batch using the same P0 -> P1 -> P2 priority rules.
  - Optional accelerator: claim multiple pending rows at once with `scripts/roadmap_batch_claim.sh --run-id <id> --batch-size <N> --agent <name>` before implementing.
  - Implement, validate, checkpoint, commit, and repeat.
- Do not stop merely because a batch completed, the next batch is larger, or the worktree is clean after a checkpoint.
- Stop the loop only when a hard stop condition is hit:
  - no pending roadmap items remain
  - validation fails and cannot be fixed within the current batch
  - unrelated worktree changes conflict with the next batch
  - a real blocker prevents progress
  - the user explicitly stops the run
  - context, token, rate-limit, or timeout pressure makes it impossible to continue safely
- Treat context, token, rate-limit, and timeout pressure as hard stop conditions only when there is concrete evidence that continuing another batch would likely lose work or prevent a checkpoint. Do not stop speculatively.
- Before stopping, write a complete checkpoint with the current batch, feature IDs, changed files, commands run, verification state, last commit, blocker if any, and exact next action.
- Codex cannot restart itself after API token, context, or rate limits. If an external runtime reset is required, rely on a harness to relaunch Codex with this prompt; SQLite and `RESUME.md` are the handoff contract.

Architecture guardrails:
- `flight_sim_cpp` is the single canonical simulator for both interactive viewing and headless deterministic regression.
- Do not revive or reintroduce the retired Rust/Bevy `simulator` crate or Rust `drone_simulator` crate unless the user explicitly asks for that architectural change.
- Keep Rust workspace work inside the active crates named in AGENTS.md.
- Favor deterministic, evidence-backed processing over opaque heuristics.

Execution:
- Use existing repo patterns and local helper APIs.
- Use TDD for non-trivial behavior: write or update the smallest meaningful failing test first, implement, then refactor with tests passing.
- Keep domain rules in small, named Rust modules or pure functions; keep handlers/routes/controllers thin.
- Keep persistence, hardware/sensor clients, UI, and external integrations outside core domain logic.
- For GIS and visualization work, update focused tests in `shared`, `geo_hub`, `geo_viewer`, or the touched crate as appropriate.
- For FlightSim work, keep changes in `flight_sim_cpp` and validate with the C++ simulator test path.
- Do not mock AGBot's own domain logic just to pass tests. Mock only true external boundaries such as hardware, MAVLink links, serial devices, cameras/LiDAR, network services, tile servers, or clocks.
- Never revert unrelated user changes.

Validation:
- Run the smallest meaningful validation for the changed surface, then broaden only when risk warrants it.
- Rust domain logic: `cargo test -p <crate> <test_name>`.
- Rust crate check: `cargo check -p <crate>`.
- Cross-crate, route, dependency, proto, or shared-contract changes: `cargo check`.
- GIS regression: `just gis-test`; use `just gis-acceptance` for advisor workflow changes.
- C++ FlightSim: `just flight-sim-build` and `just flight-sim-test`.
- Formatting/lint when appropriate: `cargo fmt`, `cargo clippy -- -D warnings`, `just fmt`, or `just clippy`.
- Record every important command and result in the checkpoint `events` table.
- Treat existing warnings as context; command exit codes and new failures are the signal.

Checkpoint and commit:
- Update `checkpoint.sqlite` and `RESUME.md` after selection, claim, implementation, validation, blockers, and completion.
- Use the statuses from AGENTS.md: `pending`, `claimed`, `in_progress`, `implemented`, `tests_passed`, `committed`, `blocked`, `skipped`.
- Treat a git commit as the strongest checkpoint.
- If validation passes, create one clear git commit for the completed batch.
- Stage only files belonging to this batch and its checkpoint updates.
- After committing, update:
  - `runs.last_commit`
  - `runs.current_batch_id`
  - `runs.next_action`
  - `batches.commit_sha`
  - completed `feature_progress.commit_sha`
  - `RESUME.md`
- Keep `RESUME.md` tiny: run ID, roadmap hash, last implementation commit, latest checkpoint commit when different, current batch, completed batch count, blocker if any, exact next action, and latest verification evidence.

Git sandbox handling:
- Run read-only git commands such as `git status`, `git diff`, `git log`, and `git show` normally.
- Treat mutating git commands such as `git add`, `git commit`, `git tag`, `git merge`, `git rebase`, `git cherry-pick`, `git stash`, and `git reset` as `.git` writes that may require escalated permissions in restricted workspaces.
- If a required mutating git command fails with `Permission denied`, `Operation not permitted`, or `Unable to create .git/index.lock`, rerun only the necessary git command with escalated permissions and a concise justification.
- Do not work around sandbox failures by copying the repository, manually editing `.git`, using `sudo`, or running destructive cleanup.
- Never run destructive git operations unless the user explicitly requested them.

Subagents:
- If subagents are available and the current runtime permits using them for this prompt, use 2-4 independent lanes for backlog triage, crate implementation, GIS/visualization work, tests, or independent verification.
- Use SQLite claims as the coordination source of truth.
- Do not let two lanes edit the same crate, module, route, controller, migration, generated file, or C++ component at the same time.
- Keep one coordinator responsible for final integration, validation, staging, commits, and checkpoint updates.
- If subagents are unavailable or not permitted, run the same roles sequentially while keeping the SQLite claim/checkpoint protocol.

If blocked:
- Do not spin.
- Record the blocker, evidence, failed command if any, verification state, and exact next action in `RESUME.md` and `checkpoint.sqlite`.
- Report the blocker concisely.

Context and resume discipline:
- After every committed batch, update SQLite and `RESUME.md`.
- Continue to the next deterministic batch after each checkpoint unless a hard stop condition applies.
- If context is genuinely too low to complete, validate, and checkpoint the next batch safely, stop starting new implementation work and write a complete checkpoint first.
- On resume, read `AGENTS.md`, the active `RESUME.md`, and the SQLite checkpoint; verify roadmap hash, last commit, and `git status --short`; then continue from `runs.next_action`.
- Do not re-enumerate the full roadmap after resume unless the roadmap hash changed or the checkpoint is invalid.

Final response:
- Lead with the outcome.
- Include selected feature IDs, files changed, validation commands, commit SHA if committed, and remaining blockers or next action.
```
