# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-14, M3 step 2.
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
- **M3 step 2, local on macOS arm64:**

| Check | Result |
|---|---|
| `cargo fmt --check`, `cargo clippy -D warnings`, `cargo build --locked`, `cargo test --locked` | clean |
| `check-fixtures` | 160 fixtures, 134 matrix IDs, ok |
| `run` reference stdio | 150 pass, 10 skipped |
| `run` reference Unix socket | 160 pass |
| `check-mutants`, both bindings | every declared mutant killed as intended; `mutants.json` records failing step and reason |
| `run` independent Python | 148 pass, 10 skipped, 2 unsupported (coverage limit: clock file) |
| `repeat_fixture.py` race regression | 20/20 correct passes; 20/20 `recheck-outside-lock` failures at step 13, "params/ended: missing" |
| `peer_user_check.py` | `unsupported` locally (no passwordless `sudo`); CI runs it with `--require` |
| `python3 scripts/check_docs.py` | 0 errors |

- **Race transcript.**
  - Correct provider: `await_any` was satisfied by signal `processing.lock.contended`.
  - Mutant: `await_any` was satisfied by both responses, and the revoked subscriber then received the put `after-revoke`.
- **CI:** [PR #4 checks](https://github.com/Combraton/protocol/pull/4/checks). Their artifacts hold the manifests, `mutants.json`, the repeat summaries and the peer-user results.

## What remains uncertain

- **Independent implementation:** it does not implement the clock file yet, so its two clock fixtures are coverage limits until the M3 spec-only pass.
- **Barrier scope:** the regression is specific to the reference implementation by design, and the barrier covers one point.
- **Draft names:** Execution operation and event names are candidates until schemas and fixtures land in steps 3–5.
- **M2 limits carried forward:** root is the only other OS user tested; capability evidence-source and cursor-past-head evidence is reference-only.

## Active resources

None.

## State and prompt disposition

[STATE](../STATE.md) is updated. The workspace M2 continuation prompt is retired; STATE and this handoff are the entry points.

## Next action

1. M3 steps 3–4: Core effects (CORE §19) with the execution base profile and the scripted executor, including schemas, fixtures, mutants and SCN-2, SCN-3 and SCN-10.
2. Bring any substantive contract change found while implementing to the owner.
