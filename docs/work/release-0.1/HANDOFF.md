# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-15.
  - The owner accepted M4 at `86e128f` as a completed milestone with its documented coverage limits. PR #5 merged as `ee82afb`.
  - M5 began on `release-0.1/m5`. Step 1 is done: the owner decided M5-Q1 to M5-Q9, and the decisions are incorporated. Implementation is authorized within M5 scope, but not merging or releasing.
  - Steps 2–5 are done ([M5 status](M5.md#status)): the Knowledge and Verification reference providers, claims in packets and claim revalidation, the owner's dependency-identity follow-up, and three spec-only independent passes with every divergence resolved ([M5-DIVERGENCES](M5-DIVERGENCES.md)).
- **Status:** M0–M4 merged. M5 is presented for owner acceptance on draft [PR #6](https://github.com/Combraton/protocol/pull/6) and is not merged. Protocol 0.1 is not accepted or released.

## Goal, decisions and constraints

- **Plan:** [PLAN](PLAN.md), [MATRIX](MATRIX.md).
- **Owner decisions:**
  - Scope accepted; macOS and Linux only (U4); MIT (U6); Rust tooling; U12 credential file plus `core.authenticate`; U2 verification depth is receipts plus `evaluate_contract` job references, with no signing or evaluator orchestration.
  - Decision records 001–007 accepted; 001 amended for distinct outcomes; 007 accepted with refinements.
  - M2 accepted on 2026-09-14. M3 conditionally accepted on 2026-09-14 with corrections C1–C3 ([M3 status](M3.md#status)).
  - M4-Q1 to M4-Q7 and the close-out items ([M4](M4.md)); M4 accepted on 2026-09-15 with its coverage limits.
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

- **`main`** at `ee82afb`, the merge of PR #5 (parents `481cf7f` and `86e128f`; its tree equals `86e128f`).
- **Branch** `release-0.1/m5` from `ee82afb`, in draft [PR #6](https://github.com/Combraton/protocol/pull/6), pushed; the candidate head is the commit carrying this handoff.
- **Worktree** `.worktrees/independent-m5` (branch `release-0.1/m5-independent`, local only, at `53ec6bf`). It is fully merged into `release-0.1/m5` (merges `87f84ec`, `808801b` and `4458bfa`) and has no uncommitted changes. Its results are copied to `conformance/results/independent-m5-pass-evidence/`. It is kept until M5 merges and the merge is verified.
- **M4 worktree:**
  - `.worktrees/independent-m4` was removed on 2026-09-15 after checks: it had no uncommitted changes, `release-0.1/m4-independent` (`6ec6712`) had no commits outside `main`, and its results directory matched the preserved copy. Only regenerable `target/` and `__pycache__/` were discarded.
  - The branch was deleted.
- **Local-only branches:** `release-0.1/m2`, `release-0.1/m3`, `release-0.1/m3-independent` and `release-0.1/m4` are merged; kept, not deleted.

## What exists

- **Merged on `main`:**
  - Core §1–§19, STREAM (stdio and Unix socket), ENCODING;
  - Execution with its optional features, context revalidation and evidence outputs;
  - Evidence and Context;
  - 256 fixtures, the reference provider and its mutants, the multi-participant runner;
  - the independent Python provider (Core, effects, Execution, Evidence, Context over stdio);
  - CI with distinct outcomes and uploaded result artifacts.
- **On `release-0.1/m5`:**
  - **Specs:** KNOWLEDGE and VERIFICATION; CONTEXT §14 (`context.claims`); EXECUTION §13.3 (`execution.claim_revalidation`); CORE §12 codes `claims_not_comparable` and `contract_unavailable`.
  - **Schemas:** `schemas/knowledge/1` (11 operations) and `schemas/verification/1` (5 operations); the Context and launch-configuration additions.
  - **Reference:** the knowledge and verification providers, claims in packets, claim revalidation, and their mutants.
  - **Fixtures:** 17 M5 fixtures (10 `knowledge.*`, 5 `verification.*`, the claims fixture, and the claims composition), for 273 in total.
  - **Runner:** patterns `$canonical_sha256` and `$base64_json`; the mixed-directive lint; `start` configuration variables, validated after substitution.
  - **Independent Python provider:** `knowledge/1`, `verification/1` and single-provider `context.claims`.
  - **Records:** [M5](M5.md), [M5-DIVERGENCES](M5-DIVERGENCES.md), and the MATRIX rows KNW-1 to KNW-10, VER-1 to VER-5, SCN-7, SCN-16 and CMP-9.

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
- **Helper evidence** (gitignored, local): `conformance/results/independent-m5-pass-evidence/pass-11`, `pass-12` and `pass-13`.
- **Earlier milestones:** [M3 status](M3.md#status) and [M3-DIVERGENCES](M3-DIVERGENCES.md); M2 evidence in [M2](M2.md).

## What remains uncertain

- **M5 decisions:** decided on 2026-09-15 ([M5](M5.md#owner-decisions-2026-09-15)); any genuinely new architectural or authority question goes back to the owner with a recommendation.
- **M4 limits carried forward:** [M5 §M4 coverage carried forward](M5.md#m4-coverage-carried-forward), with impact and disposition for each.
- **M5 limits:**
  - checks still unguarded by fixtures are listed in [M5-DIVERGENCES §E](M5-DIVERGENCES.md#e-still-unchecked-by-fixtures-coverage-limits);
  - the claims composition runs one implementation for every participant, so it is not mixed-implementation proof;
  - the independent provider has no Unix-socket binding, so SCN-16 and the Execution side of CMP-9 are checked only on the reference.
- **Profile status headers:** EXECUTION, EVIDENCE, CONTEXT, KNOWLEDGE and VERIFICATION still carry their "proposed draft" headers from before acceptance, as EXECUTION did after M3. Refreshing them belongs with M6's complete operation documentation.
- **Carried from M3:**
  - the independent provider has no Unix socket or backpressure support;
  - barrier- and signal-synchronized fixtures are coverage limits for other participants;
  - all Execution evidence is scripted, not real-adapter evidence.
- **Deferred decision:** machine-readable feature-dependency advertising is an M6 decision (CMP-5).

## Active resources

- Helper worktree `.worktrees/independent-m5`, kept until M5 merges; see Git state.
- No background processes.

## State and prompt disposition

[STATE](../STATE.md) is updated. No continuation prompt is active: the workspace `START-PROTOCOL.md` stays retired and points to STATE and this handoff. The M5 helper briefs lived in the session scratchpad only and are finished.

## Next action

1. Wait for the owner's decision on M5. Do not merge M5 or start M6 before acceptance.
2. After acceptance and an authorized merge: verify that the merge tree equals the accepted head, update STATE, this handoff and issue #1, then remove the helper worktree and branch once integration and evidence are verified.
