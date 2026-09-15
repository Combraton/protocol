# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-15 (M4 steps 2 and 3, the Evidence and Context reference providers, done).
- **Owner/current task:** Protocol session (Claude Code, Opus 5) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
  - Current milestone: [M4 task](release-0.1/M4.md), the Evidence and Context profiles.
  - Detailed state and evidence: [release handoff](release-0.1/HANDOFF.md).
- **Merged:**
  - PR #2 (M0 and M1) as `f42d21a`.
  - PR #3 (M2) as `7bd5cb9`, after the owner accepted M2.
  - PR #4 (M3) as `481cf7f`, after the owner's conditional acceptance with corrections C1–C3 on 2026-09-14. [PR #4 checks](https://github.com/Combraton/protocol/pull/4/checks) passed on its final head `19e0e16`. M3 is accepted; Protocol 0.1 is not.
- **Branch:** `release-0.1/m4` from `481cf7f`, in draft [PR #5](https://github.com/Combraton/protocol/pull/5); not merged.
- **Done in M4 so far:**
  - step 1: [EVIDENCE](../spec/profiles/EVIDENCE.md) and [CONTEXT](../spec/profiles/CONTEXT.md) with owner decisions M4-Q1 to M4-Q7, refined [MATRIX](release-0.1/MATRIX.md) rows, and the [M4 task packet](release-0.1/M4.md);
  - step 2: `evidence/1` schemas, the reference Evidence provider with a scripted store, 13 fixtures and 23 new mutants;
  - step 3: `context/1` schemas, the reference Context provider with scripted preparation, 9 fixtures and 19 new mutants.
  - Local evidence is in the [handoff](release-0.1/HANDOFF.md#evidence); CI runs on [PR #5 checks](https://github.com/Combraton/protocol/pull/5/checks).
- **Evidence at M3 merge:** CI at `19e0e16` on Ubuntu and macOS. The uploaded Ubuntu artifact shows:
  - reference over stdio: 200 pass, 12 skipped; Unix socket: 212 pass;
  - independent: 196 pass, 4 unsupported, 12 skipped;
  - mutant results: 252 of 252 and 25 of 25 as intended;
  - race regression: 20/20 each way.
- **Owner decisions recorded:**
  - scope accepted; macOS and Linux only; MIT; Rust tooling;
  - decision records 001–007 accepted (007 with refinements);
  - M2 accepted; M3 conditionally accepted and merged, with corrections C1 (runtime `not_started`), C2 (backpressure timing) and C3 (conformance conventions bind only the conformance executor);
  - M4 scope: Evidence and Context contracts and matrix first; EXE-21 kept explicit; reference participants only, with no real CBR memory engine or PIO adapters;
  - M4-Q1 to M4-Q7 decided on 2026-09-15; M4 is presented for acceptance before merge.
- **Open owner decisions:** none blocking. Machine-readable feature-dependency advertising stays an explicit M6 decision (CMP-5).
- **Prompt disposition:** no active continuation prompt. The workspace `START-PROTOCOL.md` is retired and points here.
- **Next action:** M4 step 4, the multi-participant runner and reference protocol client.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
