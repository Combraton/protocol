# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-15, M4 step 2 (Evidence reference provider) done; owner decisions M4-Q1 to M4-Q7 incorporated.
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
  - step 2: `schemas/evidence/1`, the reference Evidence provider (`conformance/reference/src/evidence.rs`) with its scripted store control `evidence_store`, 13 fixtures in `conformance/fixtures/evidence` and 23 new mutants ([M4 status](M4.md#status)).

## Evidence

- **M3 merge:** CI at `19e0e16` (Conformance run 34878446254). Ubuntu artifact: reference stdio 200 pass and 12 skipped; Unix socket 212 pass; independent 196 pass, 4 unsupported, 12 skipped; mutants 252/252 and 25/25; race regression 20/20 each way. Details in [M3 status](M3.md#status) and [M3-DIVERGENCES](M3-DIVERGENCES.md).
- **M3 helper run evidence** (not committed; local, gitignored): `conformance/results/independent-m3-pass-evidence/` and `conformance/results/independent-c1-evidence/`: first runs, fix runs, stability and sensitivity runs.
- **M4 step 1:** documentation only; `scripts/check_docs.py` passes.
- **M4 step 2 (local, macOS):** `check-fixtures` 225 fixtures ok; reference stdio 213 pass and 12 skipped; Unix socket 225 pass; independent 196 pass, 12 skipped, 17 unsupported (the 13 Evidence fixtures are unsupported: it does not claim `evidence/1`); each of the 25 mutants the Evidence fixtures declare fails every fixture declaring it; `cargo fmt --check`, `clippy -D warnings` and `cargo test` pass. CI evidence is on [PR #5 checks](https://github.com/Combraton/protocol/pull/5/checks).

## What remains uncertain

- **To mention at the next owner checkpoint:** additions beyond the approved lists (`upload_offset_mismatch`, `upload_size_exceeded`, `evidence.upload.abandon`) and the step 2 clarifications recorded in [M4](M4.md#owner-decisions-2026-09-15), notably query `filtered` meaning a restricted view rather than a withheld match.
- **Evidence is reference-only** until step 7's independent pass.
- **Carried from M3:**
  - the independent provider has no Unix socket or backpressure support;
  - barrier- and signal-synchronized fixtures are coverage limits for other participants;
  - all Execution evidence is scripted, not real-adapter evidence.
- **Deferred decision:** machine-readable feature-dependency advertising is an M6 decision (CMP-5).
- **M2 limits carried forward:** root is the only other OS user tested; capability evidence-source and cursor-past-head evidence is reference-only.

## Active resources

None.

## State and prompt disposition

[STATE](../STATE.md) is updated. The workspace `START-PROTOCOL.md` prompt is retired and points to STATE and this handoff.

## Next action

1. M4 step 3: Context reference provider (`context/1` schemas, scripted preparation, packets published as Evidence artifacts at the provider itself) with fixtures and mutants for the single-provider CTX rows.
2. Then steps 4–8 of the [M4 work plan](M4.md#work-plan).
