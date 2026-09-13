# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1) and running processes before acting.

- **Task and timestamp:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1). Owner: Protocol session (Claude Code, Opus 5). Checkpoint: 2026-09-13T08:20Z, end of M0 (scope, matrix and proposed decisions).
- **Goal and acceptance:** see [PLAN](PLAN.md) and [MATRIX](MATRIX.md).
  - The scope is **proposed and not accepted**. Owner decisions U1–U6 are open ([PLAN §6](PLAN.md#6-unresolved-choices-that-affect-scope-or-public-semantics)).
  - Constraints preserved: Protocol owns contracts and fixtures only. No PIO, CBR, Combraton or benchmark runner implementation. Coordination and Remote trust are unsupported in 0.1.
- **Git state:**
  - Repository `Combraton/protocol`, branch `release-0.1/foundation`, base `main` at `f654a29bd6574a75d8ce7c7b76d6a67ef22b45ea`.
  - The M0 documentation commit is the first commit on this branch (see `git log main..release-0.1/foundation`). No PR exists yet.
- **Dependency state:** requirements read at combraton `9af69ce`, pio `e65b7c0`, cbr `3278393`, benchmarks `c8d5878` (full SHAs in [PLAN §2](PLAN.md#2-pinned-sources)).
- **What changed in M0:**
  - [PLAN](PLAN.md) and [MATRIX](MATRIX.md).
  - Draft normative [Core profile](../../spec/profiles/CORE.md), [ENCODING](../../spec/bindings/ENCODING.md) and [STREAM](../../spec/bindings/STREAM.md).
  - Proposed [decision records 001–005](../../decisions/README.md).
  - [Conformance README](../../../conformance/README.md) stub; documentation map updates.
  - Tracking issue #1 created on GitHub.
- **Research inputs:** three read-only research helpers (conformance precedents, framing/authentication, schema/digests) ran on 2026-09-13. Their cited, pinned sources are in the decision records. Their scratch copies lived in the session scratchpad and are not durable.
- **Evidence:** `python3 scripts/check_docs.py --workspace ..` and `git diff --check` were run before the M0 commit; results are recorded in [STATE](../STATE.md). These checks validate documentation structure only. No runtime, schema or conformance check exists yet.
- **What remains uncertain:**
  - Owner decisions U1–U6.
  - The Unix-socket principal credential (U3/U9), which affects M2.
  - Whether environment-only test control can reach every Core state. M1 will show this; unreachable states are reported as `untestable`.
- **Active resources:** none started by this task. Other Claude/Codex sessions on the machine were not working in the Combraton workspace when inspected.
- **State and prompt disposition:** [STATE](../STATE.md) updated. The workspace-local Protocol kickoff prompt remains active; it is to be rewritten to the remaining work at the session's end checkpoint.
- **Next action:** implement M1 on this branch, per [PLAN §5 M1](PLAN.md#m1--core-command-path-and-conformance-harness-first-bounded-implementation-milestone). First confirm the branch head and issue state.
