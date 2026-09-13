# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-13T11:40Z.
- **Owner/current task:** Protocol session (Claude Code, Opus 5) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1). Current milestone: [M2 task](release-0.1/M2.md). Plan: [release plan](release-0.1/PLAN.md). Handoff: [release handoff](release-0.1/HANDOFF.md).
- **Merged:** PR #2 (M0 scope and M1 Core conformance foundation) merged into `main` as `f42d21a` on 2026-09-13, on owner instruction, after Conformance CI succeeded on head `2d966eb`.
- **Owner decisions (2026-09-13):**
  - Scope accepted.
  - macOS and Linux only; Windows unsupported (U4).
  - MIT license (U6).
  - Decision records 001–005 accepted.
  - U12, the Unix-socket principal credential form, is open. A recommendation is in the M2 task.
- **Branches and worktrees:**
  - `release-0.1/m2`: this session.
  - `release-0.1/m2-independent-python`: helper building the spec-only Python Core implementation, in worktree `.worktrees/independent-python` (git-ignored directory).
- **Completed on `main`:** M0 and M1. The 57-fixture Core suite, 32 mutants, Rust runner and reference provider, and CI are documented in [VERIFICATION](../VERIFICATION.md).
- **In progress (M2):** grants and principal scopes, then events and subscriptions, then capability snapshots. Effects, telemetry lost ranges and backpressure moved to M3.
- **Evidence limits:** no M2 code exists at this snapshot. M1 evidence is unchanged from the merged state.
- **Task resources:** one background helper, the independent implementation in the worktree above. No servers or daemons.
- **Prompt disposition:** the workspace-local continuation prompt points to this file; rewrite it at the next checkpoint.
- **Next action:** implement grants (CORE.md section, schemas, reference provider, mutants, fixtures). Merge and review the independent implementation when the helper reports. Ask the owner about U12 at the next checkpoint.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
