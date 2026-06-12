# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: ffb4819f8bcfda386fac619644bef0b91c7aaf3a (`batch-22-03`)
- **Latest checkpoint commit**: 88a0a3cf7245e1931ef3c104449bfc8620a97fa7 (`batch-12-11`)
- **Current batch**: none — STORY `22-03` implementation is committed and checkpoint commit is pending
- **Completed feature rows**: 123 committed; 1 blocked; 374 pending rows remain in the full-roadmap inventory
- **Blocker**: STORY `07-11` is blocked on the documented storage-authority confirmation question.

## Latest verification

- `cargo test -p orthomosaic frame_set_qa_reports_gsd_overlap_and_full_field_coverage` — failed as expected before implementation with missing QA APIs
- `cargo test -p orthomosaic frame_set_qa` — pass
- `cargo test -p orthomosaic` — pass
- `cargo fmt --check` — pass
- `cargo check` — pass with existing warnings
- `git diff --check` — pass

## Next action

Commit the checkpoint metadata for STORY `22-03`, then select the next deterministic roadmap batch.
