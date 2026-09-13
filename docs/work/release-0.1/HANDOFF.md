# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-13T18:50Z.
- **Status:** M0 and M1 merged (PR #2, `f42d21a`). M2 is feature-complete for the reference provider on branch `release-0.1/m2` ([M2 task](M2.md)). The independent implementation is behind the latest resolutions.

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
- **No PR for M2 yet.**
- **Latest M2 commits (oldest first):**

| Commit | Change |
|---|---|
| `26f8d5b` | Unix-socket binding with principal credentials (decision 006) |
| `668928e` | Positive fixtures for M2 rows that had only negative coverage |
| `6ea7c67` | Draft M3 Execution task packet |
| `561939a`, `341b363` | Independent Python provider extended to grants, events and capabilities; merged |
| next commit | Section E resolutions: spec, schema, reference, runner, 22 new and 4 revised fixtures, 40 mutants |

## What exists on `release-0.1/m2`

- **Core draft:**
  - §15 grants: denial order, delegation bounds, issue validation at step 6, binding scopes, revocation rules, which operations are protected.
  - §16 events: visibility equal to direct read authority, exact `filtered`, snapshot contents, old-epoch cursors, size limits, `ended` notifications.
  - §17 capabilities.
  - §18 credentials.
- **STREAM:** stdio and Unix-socket forms.
- **Suite:**
  - 144 fixtures; 123 reference mutants.
  - The runner enforces the caller's receive limit on every received frame and notification-after-response ordering.
  - It supports `any_of`, fresh data directories and `$repeat` values.
  - New launch key: `events.unvouched_last`.

## Evidence

Local run on macOS arm64 of the working tree committed as the section E resolution:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo build --workspace --locked`; `cargo test --workspace --locked` | passed |
| `./target/debug/combraton-conformance self-test` | 31 vectors ok |
| `./target/debug/combraton-conformance check-fixtures` | 144 ok |
| `run` and `check-mutants` with `reference-provider.json` | 144 pass; every declared mutant killed |
| `run` and `check-mutants` with `reference-provider-unix.json` | 144 pass; every declared mutant killed |
| `run` with `independent-python-core.json` | exit 1; 8 not passing, all decisions made after its pass (M2-DIVERGENCES, "Independent implementation status") |
| `python3 scripts/check_docs.py` | 0 errors |

- **CI:** runs up to `6ea7c67` succeeded. The pushed section E head is expected to fail only the independent step.

**Independence notes:**
- The second pass found a disclosure bug in the spec: grant records and subject values leaked through events.
- It also found that notifications could exceed the caller's receive limit, a bug the reference shared.
- One new mutant (`denial-order-scope-first`) survived the first fixture draft, and one proposed mutant proved unobservable because the schema already enforced its rule. Both were corrected before commit.

## What remains uncertain

- **Single implementation:** the section E decisions have only the reference implementation until the independent implementation catches up.
- **Unguarded, documented:**
  - a cursor past the current head (unreachable without constructing a cursor);
  - capability revision on evidence-source change;
  - a gap spanning epochs;
  - rejection of a different peer user on the socket (needs a second OS account).
- **Deferred:** effects, telemetry lost ranges, backpressure, pipelined requests and fault injection (M3); highest-common-major selection (M6).

## Active resources

None. Conformance runs spawn short-lived providers in temporary directories.

## State and prompt disposition

[STATE](../STATE.md) is updated. The workspace continuation prompt points here.

## Next action

1. Run a spec-only pass that brings the independent implementation level with section E. Review and merge it.
2. Re-enable the stdio `already_authenticated` fixture once the independent implementation implements it.
3. Confirm CI is green.
4. Open the M2 PR for owner review.
