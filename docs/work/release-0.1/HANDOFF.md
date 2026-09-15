# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code).
- **Checkpoint:** 2026-09-15 (later).
  - The owner accepted M5 at `00b3c5a` as a completed milestone with its documented limitations; this was explicitly not acceptance or release of Protocol 0.1. PR #6 merged as `6ed4727` (pinned to that head; merge tree equals the accepted head's tree).
  - M6 began on `release-0.1/m6` (draft PR #7) with the release-readiness plan and acceptance matrix ([M6](M6.md)) and the consumer-handoff draft ([CONSUMERS](CONSUMERS.md)).
- **Status:** M0–M5 merged and accepted as milestones. M6 awaits owner decisions M6-Q1 to M6-Q3. Protocol 0.1 is not accepted, tagged or released; doing so requires the owner's explicit authorization.

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

- **`main`** at `6ed4727`, the merge of PR #6 (its tree equals `00b3c5a`, the accepted M5 head).
- **Branch** `release-0.1/m6` from `6ed4727`, in draft [PR #7](https://github.com/Combraton/protocol/pull/7), pushed.
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
- **On `release-0.1/m6`:** the [M6 plan and acceptance matrix](M6.md) (CMP-5 proposal, mixed-implementation topology S-A/S-B/S-C, carried-gap triage) and the [consumer handoff draft](CONSUMERS.md). No implementation yet.

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
- **Earlier milestones:** [M3 status](M3.md#status) and [M3-DIVERGENCES](M3-DIVERGENCES.md); M2 evidence in [M2](M2.md).

## What remains uncertain

- **M6 decisions:** M6-Q1 (CMP-5), M6-Q2 (mixed topology) and M6-Q3 (gap dispositions) are with the owner ([M6](M6.md#owner-decisions-needed)); any genuinely new architectural or authority question goes back with a recommendation.
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

- No helper worktrees; see Git state for preserved evidence paths.
- No background processes.

## State and prompt disposition

[STATE](../STATE.md) is updated. No continuation prompt is active: the workspace `START-PROTOCOL.md` stays retired and points to STATE and this handoff. The M5 helper briefs lived in the session scratchpad only and are finished.

## Next action

1. Owner decisions M6-Q1 to M6-Q3 on [M6](M6.md#owner-decisions-needed).
2. Then M6 step 2: the four required gap closures (fixtures and mutants), followed by CMP-5 implementation per the decision and the third-party examples. Routine work continues autonomously; do not tag or publish Protocol 0.1 without the owner's authorization.
