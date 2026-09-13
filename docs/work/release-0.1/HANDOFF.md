# Handoff — Protocol 0.1 release

A dated observation, not permission to replay actions. Reconcile with Git, [issue #1](https://github.com/Combraton/protocol/issues/1), CI and running processes before acting.

## Task and timestamp

- **Task:** Protocol 0.1 standalone release, [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Owner:** Protocol session (Claude Code, Opus 5).
- **Checkpoint:** 2026-09-13T10:50Z. **M0 done; M1 implemented and awaiting review.**

## Goal and acceptance

See [PLAN](PLAN.md) and [MATRIX](MATRIX.md).

- **Status:** the release scope and decision records 001–005 are **proposed, not accepted**.
- **Owner decisions open:** U1–U6 in [PLAN §6](PLAN.md#6-unresolved-choices-that-affect-scope-or-public-semantics).
- **Owner input received 2026-09-13:** PIO, CBR and the control plane will be written in Rust. This led to Rust conformance tooling ([decision 001](../../decisions/001-conformance-suite-architecture.md) item 7).
- **Preserved constraints:**
  - Protocol owns contracts and fixtures only.
  - No PIO, CBR, Combraton or benchmark runner implementation.
  - Coordination and Remote trust are unsupported.
  - Nothing is merged or released.

## Git state

- **Branch and base:** repository `Combraton/protocol`, branch `release-0.1/foundation`, base `main` at `f654a29bd6574a75d8ce7c7b76d6a67ef22b45ea`. The branch is pushed to `origin`.
- **Commits (oldest first):**
  - `81f21be` — M0: plan, matrix, draft Core/ENCODING/STREAM, decisions 001–005.
  - `33d6e6b` — Core schemas, encoding vectors, Python/Node cross-checks, Rust tooling decision.
  - `d77fc71` — Rust reference provider with mutants.
  - `d5abb04` — Rust runner, fixture schema, first fixture.
  - `dce3863` — 52 fixtures, 32 mutants, Conformance CI, VERIFICATION.
  - `d0ae764` — five positive fixtures (57 total).
  - A later docs commit updates this handoff and STATE.
- **Uncommitted files at checkpoint:** none besides this handoff and STATE update.

## Dependency state

Requirements were read at combraton `9af69ce`, pio `e65b7c0`, cbr `3278393` and benchmarks `c8d5878` (full SHAs in [PLAN §2](PLAN.md#2-pinned-sources)). No sibling repository was modified.

## What exists now (M1)

- **Schemas:** `schemas/core/1`, `schemas/core-test/1` (conformance-only profile), `schemas/stream/1`. All JSON Schema 2020-12.
- **Draft specs:**
  - [Core](../../spec/profiles/CORE.md): negotiation, closed envelopes with `requires`/`extensions`, processing order, intent digest, deduplication generations, preconditions, epochs, errors, test profile, environment-only test control.
  - [ENCODING](../../spec/bindings/ENCODING.md) and [STREAM](../../spec/bindings/STREAM.md) (stdio form; Unix socket reserved for M2).
- **Cargo workspace** (toolchain 1.97.1, `Cargo.lock`):
  - `conformance/runner`: black-box runner with `self-test`, `check-fixtures`, `run` and `check-mutants`.
  - `conformance/reference`: reference provider with its own parser and canonicalizer, SQLite store and 32 mutants.
- **Suite data:**
  - 57 fixtures under `conformance/fixtures/`.
  - Encoding vectors in `conformance/vectors/encoding.json`, cross-checked by Python `rfc8785` 0.1.4 and Node `canonicalize` 5.0.0 (`conformance/crosscheck`).
- **CI:** `.github/workflows/conformance.yml` (Ubuntu and macOS, plus the cross-check job) alongside the existing Documentation workflow.

## Evidence

Local run on macOS arm64 at `dce3863`, then again for `d0ae764`. Log at the time: session scratchpad `verify-m1.log` (not durable). All exit 0:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo build --workspace --locked`; `cargo test --workspace --locked` | 2 test binaries, all passed |
| `./target/debug/combraton-conformance self-test` | 31 vectors ok |
| `./target/debug/combraton-conformance check-fixtures` | 57 fixtures ok |
| `./target/debug/combraton-conformance run --participant conformance/participants/reference-provider.json` | 57 pass, 0 not passing |
| `./target/debug/combraton-conformance check-mutants --participant conformance/participants/reference-provider.json` | Every mutant killed by every fixture declaring it; no survivors, no timeouts. The kill reasons were inspected and match each mutant's defect. |
| `uv run python generate_encoding_vectors.py --check ../vectors/encoding.json`; Node cross-check | match |
| `python3 scripts/check_docs.py --workspace ..`; `git diff --check` | clean |

**GitHub CI — observed:**

| Head | Runs | Result |
|---|---|---|
| `dce3863` | Conformance 34752476067, Documentation 34752476060 | Both succeeded. Conformance jobs `ubuntu-latest` and `macos-latest` (Rust) plus non-Rust cross-checks all passed. |
| `d0ae764` | Conformance 34752526676, Documentation 34752526711 | Both succeeded; same three jobs passed. |

CI annotation: `actions/checkout` at the pinned v4 SHA targets the deprecated Node.js 20 runtime and is forced onto Node 24. It is not a failure, but the pin should be updated in a follow-up change.

## What remains uncertain

- **Owner decisions U1–U6**, especially U3/U9: the Unix-socket principal credential (M2).
- **Independence:** the fixtures and the only tested provider were written by the same session. Mutants share its assumptions. The spec-only non-Rust implementation (M2) and PIO/CBR review are the next independence evidence.
- **Unreachable states:** environment-only control cannot yet reach crash-mid-transaction states. Nothing is marked `untestable` in M1 because no M1 fixture needs it.
- **Coverage limits:** M1 fixtures cover the stdio binding only. CORE-2, CORE-11–13, CORE-15–17, OBS-*, EFF-* and TRN-3–5 are M2.

## Active resources

No long-running process belongs to this task. Conformance runs spawn short-lived provider processes in temporary directories and clean them up. Local build output is in `target/` and results in `conformance/results/`, both git-ignored.

## State and prompt disposition

[STATE](../STATE.md) is updated. The workspace-local kickoff prompt was rewritten to a continuation pointer for M2 and owner review; it is outside Git.

## Next action

1. Confirm the Conformance CI result for the latest branch head.
2. Present the M1 review packet to the owner.
3. Unless the owner changes scope, begin M2 on this branch or a follow-up branch:
   - grants and principal scopes;
   - events, subscriptions, cursors and gaps;
   - capability snapshots and effects;
   - the Unix-socket binding, once U3/U9 is decided;
   - the spec-only non-Rust Core implementation.
