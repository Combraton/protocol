# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-14, after the owner-requested M2 close-out pass.
- **Status:** M0 and M1 merged (PR #2, `f42d21a`). M2 is complete on branch `release-0.1/m2`, including the close-out pass ([M2 task](M2.md)). It awaits owner acceptance in PR #3. Both implementations pass the suite.

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
| `eabfd8e` | Section F resolutions: spec, reference, 9 new and 2 revised fixtures, 20 mutants |
| `9571ed9` | PR #3 recorded |
| `8530c26` | Close-out pass: idle revocation race fix, capability revision fix, issue check order, reference-only cursor test, peer-user check, distinct outcomes, CI artifacts |
| next commit | Close-out records: M2 close-out table, M3 acceptance criteria, state, handoff |

## What exists on `release-0.1/m2`

- **Core draft:**
  - §15 grants: denial order, delegation bounds, issue validation at step 6, binding scopes, revocation rules, which operations are protected.
  - §16 events: visibility equal to direct read authority, exact `filtered`, snapshot contents, old-epoch cursors, size limits, `ended` notifications.
  - §17 capabilities.
  - §18 credentials.
- **STREAM:** stdio and Unix-socket forms.
- **Suite:**
  - 155 fixtures; 147 reference mutants.
  - The runner enforces the caller's receive limit on every received frame and notification-after-response ordering.
  - It supports `any_of`, fresh data directories and `$repeat` values.
  - New launch key: `events.unvouched_last`.

## Evidence

**CI, Conformance run 34821316336 at `8530c26`.** Success on Ubuntu and macOS, plus the cross-check job. Its artifacts `conformance-results-ubuntu-latest` and `conformance-results-macos-latest` were downloaded and inspected. On both OSes:

| Result | Outcome |
|---|---|
| Reference, stdio | 148 pass, 7 skipped (socket fixtures) |
| Reference, Unix socket | 155 pass |
| Independent Python, stdio | 148 pass, 7 skipped |
| `check-mutants`, stdio and socket | all declared mutants killed (`mutants.json`) |
| Peer-user check | `pass`: root client (uid 0) closed with 0 bytes; same-user control answered |
| Peer-user check with `skip-peer-check` | `pass`: root client answered, so the check detects the missing rule |

**Local (macOS arm64), all exit 0 except as noted:**
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build --workspace --locked`, `cargo test --workspace --locked`: the latter includes the capability-revision unit test, which fails against the old digest, and the reference cursor test;
- `self-test`, `check-fixtures` (155);
- `run` and `check-mutants` for both reference participants, and `run` for the independent participant;
- `python3 scripts/check_docs.py`;
- `peer_user_check.py`: exit 77 `unsupported`, because this host has no passwordless `sudo`.

**Independence notes:**
- The second pass found a disclosure bug and a receive-limit bug.
- The third found 21 unguarded requirements, all now guarded.
- The close-out pass found two real reference bugs, both fixed: the idle re-check race and the capability digest.

## What remains uncertain

- **Concurrency.** Ordering under concurrent revocation rests on the processing lock and code review. The race cannot be forced deterministically.
- **Idle grant expiry during a session.** No portable fixture observes it, because the launch clock is fixed. A controllable test clock is an M3 acceptance criterion.
- **Peer-user check.** It exercises only root as the other user, and hosts without passwordless `sudo` report `unsupported`.
- **Reference-only evidence.** Capability revision on an evidence-source change, and cursors past the head, are tested only by reference tests. Portable fixtures cannot force them.
- **Single implementation:** the Unix-socket binding and credentials.
- **Deferred to M3 acceptance:**
  - effects and reconciliation;
  - telemetry lost ranges;
  - backpressure;
  - fault injection;
  - pipelined requests.

  Highest-common-major selection is deferred to M6.

## Active resources

None. Conformance runs spawn short-lived providers in temporary directories.

## State and prompt disposition

[STATE](../STATE.md) is updated. The workspace continuation prompt points here.

## Next action

1. Owner acceptance of [PR #3](https://github.com/Combraton/protocol/pull/3). Do not merge without authorization.
2. After acceptance, start M3 in a separate PR per [M3](M3.md), with its explicit acceptance criteria.
