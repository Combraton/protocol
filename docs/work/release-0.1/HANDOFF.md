# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-14, M3 step 1.
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
- **Branch.** `release-0.1/m3` from `7bd5cb9`. The M3 PR link is in the issue #1 comment for this checkpoint and in `gh pr list`.
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
- **On `release-0.1/m3` (step 1, documentation):**
  - `docs/spec/profiles/EXECUTION.md` (proposed draft);
  - CORE §19 effects and obligations (proposed draft);
  - `docs/decisions/007-execution-test-controls.md` (proposed);
  - refined MATRIX rows for M3, with new rows REL-12, REL-13, OBS-9 and SCN-12 to SCN-15;
  - the M3 task packet with work plan and open questions Q1–Q4.

## Evidence

- **M2:** the close-out table in [M2](M2.md) and the [PR #3 checks](https://github.com/Combraton/protocol/pull/3/checks), whose uploaded artifacts hold every manifest.
- **M3 step 1:** documentation only. `python3 scripts/check_docs.py` reports 0 errors, and `check-fixtures` reports 155 fixtures and 134 matrix IDs ok, locally. CI evidence is the M3 PR's checks.

## What remains uncertain

- **Q1–Q4 and decision 007** await owner review:
  - completion submission internal or public;
  - discovery scope;
  - the `consumer_too_slow` ended reason;
  - the scripted vocabulary status.
- **Draft names.** Execution operation and event names are candidates until schemas and fixtures land.
- **Race evidence.** The deterministic race regression depends on the barrier design in decision 007; until step 2 lands, the M2 lock fix rests on code review.
- **M2 limits carried forward:** root is the only other OS user tested; capability evidence-source and cursor-past-head evidence is reference-only.

## Active resources

None.

## State and prompt disposition

[STATE](../STATE.md) is updated. The workspace M2 continuation prompt is retired; STATE and this handoff are the entry points.

## Next action

1. M3 step 2: test controls (clock file, `set_clock`, `kill`, `send`/`expect_response`, `pause_reading`, barriers) in the runner and reference; the idle-expiry fixture; the barrier regression with mutant `recheck-outside-lock`.
2. Bring Q1–Q4 and decision 007 to the owner at the next checkpoint.
