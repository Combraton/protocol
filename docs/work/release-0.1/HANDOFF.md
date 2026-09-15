# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code).
- **Checkpoint:** 2026-09-15 (release candidate).
  - The owner accepted M5 at `00b3c5a`; PR #6 merged as `6ed4727`.
  - The owner approved M6-Q1 to M6-Q3 with clarifications. The bounded M6 work is done ([M6 status](M6.md#status)), and the release candidate is presented on draft PR #7.
- **Status:** M0–M5 merged and accepted as milestones. On 2026-09-16 the owner authorized the close-out and the v0.1.0 release process, conditional on the checks. The close-out is in [M6](M6.md#release-close-out-2026-09-16). This commit is the release-preparation head; the tag and release details follow in issue #1.

## Goal, decisions and constraints

- **Plan:** [PLAN](PLAN.md), [MATRIX](MATRIX.md).
- **Owner decisions:**
  - Scope accepted; macOS and Linux only (U4); MIT (U6); Rust tooling; U12 credential file plus `core.authenticate`; U2 verification depth is receipts plus `evaluate_contract` job references, with no signing or evaluator orchestration.
  - Decision records 001–007 accepted; 001 amended for distinct outcomes; 007 accepted with refinements.
  - M2 accepted on 2026-09-14. M3 conditionally accepted on 2026-09-14 with corrections C1–C3 ([M3 status](M3.md#status)).
  - M4-Q1 to M4-Q7 and the close-out items ([M4](M4.md)); M4 accepted on 2026-09-15 with its coverage limits.
  - M6-Q1 to M6-Q3 (2026-09-15): [M6 decisions](M6.md#owner-decisions-2026-09-15). CMP-5 option A with explicit profile and feature triggers; S-A and S-B with the clarified roles (S-C optional, not done); gap dispositions, with new material defects needing resolution; the consumer handoff split into serves and calls.
  - M5 (2026-09-15):
    - Knowledge and Verification per the release plan, including packets carrying claims and a provider reconnecting with a changed capability;
    - contracts, matrix and proposed decisions, with concrete examples, before implementing new public semantics;
    - reuse Core, Execution, Evidence and Context, and identify compatibility changes explicitly;
    - reference participants, independent implementations and conformance tests only;
    - carry M4 gaps forward with impact and disposition, without making each one a blocker;
    - bring new architectural or authority decisions with a recommendation and alternatives.
- **Constraints:**
  - Protocol owns contracts and fixtures only. Nothing is released. Do not implement the real CBR memory engine or PIO adapters.
  - Commit and push as work progresses. Do not merge without authorization.
  - Substantive contract changes or unresolved decisions go to the owner; routine work continues.
  - Never create a records-only commit merely to note that the previous commit passed CI; link PR checks.
  - Never describe same-implementation composition as mixed-implementation proof.

## Git state

- **`main`** at `6ed4727`, the merge of PR #6 (its tree equals `00b3c5a`, the accepted M5 head).
- **Branch** `release-0.1/m6` from `6ed4727`, in draft [PR #7](https://github.com/Combraton/protocol/pull/7), pushed. The candidate head is the commit carrying this handoff.
- **M6 worktrees, kept until the candidate merges:**
  - `.worktrees/independent-m6` (`release-0.1/m6-independent`, tip `fa6b69a`, merged as `06526dc`);
  - `.worktrees/thirdparty-m6` (`release-0.1/m6-thirdparty`, tip `e4e58ac`, merged as `3120e45`).

  Neither has uncommitted changes. Their results are copied under `conformance/results/independent-m6-pass-evidence/` (pass-14, pass-15) and `conformance/results/thirdparty-m6-evidence/` (pass-1, pass-2).
- **M5 worktree:** `.worktrees/independent-m5` and branch `release-0.1/m5-independent` were removed on 2026-09-15 after checks: no uncommitted changes, tip `53ec6bf` is an ancestor of the merge (via merges `87f84ec`, `808801b`, `4458bfa`), and the run results are preserved under `conformance/results/independent-m5-pass-evidence/` (pass-11, pass-12, pass-13). Only regenerable build artifacts were discarded.
- **M4 worktree:** removed earlier on 2026-09-15 (see prior checkpoint); evidence under `conformance/results/independent-m4-pass-evidence/`.
- **Local-only branches:** `release-0.1/m2`, `release-0.1/m3`, `release-0.1/m3-independent`, `release-0.1/m4` and `release-0.1/m5` are merged; kept, not deleted.

## What exists

- **Merged on `main`:**
  - Core §1–§19, STREAM (stdio and Unix socket), ENCODING;
  - Execution with its optional features, context revalidation, claim revalidation and evidence outputs;
  - Evidence and Context (including `context.claims`, packet format `/2`);
  - Knowledge and Verification (`schemas/knowledge/1`, `schemas/verification/1`), claims in packets and the changed-capability reconnect;
  - 273 fixtures, the reference providers and their mutants, the multi-participant runner (`$canonical_sha256`, `$base64_json`, the mixed-directive lint, `start` configuration variables);
  - the independent Python provider (Core, effects, Execution, Evidence, Context, Knowledge, Verification and single-provider `context.claims`, over stdio);
  - CI with distinct outcomes and uploaded result artifacts;
  - records: [M5](M5.md), [M5-DIVERGENCES](M5-DIVERGENCES.md), MATRIX rows KNW-1..10, VER-1..5, SCN-7, SCN-16, CMP-9.
- **On `release-0.1/m6`:**
  - **CMP-5:** CORE §4.3 `core.feature_dependencies` and the feature-triggered rule in CORE §4.2, with schemas, reference and independent implementations.
  - **Compatibility tooling:** runner support for pinned participants and fixture trees (`--fixtures`, `requires_pinned`) and `scripts/build_pinned.py`.
  - **Third-party consumers:** the interface and two contracts-only clients (`conformance/thirdparty/`); runner client steps and client mutants.
  - **Fixtures:** 280 in total. M6 adds `compat.*`, the feature-dependency fixtures, the major-mismatch fixture, S-A and S-B, and new steps in the authority, reconnect, assessment and claims-composition fixtures.
  - **Documentation and package:** release-candidate status headers; `scripts/check_operations.py`; `scripts/release_inventory.py` with `docs/release/0.1/`; [CONSUMERS](CONSUMERS.md) finalized.

## Evidence

- **M4 acceptance head `86e128f`:** CI green on Ubuntu and macOS ([run 34959862568](https://github.com/Combraton/protocol/actions/runs/34959862568) and the push run). The uploaded artifacts are identical on both:
  - reference stdio 233 pass, 23 skipped; Unix socket 256 pass;
  - independent 229 pass, 4 unsupported, 23 skipped, 0 fail;
  - mutants 321 of 321 and 51 of 51 as intended;
  - race regression 20/20 each way.

  Local results and the close-out history are in [M4 status](M4.md#status) and [M4-DIVERGENCES](M4-DIVERGENCES.md).
- **M4 merge:** `gh pr merge 5 --merge --match-head-commit 86e128f…` produced `ee82afb`; `git diff 86e128f ee82afb` is empty. CI for `main` at `ee82afb` runs in the repository's [Actions](https://github.com/Combraton/protocol/actions?query=branch%3Amain).
- **Independent-implementation evidence** (gitignored, local): `conformance/results/independent-m4-pass-evidence/`, including `worktree-results/`, a copy of the helper worktree's run results for passes six to ten. M3 helper evidence stays under `conformance/results/independent-m3-pass-evidence/` and `conformance/results/independent-c1-evidence/`.
- **M5 step 1:** documentation only; `python3 scripts/check_docs.py` passes.
- **M5 candidate** (local, macOS; reference code at `6dccc56`, independent code at `4458bfa`, no code change since):
  - `cargo fmt --check`, `clippy -D warnings` and `cargo test` pass;
  - `check-fixtures`: 273 fixtures and 144 matrix requirement IDs ok; `check_docs.py` passes;
  - reference over stdio: 249 pass, 24 skipped; over the Unix socket: 273 pass;
  - independent: 245 pass, 24 skipped, 4 unsupported, 0 fail;
  - `check-mutants` passes on both bindings: every declared mutant is killed by every fixture that declares it.

  CI on Ubuntu and macOS for the candidate head: [PR #6 checks](https://github.com/Combraton/protocol/pull/6/checks).
- **M5 merge:** PR #6 head verified equal to the accepted `00b3c5a` with checks green, then `gh pr merge 6 --merge --match-head-commit 00b3c5a…` produced `6ed4727`; `git rev-parse` shows equal trees for the merge and the accepted head. CI for `main` at `6ed4727` runs in [Actions](https://github.com/Combraton/protocol/actions?query=branch%3Amain).
- **Helper evidence** (gitignored, local): `conformance/results/independent-m5-pass-evidence/pass-11`, `pass-12` and `pass-13`.
- **M6 candidate** (local, macOS, at code `3120e45`; the records commit changes only documents and the inventory):
  - fmt, clippy `-D warnings`, build and tests pass; self-test 31 vectors; `check-fixtures` 280 fixtures ok; `check_docs.py`, `check_operations.py` (59 operations) and `release_inventory.py --verify` pass;
  - reference over stdio: 252 pass, 28 skipped; over the Unix socket: 279 pass, 1 skipped (pinned-only);
  - independent: 247 pass, 28 skipped, 5 unsupported;
  - `check-mutants` passes on both bindings (398 and 59 kills, including the three client mutants);
  - accepted M5 fixture set, unmodified: reference stdio 249 pass, 24 skipped; socket 273 pass; independent 245 pass, 24 skipped, 4 unsupported;
  - pinned M5 provider: the compat fixture passes on both bindings;
  - S-A, S-B and the claims composition: 5 of 5 repeated runs each.

  CI on Ubuntu and macOS: [PR #7 checks](https://github.com/Combraton/protocol/pull/7/checks).
- **Earlier milestones:** [M3 status](M3.md#status) and [M3-DIVERGENCES](M3-DIVERGENCES.md); M2 evidence in [M2](M2.md).

## What remains uncertain

- **Release decision:** acceptance of the candidate, then authorization to merge, tag and publish, are with the owner.
- **M6 limits:** [M6 coverage limits](M6.md#m6-coverage-limits) and the release record's accepted limitations and deferred gaps.
- **M4 limits carried forward:** [M5 §M4 coverage carried forward](M5.md#m4-coverage-carried-forward), with impact and disposition for each.
- **M5 limits:**
  - checks still unguarded by fixtures are listed in [M5-DIVERGENCES §E](M5-DIVERGENCES.md#e-still-unchecked-by-fixtures-coverage-limits);
  - the claims composition runs one implementation for every participant, so it is not mixed-implementation proof;
  - the independent provider has no Unix-socket binding, so SCN-16 and the Execution side of CMP-9 are checked only on the reference.
- **Carried from M3:**
  - the independent provider has no Unix socket or backpressure support;
  - barrier- and signal-synchronized fixtures are coverage limits for other participants;
  - all Execution evidence is scripted, not real-adapter evidence.
- **CMP-5:** decided (M6-Q1) and implemented.

## Active resources

- Helper worktrees `.worktrees/independent-m6` and `.worktrees/thirdparty-m6`, kept until the candidate merges (see Git state).
- No background processes.

## State and prompt disposition

[STATE](../STATE.md) is updated. No continuation prompt is active: the workspace `START-PROTOCOL.md` stays retired and points to STATE and this handoff. The M5 helper briefs lived in the session scratchpad only and are finished.

## Next action

1. Complete the authorized release process (M6 release close-out).
2. After the merge:
   - verify that the merge tree equals the accepted head;
   - create the tag only when authorized, and record the commit and tag in issue #1 and the release record's acceptance note;
   - update STATE, this handoff and CONSUMERS;
   - remove the helper worktrees and branches once integration and evidence are verified.
