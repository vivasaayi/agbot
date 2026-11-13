# Release guard verification

The `feature/release-integrity` worktree contained an uncommitted deployment
guard at the time of verification. It records the clean default-branch revision, image ID and platform
after a staging build, rejects a stale default deployment, and pins an
accepted staging image by ID. Production references must be immutable; the
operator still has to establish that the specific artifact passed staging.

On 2026-11-12, `python3 scripts/test-release-integrity.py` passed eight tests,
including real Git fixtures and a failed-build receipt invalidation regression.
`bash -n scripts/env` and `git diff --check` passed. Against the locally
available `linux/arm64` image for `main` revision `f26b780`, the guard wrote
and verified a receipt in a temporary state directory, then rejected default
verification after the receipt was invalidated. This check did not deploy the
worktree's adapter or change the live staging state. A full real staging build
and deploy through this guard remain to be verified after the changes are
committed to the default branch.
