# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-14, M3 step 6 (output telemetry and backpressure) done.
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
  - **Steps 3–4 and D1–D5:** Core effects, the execution base profile and the owner's five contract decisions ([M3 status](M3.md#status)).
  - **Step 5:**
    - **contract:** EXECUTION §5, §7, §8, §9, §11.1–§11.6, §12, §13; CORE §19.2–§19.4;
    - **schemas:** six new operations; submit, inspect, `core.effects.get` and the launch configuration extended;
    - **reference:** `conformance/reference/src/features.rs` plus executor changes in `execution.rs`; 30 new mutants;
    - **fixtures:** 22 new; generator kept outside the repository, as for earlier steps.
  - **Step 6:**
    - **contract:** EXECUTION §14.1 and CORE §16.5 backpressure (bounded outbox, wait for room, bounded notice, closure, recovery);
    - **reference:** `conformance/reference/src/outbox.rs`, output spool in `execution.rs`, six new mutants;
    - **runner:** `pause_reading`, `resume_reading`, `collect_until_close`;
    - **fixtures:** five new, on both bindings.

## Evidence

- **M2:** the close-out table in [M2](M2.md) and the [PR #3 checks](https://github.com/Combraton/protocol/pull/3/checks).
- **M3 step 2:** issue #1 checkpoint and PR #4 checks: race regression 20/20 both ways, idle expiry, clock robustness, coverage limits.
- **M3 steps 3–4 base slice:** issue #1 checkpoint and PR #4 checks.
- **Owner decisions D1–D5:** issue #1 checkpoint and PR #4 checks.
- **M3 step 5:** PR #4 checks on `d8aa553` passed on Ubuntu and macOS.
- **M3 step 6, local on macOS arm64:**

| Check | Result |
|---|---|
| `cargo fmt --check`, `cargo clippy -D warnings`, `cargo build --locked`, `cargo test --locked`, `self-test` | clean |
| `check-fixtures` | 209 fixtures, 138 matrix IDs, ok |
| `run` reference stdio | 197 pass, 12 skipped (Unix-socket fixtures) |
| `run` reference Unix socket | 209 pass |
| backpressure fixtures repeated | 3 runs on each binding, all pass |
| `check-mutants`, both bindings | all killed as intended (see `mutants.json`) |
| `run` independent Python | 151 pass, 12 skipped, 46 unsupported (Execution, backpressure signals and the clock file not claimed) |
| `python3 scripts/check_docs.py` | 0 errors |

- **CI:** [PR #4 checks](https://github.com/Combraton/protocol/pull/4/checks).

## What remains uncertain

- **Deferred decision.** Machine-readable feature-dependency advertising is an explicit M6 decision (CMP-5).
- **Independent implementation:** it does not yet claim `execution/1`, `core.effects`, the clock file or the executor script. Those fixtures are recorded as unsupported until the M3 spec-only pass.
- **Execution authorization under grants:** covered for effect reads, recovery revalidation and feature rights; submit and cancel denials are exercised only indirectly.
- **Session-close evidence on the socket binding** uses the reference-specific signal `session.closed`; other socket participants report that fixture as unsupported.
- **Backpressure evidence is reference-specific in its synchronization.** The four slow-consumer fixtures wait for the reference's `backpressure.*` signals; participants without them report coverage limits. The wait for room reuses `backpressure_notice_ms` as its bound (CORE §16.5).
- **Scripted evidence only.** Every Execution fixture drives the scripted executor. None of it is evidence about a real harness adapter, which stays PIO's gate.
- **M2 limits carried forward:** root is the only other OS user tested; capability evidence-source and cursor-past-head evidence is reference-only.

## Active resources

None.

## State and prompt disposition

[STATE](../STATE.md) is updated. The workspace M2 continuation prompt is retired; STATE and this handoff are the entry points.

## Next action

1. M3 step 7: re-check the acceptance items; add fault injection for `unavailable` (nothing bound) and `internal_error` (outcome unknown) on execution commands, and a killing mutant for EXE-20.
2. Steps 8–9 per the [M3 task](M3.md): the independent spec-only pass with a disposition of its clock-file coverage limits, close-out.
