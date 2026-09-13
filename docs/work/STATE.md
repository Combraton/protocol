# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-13T08:20Z.
- **Owner/current task:** Protocol session (Claude Code, Opus 5) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1). Plan: [release plan](release-0.1/PLAN.md). Handoff: [release handoff](release-0.1/HANDOFF.md).
- **Inspected revisions:** protocol `main` at `f654a29`, clean and equal to `origin/main` at session start. Sibling checkouts combraton `9af69ce`, pio `e65b7c0`, cbr `3278393`, benchmarks `c8d5878`, all clean. No issues or PRs existed in any of the five repositories before issue #1.
- **Branch:** `release-0.1/foundation` (not merged; no PR yet).
- **Completed (M0, documentation):**
  - Proposed release scope, [requirements-to-acceptance matrix](release-0.1/MATRIX.md) and milestone plan.
  - Draft [Core profile](../spec/profiles/CORE.md), [ENCODING](../spec/bindings/ENCODING.md) and [STREAM](../spec/bindings/STREAM.md).
  - Proposed [decision records 001–005](../decisions/README.md).
- **Remaining:**
  - M1 (Core command path and conformance harness) is next.
  - M2–M6 are coarse in the plan.
  - The release scope is **not accepted**. Owner decisions U1–U6 are open in [PLAN §6](release-0.1/PLAN.md#6-unresolved-choices-that-affect-scope-or-public-semantics).
- **Decisions/uncertainty:**
  - All decision records are proposed, not accepted.
  - The Unix-socket principal credential is open (U3/U9).
  - Windows transport is proposed for deferral (U4).
  - A license is required before any usable release (U6).
- **Current validation:** `python3 scripts/check_docs.py --workspace ..` and `git diff --check` must be rerun and recorded at each commit. These are documentation checks only. No runtime, schema-validation or conformance command exists at this checkpoint.
- **Task resources:** none running. Research helpers have finished.
- **Prompt disposition:** the workspace-local Protocol kickoff is still active for this task. Its owner rewrites it to the remaining work at the end-of-session checkpoint.
- **Next action:** implement M1 on `release-0.1/foundation`, then update this snapshot and the handoff with commands, exit statuses and evidence.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
