# Current session state — protocol

This is a dated navigation snapshot. Reconcile it with Git, linked issues and current task evidence before acting. Issues own live progress; this file does not grant authority or maintain a second backlog.

- **Updated:** 2026-09-16. **Protocol 0.1 is released as [v0.1.0](https://github.com/Combraton/protocol/releases/tag/v0.1.0).**
- **Owner/last task:** Protocol session (Claude Code) on the Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1). Release details and evidence: [release handoff](release-0.1/HANDOFF.md#release-v010).
- **Release:**
  - **Tag:** annotated `v0.1.0` (tag object `71856e599200dde618ecd197d985ff230165b71f`) at commit `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`.
  - **Commit:** the merge of PR #7. Its tree `ad57cc4ef067834c20d868dbcc844f70b8ef223f` equals the tested head `f7e67975dc712bb810d4e14aa0819d980b06dbe3`.
  - **Assets and records:** source bundle, full-bundle checksums, CI evidence, `release-manifest.json`, `SHA256SUMS` and the release notes; see the handoff.
  - **Normative inventory:** 420 files, listing SHA-256 `80b39377b10685c29bb5823e69ee539ef91bb1eace5b049f2894b603ce08b41d`.
- **Merged:**
  - PR #2 (M0 and M1) as `f42d21a`.
  - PR #3 (M2) as `7bd5cb9`.
  - PR #4 (M3) as `481cf7f`.
  - PR #5 (M4) as `ee82afb`.
  - PR #6 (M5) as `6ed4727`.
  - PR #7 (M6 and the release close-out) as `cbf8e4d`, pinned to `f7e6797`.
  - This handoff PR records the release and changes no released file.
- **Owner decisions recorded:**
  - scope; macOS and Linux; MIT; decision records 001–007;
  - M2–M5 accepted;
  - M6-Q1..Q3;
  - the 2026-09-16 authorization for the close-out and the v0.1.0 release process.
- **Worktrees:** none. The M6 helper worktrees were removed after verifying they were merged and their evidence copied under `conformance/results/independent-m6-pass-evidence/` and `conformance/results/thirdparty-m6-evidence/` (local, gitignored). CI evidence is attached to the release.
- **Prompt disposition:** no active continuation prompt. The workspace `START-PROTOCOL.md` is retired and points to the release. PIO and CBR kickoffs are given to the owner, pinned to v0.1.0.
- **Next action:** none authorized here. Protocol 0.1 changes after the release need a versioned revision (new feature or major) proposed on this repository. PIO and CBR begin their standalone releases against v0.1.0.

At the next meaningful checkpoint, replace stale observations with verified current state. Record exact test commands, exit status, evidence and remaining limitations for the work performed.
