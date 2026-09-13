# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-13T10:55Z.
- **Owner/current task:** Protocol session (Claude Code, Opus 5) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1). Plan: [release plan](release-0.1/PLAN.md). Detailed evidence and next steps: [release handoff](release-0.1/HANDOFF.md).
- **Inspected revisions:** protocol `main` at `f654a29` (unchanged). Sibling checkouts combraton `9af69ce`, pio `e65b7c0`, cbr `3278393`, benchmarks `c8d5878`; none modified.
- **Branch:** `release-0.1/foundation`, pushed; head recorded in the handoff. Not merged. Opened as a draft pull request for review; see issue #1.
- **Completed:**
  - M0: proposed scope, matrix, milestone plan and decision records 001–005.
  - M1: Core command path and stdio binding drafts; JSON Schemas; Rust runner and reference provider with 32 mutants; 57 fixtures; encoding vectors with Python and Node cross-checks; Conformance CI.
- **Evidence:**
  - All documented commands in [VERIFICATION](../VERIFICATION.md) exited 0 locally on macOS arm64.
  - GitHub Conformance CI succeeded on `dce3863` and `d0ae764` (Ubuntu and macOS Rust jobs, non-Rust cross-checks).
  - Limits: stdio only; one provider written by the fixture author; no grants, events, sockets or non-Core profiles; no PIO/CBR integration.
- **Remaining:** owner review of M0/M1; then M2 to M6 per the plan. The release scope is **not accepted**.
- **Decisions/uncertainty:**
  - Decision records 001–005 are proposed.
  - Owner decisions U1–U6 are open (PLAN §6), including the Unix-socket principal credential (U3/U9) needed for M2 sockets, and the license (U6).
  - Owner input 2026-09-13: PIO, CBR and the control plane are Rust. This selected Rust conformance tooling.
- **Task resources:** none running. Build and results directories are git-ignored.
- **Prompt disposition:** the workspace-local kickoff prompt was rewritten to a continuation notice pointing here and to the handoff.
- **Next action:** owner reviews the M1 packet and decides U1–U6. Then start M2 with grants, events and subscriptions, and the spec-only non-Rust Core implementation. Confirm CI on the current head first.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
