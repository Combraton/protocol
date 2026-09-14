# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-14 (M3 step 1 drafted).
- **Owner/current task:** Protocol session (Claude Code, Opus 5) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
  - Current milestone: [M3 task](release-0.1/M3.md), the Execution profile.
  - Detailed state and evidence: [release handoff](release-0.1/HANDOFF.md).
- **Merged:**
  - PR #2 (M0 and M1) as `f42d21a`.
  - PR #3 (M2) as `7bd5cb9`, after the owner accepted M2's scope and documented limitations on 2026-09-14. [PR #3 checks](https://github.com/Combraton/protocol/pull/3/checks) passed on its final head `e7937ef`.
- **Branch:** `release-0.1/m3` from `7bd5cb9`, in a separate M3 PR (see the handoff); not merged.
- **Done in M3 so far (step 1, documentation):**
  - [EXECUTION](../spec/profiles/EXECUTION.md) proposed draft.
  - CORE §19 effects and obligations draft.
  - [Decision 007](../decisions/007-execution-test-controls.md) on test controls (proposed).
  - Refined [matrix](release-0.1/MATRIX.md), with new rows REL-12, REL-13, OBS-9 and SCN-12 to SCN-15.
  - Task packet with explicit acceptance criteria.
- **Evidence:** documentation only. `check_docs` and `check-fixtures` pass locally. The M3 PR's checks are the CI record.
- **Owner decisions recorded:**
  - scope accepted; macOS and Linux only; MIT; Rust tooling;
  - decision records 001–006 accepted (001 amended for distinct outcomes);
  - M2 accepted;
  - M3 acceptance must keep effects/reconciliation, telemetry gaps, backpressure, fault injection, a controllable test clock, idle-expiry coverage, a deterministic concurrency regression where feasible, test controls outside the protocol, a scripted executor, independent checks and uploaded CI evidence.
- **Open owner decisions:** M3 questions Q1–Q4 and decision 007, each with a recommendation in the M3 task. Steps 2–4 proceed without them.
- **Prompt disposition:** no active continuation prompt. The workspace M2 prompt is retired, and this file and the handoff are the entry points.
- **Next action:** M3 step 2, the test controls: clock file, process kills, pipelined sends, paused reading and barriers; the idle-expiry fixture; the deterministic regression of the M2 re-check race.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
