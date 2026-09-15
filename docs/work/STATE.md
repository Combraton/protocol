# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-15 (M5 steps 1–5 done and presented for owner acceptance on draft PR #6; not merged).
- **Owner/current task:** Protocol session (Claude Code, Opus 5) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
  - Current milestone: [M5 task](release-0.1/M5.md), the Knowledge and Verification profiles.
  - Detailed state and evidence: [release handoff](release-0.1/HANDOFF.md).
- **Merged:**
  - PR #2 (M0 and M1) as `f42d21a`.
  - PR #3 (M2) as `7bd5cb9`, after the owner accepted M2.
  - PR #4 (M3) as `481cf7f`, after the owner's conditional acceptance with corrections C1–C3 on 2026-09-14. [PR #4 checks](https://github.com/Combraton/protocol/pull/4/checks) passed on its final head `19e0e16`.
  - PR #5 (M4) as `ee82afb` on 2026-09-15:
    - The owner accepted M4 at `86e128f` as a completed milestone with its documented coverage limits.
    - [PR #5 checks](https://github.com/Combraton/protocol/pull/5/checks) passed on that head.
    - The merge commit's tree equals `86e128f`.

  M2, M3 and M4 are accepted milestones; Protocol 0.1 is not accepted or released.
- **Branch:** `release-0.1/m5` from `ee82afb`, in draft [PR #6](https://github.com/Combraton/protocol/pull/6); not merged.
- **Done in M5** ([status](release-0.1/M5.md#status)):
  - **Step 1:** contracts, examples and matrix, with owner decisions M5-Q1 to M5-Q9 incorporated.
  - **Steps 2–4:** reference Knowledge and Verification providers; claims in packets and claim revalidation.
  - **Owner follow-up:** exact provider-qualified dependency identity.
  - **Step 5:** three spec-only independent passes (eleventh, twelfth and thirteenth), each divergence resolved against the written contracts ([M5-DIVERGENCES](release-0.1/M5-DIVERGENCES.md)).
- **Evidence at M4 acceptance:** CI at `86e128f` on Ubuntu and macOS ([run 34959862568](https://github.com/Combraton/protocol/actions/runs/34959862568)):
  - reference over stdio: 233 pass, 23 skipped; Unix socket: 256 pass;
  - independent: 229 pass, 4 unsupported, 23 skipped, 0 fail;
  - mutant results: 321 of 321 and 51 of 51 as intended;
  - race regression: 20/20 each way.
- **Owner decisions recorded:**
  - scope accepted; macOS and Linux only; MIT; Rust tooling;
  - decision records 001–007 accepted (007 with refinements);
  - M2 accepted; M3 conditionally accepted (C1–C3); M4-Q1 to M4-Q7; the M4 close-out items; M4 accepted with its coverage limits on 2026-09-15;
  - M5: Knowledge and Verification per the release plan, reusing Core, Execution, Evidence and Context, with any compatibility change identified explicitly; reference participants only, with no real CBR memory engine or PIO adapters; M4 gaps carried forward with impact and disposition, not automatically blockers.
- **Owner decisions M5-Q1 to M5-Q9** (2026-09-15): recorded in [M5](release-0.1/M5.md#owner-decisions-2026-09-15). Implementation is authorized within M5 scope; merging M5 and releasing Protocol 0.1 are not.
- **Open owner decisions:** acceptance of M5, presented on [PR #6](https://github.com/Combraton/protocol/pull/6) with verified results and limitations. Machine-readable feature-dependency advertising stays an M6 decision (CMP-5).
- **Prompt disposition:** no active continuation prompt. The workspace `START-PROTOCOL.md` is retired and points here.
- **Evidence at the M5 candidate head:** local results in the [release handoff](release-0.1/HANDOFF.md#evidence); CI on Ubuntu and macOS in [PR #6 checks](https://github.com/Combraton/protocol/pull/6/checks).
- **Next action:** wait for the owner's decision on M5. Do not merge M5 or start M6 before acceptance.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
