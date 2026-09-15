# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code).
- **Checkpoint:** 2026-09-16 (released).
- **Status:** Protocol 0.1 is released as [v0.1.0](https://github.com/Combraton/protocol/releases/tag/v0.1.0), under the owner's 2026-09-16 authorization. The close-out is in [M6](M6.md#release-close-out-2026-09-16); the release facts are [below](#release-v010).

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
  - Protocol owns contracts and fixtures only. Do not implement the real CBR memory engine or PIO adapters.
  - A released tag or asset is never moved, overwritten or silently replaced. Changes to Protocol 0.1 after v0.1.0 are versioned revisions.
  - Commit and push as work progresses. Do not merge without authorization.
  - Substantive contract changes or unresolved decisions go to the owner; routine work continues.
  - Never create a records-only commit merely to note that the previous commit passed CI; link PR checks.
  - Never describe same-implementation composition as mixed-implementation proof.

## Git state

- **`main`** at `cbf8e4d`, the merge of PR #7 (parents `6ed4727` and `f7e6797`; its tree `ad57cc4` equals the tested head's tree). The handoff PR `release-0.1/v0.1.0-handoff` follows it and changes only session records.
- **Tag** `v0.1.0`: annotated, tag object `71856e599200dde618ecd197d985ff230165b71f`, pointing at `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`.
- **Worktrees:** none.
  - `.worktrees/independent-m6` (tip `fa6b69a`) and `.worktrees/thirdparty-m6` (tip `e6d60c3`) were removed on 2026-09-16, with their local branches. Before removal: no uncommitted changes; both tips are ancestors of `main`; their results matched the copies under `conformance/results/independent-m6-pass-evidence/` (pass-14, pass-15) and `conformance/results/thirdparty-m6-evidence/` (pass-1 to pass-3).
  - Earlier helper worktrees were removed the same way (see the M5 and M4 checkpoints).
- **Local-only branches:** `release-0.1/m2` to `release-0.1/m6` and `release-0.1/m3-independent` are merged; kept, not deleted.

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
- **Merged with PR #7 (M6 and the release close-out):**
  - `core.feature_dependencies` (CMP-5) and feature-triggered dependency enforcement;
  - pinned-M5 compatibility tooling and fixtures;
  - contracts-only third-party clients (S-A kernel with three client mutants, S-B publisher);
  - 280 fixtures;
  - release-candidate status headers and the operation check;
  - `scripts/release_inventory.py` (Git or file-system mode) and `scripts/release.py`;
  - the CI job that verifies the source bundle from a fresh extraction;
  - `docs/release/0.1/` (record, notes, inventory) and [CONSUMERS](CONSUMERS.md).

## Release v0.1.0

- **Release:** https://github.com/Combraton/protocol/releases/tag/v0.1.0 (not a draft, not a pre-release).
- **Tag:** `v0.1.0`, annotated (tag object `71856e599200dde618ecd197d985ff230165b71f`), at commit `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`.
- **Tested head:** `f7e67975dc712bb810d4e14aa0819d980b06dbe3`, the same tree `ad57cc4ef067834c20d868dbcc844f70b8ef223f`.
- **Asset SHA-256:**

  | Asset | SHA-256 |
  |---|---|
  | `combraton-protocol-0.1.0-source.tar.gz` | `bde7892404bcc6fcf45f4cddb95026517e92a6bdf352ce20f959941596a3511d` |
  | `combraton-protocol-0.1.0-BUNDLE-SHA256SUMS` (539 files) | `844850ba83110a7e92d7e8c33c74ff3bdb9b40ee23a497ba38ced81f94e49ea3` |
  | `combraton-protocol-0.1.0-evidence.tar.gz` | `1344e0ba30c6386c3efe3d02b9578dadf5c6e009853a5da4416b4e23c3784f61` |
  | `release-manifest.json` | `af6ce6526c3761f2f75466cf203fb57fcd48c159221462e8f943329655c45358` |
  | `release-notes.md` | `adf7a17fc9e21359a86eb756bb13d959037b27e56f9cd65facf04c696b32b6d9` |
  | `SHA256SUMS` (lists the five above) | `50b5123513c6c2705501f1e984226ec3d139ca23f3710e1e4ccb8fbddf830e12` |

- **Normative inventory:** 420 files, listing SHA-256 `80b39377b10685c29bb5823e69ee539ef91bb1eace5b049f2894b603ce08b41d`.
- **Versions:** runner, reference provider, independent provider and third-party clients 0.1.0; Rust toolchain 1.97.1.
- **CI:** all passed on Ubuntu and macOS.
  - [35012512679](https://github.com/Combraton/protocol/actions/runs/35012512679) (push) and [35012514846](https://github.com/Combraton/protocol/actions/runs/35012514846) (pull request) on the tested head.
  - [35014862971](https://github.com/Combraton/protocol/actions/runs/35014862971) on `main` at the release commit.
  - Results, identical on both OSes:
    - reference stdio 252 pass, 28 skipped; Unix socket 279 pass, 1 skipped;
    - independent 247 pass, 28 skipped, 5 unsupported;
    - `check-mutants` passes on both bindings (398 and 60 kills, including four client mutants at their stated steps);
    - the accepted M5 fixture set, unmodified: 249 pass, 24 skipped over stdio; 273 pass over the socket; independent 245 pass, 24 skipped, 4 unsupported;
    - the pinned M5 provider compat fixture passes on both bindings;
    - peer-user checks pass; race regression 20/20 each way;
    - the source bundle builds byte-identical twice, and the full consumer path passes from a fresh extraction.
  - On `main`, CI built the bundle with the published SHA-256 `bde78924…` on both OSes.
- **Local checks:**
  - The release bundle built locally from `cbf8e4d` is byte-identical to CI's, and passed `scripts/release.py verify-bundle --full` from a fresh extraction.
  - After publication, the downloaded assets passed `shasum -a 256 -c SHA256SUMS`, `BUNDLE-SHA256SUMS` (539 files), `release_inventory.py --verify` (file-system mode), the docs and operations checks, the locked build, `version`, `self-test` and `check-fixtures`, and the reference provider over stdio and the Unix socket.
- **Evidence asset:** the CI artifacts of the two tested-head runs (result manifests, transcripts, mutant summaries, compatibility, composition and bundle verification), with 4,580 run-scoped credential strings redacted. The local verification record was not published, because it contains machine-local paths.
- **Verification commands:** [docs/VERIFICATION.md](../../VERIFICATION.md) and the release notes.
  - **Archive:** `shasum -a 256 -c SHA256SUMS --ignore-missing`; extract; `shasum -a 256 -c BUNDLE-SHA256SUMS`; `python3 scripts/release_inventory.py --verify`; `cargo build --workspace --locked`; the runner commands.
  - **Git:** `git checkout v0.1.0`; `git rev-parse v0.1.0^{commit}` must equal `cbf8e4df9df2ca8a9b50264df6acace6e4c3a0fc`; `python3 scripts/release_inventory.py --verify`; the same runner commands.

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

- None: no worktrees, and no background processes owned by this task.

## State and prompt disposition

- **[STATE](../STATE.md)** is updated.
- **No continuation prompt is active.** The workspace `START-PROTOCOL.md` is retired and points to the v0.1.0 release. The helper briefs lived only in the session scratchpad and are finished.
- **Consumer kickoffs.** The PIO (Codex) and CBR (Claude) kickoff prompts pinned to v0.1.0 were given to the owner. They are not stored here.

## Next action

None authorized in this repository. Future Protocol changes are versioned revisions proposed on issues. PIO and CBR pin v0.1.0 as described in [CONSUMERS](CONSUMERS.md) and the release notes.
