# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-14 (M3 step 2, test controls, done).
- **Owner/current task:** Protocol session (Claude Code, Opus 5) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
  - Current milestone: [M3 task](release-0.1/M3.md), the Execution profile.
  - Detailed state and evidence: [release handoff](release-0.1/HANDOFF.md).
- **Merged:**
  - PR #2 (M0 and M1) as `f42d21a`.
  - PR #3 (M2) as `7bd5cb9`, after the owner accepted M2's scope and documented limitations on 2026-09-14. [PR #3 checks](https://github.com/Combraton/protocol/pull/3/checks) passed on its final head `e7937ef`.
- **Branch:** `release-0.1/m3` from `7bd5cb9`, in draft [PR #4](https://github.com/Combraton/protocol/pull/4); not merged.
- **Done in M3 so far:**
  - **Step 1:** Execution contract draft, CORE §19 effects draft, refined matrix, task packet.
  - **Owner decisions Q1–Q4**, and decision 007 accepted with refinements.
  - **Step 2:**
    - test controls outside the protocol: clock file, pipelined sends, `kill`, barriers and signals, coverage limits, kill expectations;
    - idle-expiry fixtures;
    - the deterministic regression of the M2 idle re-check race;
    - clock robustness fixtures.
- **Evidence (step 2, local macOS arm64):**
  - fmt, clippy, build and test clean; check-fixtures 160;
  - reference stdio: 150 pass, 10 skipped; reference Unix socket: 160 pass;
  - `check-mutants` on both: every mutant killed as intended, per `mutants.json`;
  - independent: 148 pass, 10 skipped, 2 unsupported (clock-file coverage limits);
  - race regression: 20/20 passes on the correct provider and 20/20 intended failures on `recheck-outside-lock`;
  - peer-user check `unsupported` locally (runs in CI).

  CI: [PR #4 checks](https://github.com/Combraton/protocol/pull/4/checks).
- **Owner decisions recorded:**
  - scope accepted; macOS and Linux only; MIT; Rust tooling;
  - decision records 001–006 accepted (001 amended for distinct outcomes);
  - M2 accepted;
  - M3 acceptance must keep effects/reconciliation, telemetry gaps, backpressure, fault injection, a controllable test clock, idle-expiry coverage, a deterministic concurrency regression where feasible, test controls outside the protocol, a scripted executor, independent checks and uploaded CI evidence.
- **Open owner decisions:** none pending. Substantive contract changes found during implementation go back to the owner.
- **Prompt disposition:** no active continuation prompt. The workspace M2 prompt is retired, and this file and the handoff are the entry points.
- **Next action:** M3 steps 3–4, Core effects together with the execution base profile and the scripted executor.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
