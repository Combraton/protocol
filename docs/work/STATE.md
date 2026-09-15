# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-16. The owner found the Protocol 0.1 scope ready and authorized the bounded close-out and the v0.1.0 release process. This commit is the release-preparation head. The merge, tag, release URL and asset checksums are recorded in issue #1 and in the post-release handoff.
- **Owner/current task:** Protocol session (Claude Code) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
  - Current milestone: [M6 task](release-0.1/M6.md), the release candidate.
  - Detailed state and evidence: [release handoff](release-0.1/HANDOFF.md).
  - Release record: [docs/release/0.1](../release/0.1/README.md); consumer handoff: [CONSUMERS](release-0.1/CONSUMERS.md).
- **Merged:**
  - PR #2 (M0 and M1) as `f42d21a`.
  - PR #3 (M2) as `7bd5cb9`, after the owner accepted M2.
  - PR #4 (M3) as `481cf7f`, after the owner's conditional acceptance with corrections C1–C3 on 2026-09-14.
  - PR #5 (M4) as `ee82afb`, after the owner accepted M4 at `86e128f` on 2026-09-15 with its documented coverage limits.
  - PR #6 (M5) as `6ed4727`, after the owner accepted M5 at `00b3c5a` on 2026-09-15 with its documented limitations. The merge tree equals the accepted head's tree.

  M2–M5 are accepted milestones; Protocol 0.1 is not accepted or released.
- **Branch:** `release-0.1/m6` from `6ed4727`, in draft [PR #7](https://github.com/Combraton/protocol/pull/7); the candidate head is the commit carrying this snapshot.
- **Owner decisions M6-Q1 to M6-Q3** (2026-09-15, with clarifications): recorded in [M6](release-0.1/M6.md#owner-decisions-2026-09-15). Implementation was authorized; merging, tagging and publishing were not.
- **Done in M6:** [status](release-0.1/M6.md#status) and [acceptance evidence](release-0.1/M6.md#acceptance-evidence-at-the-candidate):
  - `core.feature_dependencies`, with the documented `execution.claim_revalidation` dependency now enforced;
  - pinned-M5 compatibility checks;
  - the four gap closures;
  - the S-A and S-B third-party compositions;
  - the independent passes 14 and 15;
  - the compatibility sweep, status headers, operation check, inventory and release record.
- **Release authorization (2026-09-16):** close out ([M6 release close-out](release-0.1/M6.md#release-close-out-2026-09-16)), then, if every condition passes, merge PR #7 pinned to the final head, create the annotated `v0.1.0` tag at the verified merge, and publish the source release with checksums, manifest and evidence. Unrelated protocol changes are not authorized.
- **Worktrees kept until the candidate merges:** `.worktrees/independent-m6` (branch `release-0.1/m6-independent`) and `.worktrees/thirdparty-m6` (branch `release-0.1/m6-thirdparty`), both fully merged. Their results are copied under `conformance/results/independent-m6-pass-evidence/` and `conformance/results/thirdparty-m6-evidence/`.
- **Prompt disposition:** no active continuation prompt. The workspace `START-PROTOCOL.md` stays retired and points here.
- **Next action:** complete the release process, then record the release in issue #1 and the post-release handoff.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
