# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-15, M4 steps 1–8 done and presented for owner acceptance. It is not merged.
- **Status:** M0–M3 merged. M4 in progress on `release-0.1/m4` ([M4 task](M4.md)). Protocol 0.1 is not released.

## Goal, decisions and constraints

- **Plan:** [PLAN](PLAN.md), [MATRIX](MATRIX.md).
- **Owner decisions:**
  - Scope accepted; macOS and Linux only (U4); MIT (U6); Rust tooling; U12 credential file plus `core.authenticate`.
  - Decision records 001–007 accepted; 001 amended for distinct outcomes; 007 accepted with refinements.
  - M2 accepted on 2026-09-14.
  - M3 conditionally accepted on 2026-09-14 with corrections C1–C3 ([M3 status](M3.md#status)) and merged.
  - M4: contracts and matrix before implementation; EXE-21 explicit; cover exact artifact and packet identity, authorized access, provenance and availability, missing required context, corrections during preparation, stale basis, shared-subscriber cancellation, CBR outage and the preparation/resource dependency cycle; reference participants only.
  - M4-Q1 to M4-Q7 decided on 2026-09-15 ([M4 decisions](M4.md#owner-decisions-2026-09-15)). Negative fixtures are required for authority, cross-provider, stale-boundary, upload-retry and manifest cases; the independent implementation and cross-profile scenarios stay explicit; M4 is presented for acceptance before merge.
- **Constraints:**
  - Protocol owns contracts and fixtures only. Nothing is released. Do not implement the real CBR memory engine or PIO adapters.
  - Commit and push as work progresses. Do not merge without authorization.
  - Substantive contract changes or unresolved decisions go to the owner; routine work continues.
  - Never create a records-only commit merely to note that the previous commit passed CI; link PR checks.

## Git state

- **`main`** at `481cf7f`, the merge of PR #4. Its final head `19e0e16` passed all [PR #4 checks](https://github.com/Combraton/protocol/pull/4/checks).
- **Branch** `release-0.1/m4` from `481cf7f`, in draft [PR #5](https://github.com/Combraton/protocol/pull/5).
- **Worktrees:** none. Both M3 helper worktrees were removed after their sources were verified as merged.
- **Local-only branches:** `release-0.1/m2`, `release-0.1/m3` and `release-0.1/m3-independent` are merged; kept, not deleted.

## What exists

- **Merged on `main`:**
  - Core §1–§19, STREAM (stdio and Unix socket), ENCODING;
  - the Execution profile with its optional features, telemetry and backpressure;
  - 212 fixtures, the reference provider and its mutants, the independent Python provider (Core, effects and Execution);
  - CI with distinct outcomes and uploaded result artifacts.
- **On `release-0.1/m4`:**
  - step 1: `docs/spec/profiles/EVIDENCE.md` and `docs/spec/profiles/CONTEXT.md` drafts with owner decisions M4-Q1 to M4-Q7; refined MATRIX rows EVD-1 to EVD-8, CTX-1 to CTX-20, EXE-21, CMP-8 and the M4 scenarios;
  - step 2: `schemas/evidence/1`, the reference Evidence provider (`conformance/reference/src/evidence.rs`) with its scripted store control `evidence_store`, 13 fixtures in `conformance/fixtures/evidence` and 23 new mutants ([M4 status](M4.md#status));
  - step 4: runner steps `start_participant`, `stop_participant`, `kill_participant` and `connect` with `participant`; the reference protocol client `conformance/reference/src/peer.rs`; packets sealed at a separate evidence provider; 2 fixtures in `conformance/fixtures/composition` and 3 new mutants;
  - step 3: `schemas/context/1`, the reference Context provider (`conformance/reference/src/context.rs`) with scripted preparation (`context`), packets sealed as Evidence artifacts in its own store, runner pattern `$sha256_base64`, 9 fixtures in `conformance/fixtures/context` and 19 new mutants.

## Evidence

- **M3 merge:** CI at `19e0e16` (Conformance run 34878446254). Ubuntu artifact: reference stdio 200 pass and 12 skipped; Unix socket 212 pass; independent 196 pass, 4 unsupported, 12 skipped; mutants 252/252 and 25/25; race regression 20/20 each way. Details in [M3 status](M3.md#status) and [M3-DIVERGENCES](M3-DIVERGENCES.md).
- **M3 helper run evidence** (not committed; local, gitignored): `conformance/results/independent-m3-pass-evidence/` and `conformance/results/independent-c1-evidence/`: first runs, fix runs, stability and sensitivity runs.
- **M4 step 1:** documentation only; `scripts/check_docs.py` passes.
- **M4 step 2:** CI green on Ubuntu and macOS at `2dc3d71`. Locally, the full stdio `check-mutants` also passed.
- **M4 step 3 (local, macOS):**
  - `check-fixtures`: 234 fixtures ok;
  - reference: stdio 222 pass and 12 skipped; Unix socket 234 pass;
  - independent: 196 pass, 12 skipped, 26 unsupported (it claims neither `evidence/1` nor `context/1`);
  - mutants: each one the Evidence and Context fixtures declare fails every fixture declaring it;
  - `cargo fmt --check`, `clippy -D warnings`, `cargo test` and `check_docs.py` pass.
- **M4 step 4 (local, macOS):**
  - `check-fixtures`: 236 fixtures ok;
  - reference: stdio 222 pass and 14 skipped (compositions need the Unix-socket binding); Unix socket 236 pass;
  - independent: 196 pass, 14 skipped, 26 unsupported;
  - Unix-socket `check-mutants` passes, and so does the per-group mutant check for Evidence and Context;
  - each composition fixture ran as intended in 15 of 15 repeated runs.
- **M4 steps 5 and 6 (local, macOS):**
  - `check-fixtures`: 246 fixtures ok;
  - reference: stdio 226 pass and 20 skipped (compositions need the Unix-socket binding); Unix socket 246 pass;
  - independent: 196 pass, 20 skipped, 30 unsupported;
  - `check-mutants` passes on both bindings;
  - each new composition scenario ran as intended 10 of 10 times;
  - `cargo fmt --check`, `clippy -D warnings`, `cargo test` and `check_docs.py` pass.
- **M4 step 7 (local, macOS, head before close-out):**
  - reference: stdio 229 pass and 20 skipped; Unix socket 249 pass;
  - `check-mutants` passes on both bindings;
  - independent after the seventh pass: 225 pass, 0 fail, 4 unsupported, 20 skipped.
- **CI:** Ubuntu and macOS were green at `39e9dc8`, where the uploaded artifacts show stdio 226 pass and 20 skipped, Unix socket 246 pass, independent 196 pass, 20 skipped and 30 unsupported, and all mutants killed (307 of 307 stdio pairs, 42 of 42 Unix pairs). The final head's results are on [PR #5 checks](https://github.com/Combraton/protocol/pull/5/checks).

## What remains uncertain

- **To mention at the next owner checkpoint:** additions beyond the approved lists (`upload_offset_mismatch`, `upload_size_exceeded`, `evidence.upload.abandon`) and the step 2 clarifications recorded in [M4](M4.md#owner-decisions-2026-09-15), notably query `filtered` meaning a restricted view rather than a withheld match.
- **Evidence and Context are reference-only** until step 7's independent pass.
- **Step 3 clarifications to mention:** the concrete CONTEXT shapes (request, items and checks, packet facts, excerpt without digest, packet byte format) and the rules recorded in [M4 status](M4.md#status).
- **Carried from M3:**
  - the independent provider has no Unix socket or backpressure support;
  - barrier- and signal-synchronized fixtures are coverage limits for other participants;
  - all Execution evidence is scripted, not real-adapter evidence.
- **Deferred decision:** machine-readable feature-dependency advertising is an M6 decision (CMP-5).
- **M2 limits carried forward:** root is the only other OS user tested; capability evidence-source and cursor-past-head evidence is reference-only.

## Active resources

- Worktree `.worktrees/independent-m4` (branch `release-0.1/m4-independent`, merged into `release-0.1/m4`). It is kept until M4 merges.

## State and prompt disposition

[STATE](../STATE.md) is updated. The workspace `START-PROTOCOL.md` prompt is retired and points to STATE and this handoff.

## Next action

1. Owner review of M4 and its [points for the owner](M4.md#points-for-the-owner-at-acceptance). Point 4, whether an execution blocked at dispatch releases its capacity slot, awaits a decision.
2. After acceptance and green checks on the final head, merge PR #5 if the owner authorizes it. Then remove the helper worktree `.worktrees/independent-m4` once its branch is verified as merged; its run evidence is preserved locally under `conformance/results/independent-m4-pass-evidence/`.
