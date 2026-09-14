# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-14, M3 steps 3–4 base slice.
- **Status:** M0, M1 and M2 merged. M3 in progress on `release-0.1/m3` ([M3 task](M3.md)).

## Goal, decisions and constraints

- **Plan:** [PLAN](PLAN.md), [MATRIX](MATRIX.md).
- **Owner decisions:**
  - Scope accepted; macOS and Linux only (U4); MIT (U6); Rust tooling.
  - Decision records 001–006 accepted; 001 amended for distinct outcomes.
  - U12: credential file plus `core.authenticate`.
  - **M2 accepted on 2026-09-14** with its documented limitations.
  - M3 is a separate PR. Its acceptance keeps:
    - effects/reconciliation;
    - telemetry gaps and backpressure;
    - fault injection;
    - a controllable test clock;
    - idle-expiry coverage;
    - a deterministic concurrency regression where feasible;
    - test controls outside the production protocol;
    - a scripted executor, with real harness adapters remaining PIO work;
    - independent checks and uploaded CI evidence.
- **Constraints:**
  - Protocol owns contracts and fixtures only. Nothing is released.
  - Commit and push as work progresses. Do not merge without authorization.
  - Substantive code changes found necessary before a merge are presented for review first.
  - Never create a records-only commit merely to note that the previous commit passed CI; link PR checks.

## Git state

- **Merges.**
  - `main` at `7bd5cb9`, the merge of PR #3.
  - PR #3's final head `e7937ef` passed all checks: see [PR #3 checks](https://github.com/Combraton/protocol/pull/3/checks).
- **Branch.** `release-0.1/m3` from `7bd5cb9`, in draft [PR #4](https://github.com/Combraton/protocol/pull/4).
- **Worktrees.** None active.

## What exists

- **Merged on `main`:**
  - Core §1–§18;
  - STREAM with stdio and Unix socket;
  - ENCODING;
  - 155 fixtures and 147 reference mutants;
  - the independent Python Core provider;
  - the peer-user check;
  - CI with distinct outcomes and uploaded result artifacts.
- **On `release-0.1/m3`:**
  - **Step 1:**
    - `docs/spec/profiles/EXECUTION.md` and CORE §19 drafts, updated for owner decisions Q1–Q4 (internal completion submission, executor-neutral discovery, negotiated `consumer_too_slow` backpressure, test-adapter scripting);
    - decision 007 accepted with refinements;
    - refined MATRIX;
    - the M3 task packet.
  - **Step 2:**
    - **runner:** pipelined sends, `set_clock`, `kill`, barrier and signal steps, coverage limits, kill expectations, per-mutant reasons in `mutants.json`;
    - **reference:** clock file, barrier and signal, five mutants;
    - **fixtures:** five new;
    - **scripts:** `conformance/scripts/repeat_fixture.py`, run in CI;
    - **docs:** conformance README, VERIFICATION and CORE §13.1 updated.

## Evidence

- **M2:** the close-out table in [M2](M2.md) and the [PR #3 checks](https://github.com/Combraton/protocol/pull/3/checks).
- **M3 step 2:** issue #1 checkpoint and PR #4 checks: race regression 20/20 both ways, idle expiry, clock robustness, coverage limits.
- **M3 steps 3–4 (base slice), local on macOS arm64:**

| Check | Result |
|---|---|
| `cargo fmt --check`, `cargo clippy -D warnings`, `cargo build --locked`, `cargo test --locked` | clean; 4 reference unit tests plus the cursor integration test |
| `check-fixtures` | 172 fixtures, 134 matrix IDs, ok |
| `run` reference stdio | 162 pass, 10 skipped |
| `run` reference Unix socket | 172 pass |
| `check-mutants`, both bindings | all killed as intended; 186 stdio and 12 socket mutant-fixture results in `mutants.json` |
| `run` independent Python | 148 pass, 10 skipped, 14 unsupported (does not claim `execution/1`; clock-file coverage limits) |
| `repeat_fixture.py` race regression | 20/20 correct; 20/20 mutant failures at step 13 |
| `python3 scripts/check_docs.py` | 0 errors |

- **CI:** [PR #4 checks](https://github.com/Combraton/protocol/pull/4/checks).

## What remains uncertain

- **Owner review.** Five contract clarifications from steps 3–4 await the owner ([M3 status](M3.md#status)): subject kind; feature dependencies not in the manifest; delivery axis semantics; restart recovery; `effect_refs`.
- **Independent implementation:** it does not yet claim `execution/1`, `core.effects`, the clock file or the executor script. Those fixtures are recorded as unsupported until the M3 spec-only pass.
- **Execution authorization under grants:** implemented in the reference (`execution.submit`, `execution.cancel`, `execution.read`), but not yet covered by fixtures.
- **M2 limits carried forward:** root is the only other OS user tested; capability evidence-source and cursor-past-head evidence is reference-only.

## Active resources

None.

## State and prompt disposition

[STATE](../STATE.md) is updated. The workspace M2 continuation prompt is retired; STATE and this handoff are the entry points.

## Next action

1. Owner review of the five step 3–4 contract clarifications.
2. M3 step 5, optional features, then steps 6–9 per the [M3 task](M3.md).
