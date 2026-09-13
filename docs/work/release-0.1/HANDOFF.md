# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-13T20:30Z.
- **Status:** M0 and M1 merged (PR #2, `f42d21a`). M2 is complete on branch `release-0.1/m2` pending CI and owner review ([M2 task](M2.md)). Both implementations pass the suite.

## Goal, decisions and constraints

- **Plan:** [PLAN](PLAN.md), [MATRIX](MATRIX.md).
- **Owner decisions (2026-09-13):**
  - Scope accepted.
  - macOS and Linux only (U4).
  - MIT license (U6).
  - Decision records 001–005 accepted.
  - Rust tooling, because PIO, CBR and the control plane are Rust.
  - U12: credential file plus `core.authenticate` ([decision 006](../../decisions/006-unix-socket-principal-credential.md)).
- **Constraints:** Protocol owns contracts and fixtures only. Nothing is released. Commit and push as work progresses (owner request). Do not merge without owner authorization.

## Git state

- **Branches.** `main` at `f42d21a`. `release-0.1/m2` is pushed; its head is in `git log`.
- **M2 PR:** [#3](https://github.com/Combraton/protocol/pull/3), open for owner review.
- **Latest M2 commits (oldest first):**

| Commit | Change |
|---|---|
| `26f8d5b` | Unix-socket binding with principal credentials (decision 006) |
| `668928e` | Positive fixtures for M2 rows that had only negative coverage |
| `6ea7c67` | Draft M3 Execution task packet |
| `561939a`, `341b363` | Independent Python provider extended to grants, events and capabilities; merged |
| `6c64ae4` | Section E resolutions: spec, schema, reference, runner, 22 new and 4 revised fixtures |
| `572cd65` | Merge of the independent third pass (section F of its divergence log) |
| next commit | Section F resolutions: spec, reference, 9 new and 2 revised fixtures, 20 mutants |

## What exists on `release-0.1/m2`

- **Core draft:**
  - §15 grants: denial order, delegation bounds, issue validation at step 6, binding scopes, revocation rules, which operations are protected.
  - §16 events: visibility equal to direct read authority, exact `filtered`, snapshot contents, old-epoch cursors, size limits, `ended` notifications.
  - §17 capabilities.
  - §18 credentials.
- **STREAM:** stdio and Unix-socket forms.
- **Suite:**
  - 153 fixtures; 143 reference mutants.
  - The runner enforces the caller's receive limit on every received frame and notification-after-response ordering.
  - It supports `any_of`, fresh data directories and `$repeat` values.
  - New launch key: `events.unvouched_last`.

## Evidence

Local run on macOS arm64 of the working tree committed as the section F resolution, all exit 0:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo build --workspace --locked`; `cargo test --workspace --locked` | passed |
| `./target/debug/combraton-conformance self-test` | 31 vectors ok |
| `./target/debug/combraton-conformance check-fixtures` | 153 ok |
| `run` and `check-mutants` with `reference-provider.json` | 153 pass; every declared mutant killed |
| `run` and `check-mutants` with `reference-provider-unix.json` | 153 pass; every declared mutant killed |
| `run` with `independent-python-core.json` | 0 not passing (socket fixtures not applicable) |
| `python3 scripts/check_docs.py` | 0 errors |

- **CI:** `6c64ae4` failed only the independent step, as expected. At `eabfd8e`, Conformance run 34777724435 succeeded on Ubuntu and macOS, including the cross-checks, and Documentation run 34777724430 succeeded.
- **Kill reasons.** The kill reasons of 11 new mutants were inspected one by one; each fails its fixture at the intended step.

**Independence notes:**
- The second pass found a disclosure bug in the spec: grant records and subject values leaked through events. It also found that notifications could exceed the caller's receive limit, a bug the reference shared.
- The third pass found that this fix was unguarded against the original leak, along with 20 other unguarded requirements. All now have fixtures and mutants.
- The 9 new fixtures pass on both implementations.

## What remains uncertain

- **Single implementation:** the Unix-socket binding and credentials.
- **Unguarded, documented:**
  - a cursor past the current head (unreachable without constructing a cursor);
  - capability revision on evidence-source change;
  - the issue check order, where only `details.path` differs;
  - rejection of a different peer user on the socket (needs a second OS account).
- **Deferred:** effects, telemetry lost ranges, backpressure, pipelined requests and fault injection (M3); highest-common-major selection (M6).

## Active resources

None. Conformance runs spawn short-lived providers in temporary directories.

## State and prompt disposition

[STATE](../STATE.md) is updated. The workspace continuation prompt points here.

## Next action

1. Owner review of [PR #3](https://github.com/Combraton/protocol/pull/3). Address comments; do not merge without authorization.
2. Detail M3 ([draft](M3.md)) as a task.
