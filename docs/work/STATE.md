# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-15 (M5 accepted and merged; M6 started with the release-readiness plan).
- **Owner/current task:** Protocol session (Claude Code) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
  - Current milestone: [M6 task](release-0.1/M6.md), the release candidate.
  - Detailed state and evidence: [release handoff](release-0.1/HANDOFF.md).
- **Merged:**
  - PR #2 (M0 and M1) as `f42d21a`.
  - PR #3 (M2) as `7bd5cb9`, after the owner accepted M2.
  - PR #4 (M3) as `481cf7f`, after the owner's conditional acceptance with corrections C1–C3 on 2026-09-14.
  - PR #5 (M4) as `ee82afb`, after the owner accepted M4 at `86e128f` on 2026-09-15 with its documented coverage limits.
  - PR #6 (M5) as `6ed4727` on 2026-09-15:
    - The owner accepted M5 at `00b3c5a` as a completed milestone with its documented limitations, explicitly not accepting or releasing Protocol 0.1.
    - [PR #6 checks](https://github.com/Combraton/protocol/pull/6/checks) passed on that head; the merge was pinned with `--match-head-commit`.
    - The merge commit's tree equals `00b3c5a`'s tree.

  M2–M5 are accepted milestones; Protocol 0.1 is not accepted or released.
- **Worktrees:** `.worktrees/independent-m5` and branch `release-0.1/m5-independent` were removed on 2026-09-15 after checks: no uncommitted changes, the branch tip `53ec6bf` is an ancestor of the merge, and pass-11/12/13 evidence is preserved under `conformance/results/independent-m5-pass-evidence/`.
- **Branch:** `release-0.1/m6` from `6ed4727`, in draft [PR #7](https://github.com/Combraton/protocol/pull/7); not merged.
- **Done in M6:** the release-readiness plan and acceptance matrix ([M6](release-0.1/M6.md)) and the consumer-handoff draft ([CONSUMERS](release-0.1/CONSUMERS.md)).
- **Open owner decisions:** M6-Q1 (CMP-5 mechanism; option A recommended), M6-Q2 (mixed-implementation topology S-A/S-B, S-C opportunistic), M6-Q3 (carried-gap dispositions: four required closures, accepted limitations, deferred backlog) — [M6 §Owner decisions needed](release-0.1/M6.md#owner-decisions-needed).
- **Owner decisions recorded:** scope; macOS and Linux only; MIT; Rust tooling; decision records 001–007; M2, M3 (C1–C3), M4 and M5 accepted with their limits; M4-Q1..Q7; M5-Q1..Q9. Publishing or tagging Protocol 0.1 requires the owner's explicit authorization.
- **Prompt disposition:** no active continuation prompt; the workspace `START-PROTOCOL.md` stays retired and points here.
- **Next action:** owner decisions M6-Q1..Q3, then M6 step 2 (consequential gap closures). Routine work continues autonomously; genuinely new compatibility or scope decisions go to the owner with a recommendation.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
