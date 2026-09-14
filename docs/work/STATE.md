# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-14 (M3 step 7, acceptance re-check and fault injection, done).
- **Owner/current task:** Protocol session (Claude Code, Opus 5) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
  - Current milestone: [M3 task](release-0.1/M3.md), the Execution profile.
  - Detailed state and evidence: [release handoff](release-0.1/HANDOFF.md).
- **Merged:**
  - PR #2 (M0 and M1) as `f42d21a`.
  - PR #3 (M2) as `7bd5cb9`, after the owner accepted M2's scope and documented limitations on 2026-09-14. [PR #3 checks](https://github.com/Combraton/protocol/pull/3/checks) passed on its final head `e7937ef`.
- **Branch:** `release-0.1/m3` from `7bd5cb9`, in draft [PR #4](https://github.com/Combraton/protocol/pull/4); not merged.
- **Done in M3 so far:**
  - **Step 1:** Execution contract, Core effects draft, refined matrix, task packet.
  - **Owner decisions:** Q1–Q4; decision 007 accepted with refinements.
  - **Step 2:** test controls, idle-expiry fixtures, deterministic race regression, clock robustness.
  - **Steps 3–4 (base slice):** Core effects and the execution base profile over the scripted executor.
  - **Owner decisions D1–D5**, implemented in specs, schemas, reference and fixtures ([M3 status](release-0.1/M3.md#status)):
    - delivery determinations with history;
    - restart recovery with a write-ahead marker, fencing and revalidation;
    - negotiated Core-feature dependencies with older-participant compatibility;
    - `effect_refs` compatibility and authorized effect reads.
  - **Step 7:** acceptance re-check ([M3 acceptance evidence](release-0.1/M3.md#acceptance-evidence-step-7-re-check)); store faults for `unavailable` and `internal_error`; EXE-20 mutant.
  - **Step 6:** output telemetry with declared lost ranges; `core.events.backpressure` with a bounded outbox, bounded notice, closure and cursor recovery on both bindings.
  - **Step 5:** optional features (steering, actions, controller lease, workspaces, usage and budgets, context bindings, discovery, continuation), capacity and context queueing, the remaining timeouts, retry classes with attempts, obligation abort, detach and reattach, and SCN-12 to SCN-15.
- **Evidence at step 7 (local macOS arm64):**
  - fmt, clippy, build, test and self-test clean; check-fixtures 211;
  - reference over stdio: 199 pass, 12 skipped; reference over the Unix socket: 211 pass;
  - `check-mutants`: 244 stdio and 22 socket mutant-fixture results, all as intended;
  - independent: 151 pass, 12 skipped, 48 unsupported (it does not claim Execution yet).
  - Step 5 CI on `d8aa553` passed on Ubuntu and macOS.

  CI: [PR #4 checks](https://github.com/Combraton/protocol/pull/4/checks).
- **Owner decisions recorded:**
  - scope accepted; macOS and Linux only; MIT; Rust tooling;
  - decision records 001–006 accepted (001 amended for distinct outcomes);
  - M2 accepted;
  - M3 acceptance must keep effects/reconciliation, telemetry gaps, backpressure, fault injection, a controllable test clock, idle-expiry coverage, a deterministic concurrency regression where feasible, test controls outside the protocol, a scripted executor, independent checks and uploaded CI evidence.
- **Open owner decisions:** none pending. Machine-readable feature-dependency advertising is an explicit M6 decision (CMP-5).
- **Prompt disposition:** no active continuation prompt. The workspace M2 prompt is retired, and this file and the handoff are the entry points.
- **Next action:** M3 step 8 (independent spec-only pass in worktree `.worktrees/independent-m3`), then close-out.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
