# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-13T13:10Z.
- **Owner/current task:** Protocol session (Claude Code, Opus 5) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
  - Current milestone: [M2 task](release-0.1/M2.md).
  - Detailed state and evidence: [release handoff](release-0.1/HANDOFF.md).
- **Merged:** PR #2 (M0 and M1) into `main` as `f42d21a`.
- **Branch:** `release-0.1/m2`, pushed, not merged, no PR yet.
- **Done in M2:**
  - Grants, events and subscriptions, and capabilities: spec, schemas, reference provider, fixtures, mutants.
  - An independent spec-only Python Core provider, whose findings are resolved in [M2-DIVERGENCES](release-0.1/M2-DIVERGENCES.md).
  - 108 fixtures and 78 mutants.
- **Evidence:** all [VERIFICATION](../VERIFICATION.md) commands exit 0 locally at `3b32037`. The CI result for that head was pending at this snapshot.
- **Open owner decision:** U12, the Unix-socket principal credential form. The recommendation is in the M2 task. The Unix-socket binding waits for it.
- **Owner decisions recorded:** scope accepted; macOS and Linux only; MIT license; decision records 001–005 accepted; Rust tooling.
- **Task resources:** a spec-only helper extends the independent implementation to grants, events and capabilities in worktree `.worktrees/independent-python-m2`, branch `release-0.1/m2-independent-python-features`.
- **Prompt disposition:** the workspace-local continuation prompt points to this file.
- **Next action:**
  1. Confirm CI.
  2. Merge and review the helper's work.
  3. Implement the Unix-socket binding after U12.
  4. Open the M2 PR.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
