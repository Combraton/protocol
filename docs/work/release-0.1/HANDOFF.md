# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-13T13:10Z.
- **Status:** M0 and M1 merged (PR #2, `f42d21a`). M2 in progress on branch `release-0.1/m2`; see [M2 task](M2.md).

## Goal, decisions and constraints

- **Plan:** [PLAN](PLAN.md), [MATRIX](MATRIX.md).
- **Owner decisions (2026-09-13):**
  - Scope accepted.
  - macOS and Linux only (U4).
  - MIT license (U6).
  - Decision records 001–005 accepted.
  - PIO, CBR and the control plane are Rust, so the conformance tooling is Rust.
- **Open owner decision: U12,** the Unix-socket principal credential form. The recommendation is in [M2 §Unix-socket principal credential](M2.md#unix-socket-principal-credential-u12--recommendation-for-the-owner). The Unix-socket binding and fixtures (TRN-3, CORE-15) wait for it.
- **Constraints:** Protocol owns contracts and fixtures only. Nothing is released. Commit and push as work progresses (owner request).

## Git state

- **Branches.** `main` at `f42d21a`. `release-0.1/m2` is pushed; its head is in `git log`.
- **M2 commits so far (oldest first):**

| Commit | Change |
|---|---|
| `a8c27ac` | M2 task packet; effects moved to M3 |
| `0d61154` | Grants |
| `e96641c` | Events and subscriptions |
| `82c5364` | Capabilities |
| `c91bf7d` | Merge of the independent Python Core implementation |
| `6daa3cb` | CI runs the independent implementation |
| `ed3f494` | Spec resolutions of its findings |
| `3b32037` | 21 new and 2 revised fixtures, 19 mutants, runner and reference fixes |

- **No PR for M2 yet.**
- **Worktree `.worktrees/independent-python-m2`** (git-ignored directory), branch `release-0.1/m2-independent-python-features`. A spec-only helper is extending the independent implementation to grants, events and capabilities. Review and merge its branch when it reports. Do not edit that worktree from the main checkout.

## What exists on `release-0.1/m2`

- **Core draft:**
  - §15 grants (provider-held records, delegation, revocation cascade, epoch binding, expiry, step-6 authorization, non-leakage);
  - §16 events (durable stream, epochs, cursors, gap and epoch-change items, subscriptions, filtering);
  - §17 capabilities (revisioned snapshot, loss refusal, replay preserved, change events);
  - clarified check order, negotiation rules, limit measurement, and launch-configuration-only test control.
- **STREAM:** code-point request IDs, malformed and ID-less object rules, binding error retry classes, exit status.
- **Suite:**
  - 108 fixtures; 78 reference mutants.
  - Runner supports notifications, feature applicability, launch-configuration validation and exit status.
  - `conformance/independent/python-core`: spec-only Python provider plus its divergence log. Its findings are resolved in [M2-DIVERGENCES](M2-DIVERGENCES.md).

## Evidence

Local run on macOS arm64 at `3b32037`, all exit 0:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo build --workspace --locked`; `cargo test --workspace --locked` | passed |
| `./target/debug/combraton-conformance self-test` | vectors ok |
| `./target/debug/combraton-conformance check-fixtures` | 108 ok |
| `./target/debug/combraton-conformance run --participant conformance/participants/reference-provider.json` | 108 pass |
| `./target/debug/combraton-conformance check-mutants --participant conformance/participants/reference-provider.json` | all 78 mutants killed by every declaring fixture; the reasons were inspected |
| `./target/debug/combraton-conformance run --participant conformance/participants/independent-python-core.json --out conformance/results/independent-python-core` | 108, 0 not passing (M2 feature fixtures not applicable until the helper's extension merges) |
| `python3 scripts/check_docs.py --workspace ..` | clean |

- **CI:** Conformance run 34772340465 for `3b32037` was in progress at this checkpoint; confirm its result before relying on it. Earlier M2 heads were not individually watched.
- **Independence notes:**
  - The first independent implementation passed all 57 M1 fixtures, yet 19 deliberate violations also passed. This is recorded, and the new fixtures now kill equivalent reference mutants.
  - Four new fixtures were wrong when first written. Both implementations failed them identically, and the fixtures were corrected.

## What remains uncertain

- **U12** (owner). Until decided, the Unix-socket binding, CORE-15 and TRN-3 stay open.
- **Single-implementation features:** grants, events and capabilities have only the reference implementation until the helper's work merges.
- **Deferred:** effects, telemetry lost ranges and backpressure (M3); pipelined-request fixtures (M3); highest-common-major selection (M6); `unavailable` and `internal_error` fault injection (M3).

## Active resources

- One background helper in `.worktrees/independent-python-m2`.
- No servers or daemons. Conformance runs spawn short-lived providers in temporary directories.

## State and prompt disposition

[STATE](../STATE.md) is updated. The workspace continuation prompt points here.

## Next action

1. Confirm CI for the branch head.
2. Review and merge the helper's branch; resolve its findings like [M2-DIVERGENCES](M2-DIVERGENCES.md).
3. Obtain U12 and implement the Unix-socket binding with its fixtures.
4. Open the M2 PR for owner review.
