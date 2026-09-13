# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-13T18:50Z.
- **Owner/current task:** Protocol session (Claude Code, Opus 5) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
  - Current milestone: [M2 task](release-0.1/M2.md).
  - Detailed state and evidence: [release handoff](release-0.1/HANDOFF.md).
- **Merged:** PR #2 (M0 and M1) into `main` as `f42d21a`.
- **Branch:** `release-0.1/m2`, pushed, not merged, no PR yet.
- **Done in M2:**
  - Grants, events and subscriptions, and capabilities: spec, schemas, reference provider, fixtures, mutants.
  - Unix-socket binding with principal credentials ([decision 006](../decisions/006-unix-socket-principal-credential.md)).
  - Two spec-only independent passes (Core, then grants, events and capabilities). Their findings are resolved in [M2-DIVERGENCES](release-0.1/M2-DIVERGENCES.md) sections A–E.
  - 144 fixtures; 123 reference mutants (117 stdio, 6 socket).
- **Evidence:** at the section E resolution commit, locally on macOS arm64:
  - Every [VERIFICATION](../VERIFICATION.md) command for the reference provider exits 0 over stdio and the Unix socket, and every declared mutant is killed.
  - The independent Python run exits 1: 8 fixtures encode decisions made after its pass. CI's independent step is therefore expected to fail until it catches up.
- **Owner decisions recorded:** scope accepted; macOS and Linux only; MIT license; decision records 001–006 accepted; Rust tooling; U12 credential file plus `core.authenticate`.
- **Open owner decisions:** none.
- **Prompt disposition:** the workspace-local continuation prompt points to this file.
- **Next action:**
  1. Bring the independent implementation level with section E through a spec-only pass.
  2. Re-enable the held-back stdio `already_authenticated` fixture.
  3. Confirm CI.
  4. Open the M2 PR for owner review.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
