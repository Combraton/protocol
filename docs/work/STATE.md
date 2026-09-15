# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-15 (M3 merged; M4 step 1 drafted).
- **Owner/current task:** Protocol session (Claude Code, Opus 5) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
  - Current milestone: [M4 task](release-0.1/M4.md), the Evidence and Context profiles.
  - Detailed state and evidence: [release handoff](release-0.1/HANDOFF.md).
- **Merged:**
  - PR #2 (M0 and M1) as `f42d21a`.
  - PR #3 (M2) as `7bd5cb9`, after the owner accepted M2.
  - PR #4 (M3) as `481cf7f`, after the owner's conditional acceptance with corrections C1–C3 on 2026-09-14. [PR #4 checks](https://github.com/Combraton/protocol/pull/4/checks) passed on its final head `19e0e16`. M3 is accepted; Protocol 0.1 is not.
- **Branch:** `release-0.1/m4` from `481cf7f`, in a draft PR; not merged.
- **Done in M4 so far:** step 1 drafts: [EVIDENCE](../spec/profiles/EVIDENCE.md), [CONTEXT](../spec/profiles/CONTEXT.md), refined [MATRIX](release-0.1/MATRIX.md) rows (EVD, CTX, EXE-21 and the M4 scenarios), and the [M4 task packet](release-0.1/M4.md). No implementation yet.
- **Evidence at M3 merge:** CI at `19e0e16` on Ubuntu and macOS. The uploaded Ubuntu artifact shows:
  - reference over stdio: 200 pass, 12 skipped; Unix socket: 212 pass;
  - independent: 196 pass, 4 unsupported, 12 skipped;
  - mutant results: 252 of 252 and 25 of 25 as intended;
  - race regression: 20/20 each way.
- **Owner decisions recorded:**
  - scope accepted; macOS and Linux only; MIT; Rust tooling;
  - decision records 001–007 accepted (007 with refinements);
  - M2 accepted; M3 conditionally accepted and merged, with corrections C1 (runtime `not_started`), C2 (backpressure timing) and C3 (conformance conventions bind only the conformance executor);
  - M4 scope: Evidence and Context contracts and matrix first; EXE-21 kept explicit; reference participants only, with no real CBR memory engine or PIO adapters.
- **Open owner decisions:** M4-Q1 to M4-Q7 in the [M4 task](release-0.1/M4.md#questions-for-the-owner). Machine-readable feature-dependency advertising stays an explicit M6 decision (CMP-5).
- **Prompt disposition:** no active continuation prompt. The workspace `START-PROTOCOL.md` is retired and points here.
- **Next action:** owner decisions on M4-Q1 to M4-Q7, then M4 step 2 (runner topology and scripted provider vocabularies).

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
