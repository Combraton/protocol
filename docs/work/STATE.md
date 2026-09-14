# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-14 (M2 close-out pass complete).
- **Owner/current task:** Protocol session (Claude Code, Opus 5) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
  - Milestone awaiting acceptance: [M2 task](release-0.1/M2.md), including its close-out pass table.
  - Detailed state and evidence: [release handoff](release-0.1/HANDOFF.md).
- **Merged:** PR #2 (M0 and M1) into `main` as `f42d21a`.
- **Branch:** `release-0.1/m2` in [PR #3](https://github.com/Combraton/protocol/pull/3). It is open for owner acceptance and not merged.
- **Done in M2:**
  - grants, events and subscriptions, and capabilities;
  - the Unix-socket binding (decision 006);
  - three resolved independent passes;
  - the owner-requested close-out pass.

  Totals: 155 fixtures and 147 reference mutants.
- **Evidence:** Conformance CI run 34821316336 at `8530c26` succeeded on Ubuntu and macOS. Its uploaded result manifests show:
  - reference over stdio: 148 pass, 7 skipped;
  - reference over the Unix socket: 155 pass;
  - independent implementation: 148 pass, 7 skipped;
  - every declared mutant killed;
  - peer-user check `pass`, and its mutant check `pass`.

  CI for the later records-only commits is shown in PR #3's checks. `0f84d6c` passed Conformance run 34827777661 and Documentation.
- **Owner decisions recorded:**
  - scope accepted;
  - macOS and Linux only;
  - MIT license;
  - Rust tooling;
  - decision records 001–006 accepted, with 001 amended for distinct `unsupported` and `skipped` outcomes;
  - U12: credential file plus `core.authenticate`;
  - M3 starts only after M2 acceptance, in a separate PR, with explicit acceptance criteria.
- **Open owner decision:** acceptance of M2 (PR #3).
- **Prompt disposition:** the workspace-local continuation prompt is the single active pointer to this file; earlier versions are retired.
- **Next action:**
  1. Owner reviews and accepts PR #3. Merge only with authorization.
  2. After acceptance, start M3 in a separate PR per [M3](release-0.1/M3.md).

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
