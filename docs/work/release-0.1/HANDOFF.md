# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-15, M3 merged; M4 step 1 (contracts and acceptance matrix) drafted.
- **Status:** M0–M3 merged. M4 in progress on `release-0.1/m4` ([M4 task](M4.md)). Protocol 0.1 is not released.

## Goal, decisions and constraints

- **Plan:** [PLAN](PLAN.md), [MATRIX](MATRIX.md).
- **Owner decisions:**
  - Scope accepted; macOS and Linux only (U4); MIT (U6); Rust tooling; U12 credential file plus `core.authenticate`.
  - Decision records 001–007 accepted; 001 amended for distinct outcomes; 007 accepted with refinements.
  - M2 accepted on 2026-09-14.
  - M3 conditionally accepted on 2026-09-14 with corrections C1–C3 ([M3 status](M3.md#status)) and merged.
  - M4: contracts and matrix before implementation; EXE-21 explicit; cover exact artifact and packet identity, authorized access, provenance and availability, missing required context, corrections during preparation, stale basis, shared-subscriber cancellation, CBR outage and the preparation/resource dependency cycle; reference participants only.
- **Constraints:**
  - Protocol owns contracts and fixtures only. Nothing is released. Do not implement the real CBR memory engine or PIO adapters.
  - Commit and push as work progresses. Do not merge without authorization.
  - Substantive contract changes or unresolved decisions go to the owner; routine work continues.
  - Never create a records-only commit merely to note that the previous commit passed CI; link PR checks.

## Git state

- **`main`** at `481cf7f`, the merge of PR #4. Its final head `19e0e16` passed all [PR #4 checks](https://github.com/Combraton/protocol/pull/4/checks).
- **Branch** `release-0.1/m4` from `481cf7f`, in a draft PR.
- **Worktrees:** none. Both M3 helper worktrees were removed after their sources were verified as merged.
- **Local-only branches:** `release-0.1/m2`, `release-0.1/m3` and `release-0.1/m3-independent` are merged; kept, not deleted.

## What exists

- **Merged on `main`:**
  - Core §1–§19, STREAM (stdio and Unix socket), ENCODING;
  - the Execution profile with its optional features, telemetry and backpressure;
  - 212 fixtures, the reference provider and its mutants, the independent Python provider (Core, effects and Execution);
  - CI with distinct outcomes and uploaded result artifacts.
- **On `release-0.1/m4`, step 1:**
  - `docs/spec/profiles/EVIDENCE.md` and `docs/spec/profiles/CONTEXT.md` drafts;
  - refined MATRIX rows EVD-1 to EVD-8, CTX-1 to CTX-20, EXE-21, SCN-1, SCN-4 to SCN-6, SCN-8, SCN-9, SCN-11;
  - the M4 task packet with owner questions M4-Q1 to M4-Q7.

## Evidence

- **M3 merge:** CI at `19e0e16` (Conformance run 34878446254). Ubuntu artifact: reference stdio 200 pass and 12 skipped; Unix socket 212 pass; independent 196 pass, 4 unsupported, 12 skipped; mutants 252/252 and 25/25; race regression 20/20 each way. Details in [M3 status](M3.md#status) and [M3-DIVERGENCES](M3-DIVERGENCES.md).
- **M3 helper run evidence** (not committed; local, gitignored): `conformance/results/independent-m3-pass-evidence/` and `conformance/results/independent-c1-evidence/`: first runs, fix runs, stability and sensitivity runs.
- **M4 step 1:** documentation only; `scripts/check_docs.py` passes. No conformance behavior changed.

## What remains uncertain

- **M4 decisions pending:** byte transfer, packets as Evidence artifacts, cross-profile conformance topology, stale-basis revalidation, new error codes, purge with holds, manifest format (M4-Q1 to M4-Q7).
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

1. Owner decisions on M4-Q1 to M4-Q7.
2. M4 step 2: runner topology for cross-profile cases and the scripted evidence store and preparation vocabularies.
