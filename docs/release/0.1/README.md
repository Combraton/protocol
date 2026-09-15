# Protocol 0.1 release record

> **Status: release candidate, not released.**
> - **Owner acceptance.** The owner accepts or refuses the candidate named in [issue #1](https://github.com/Combraton/protocol/issues/1) and on [PR #7](https://github.com/Combraton/protocol/pull/7).
> - **Before acceptance.** There is no tag and no publication. No `main` commit is equivalent to a release.
> - **At acceptance.** The accepted commit and its tag are recorded in the issue and in the tag itself. This file cannot name its own commit.

## What the release contains

| Profile or binding | Major | Features | In-session dependencies (negotiation enforces them; `core.feature_dependencies` reports them) |
|---|---|---|---|
| `core` | 1 | `core.grants`, `core.events`, `core.capabilities`, `core.effects`, `core.events.backpressure` | — |
| `execution` | 1 | `execution.context`, `execution.context_revalidation`, `execution.claim_revalidation`, `execution.evidence_outputs`, `execution.steering`, `execution.actions`, `execution.controller`, `execution.workspaces`, `execution.usage`, `execution.discovery`, `execution.continuation`, `execution.output` | Profile: `core/1` with `core.events`, `core.capabilities`, `core.effects`. Feature: `execution.claim_revalidation` requires `execution/1` with `execution.context_revalidation`. |
| `evidence` | 1 | `evidence.manifests`, `evidence.retention_control`, `evidence.work_binding` | Profile: `core/1` with `core.events` |
| `context` | 1 | `context.advisory`, `context.required_before_start`, `context.required_before_transition`, `context.shared_jobs`, `context.updates`, `context.expand`, `context.claims` | Profile: `core/1` with `core.events` |
| `knowledge` | 1 | — | Profile: `core/1` with `core.events` |
| `verification` | 1 | `verification.jobs`, `verification.record` | Profile: `core/1` with `core.events`, `core.capabilities` |
| `core-test` | 1 | — (conformance only, never a product profile) | Profile: `core/1` |
| `stream` binding | 1 | stdio and Unix domain socket (macOS and Linux) | — |
| `encoding` | 1 | canonical JSON and digests | — |

- **Not in the release.** The providers declare `coordination` and `remote-trust` unsupported (`not_in_release`).
- **Cross-provider dependencies.** Context, Knowledge and Verification cite Evidence artifacts, but none of them requires `evidence/1` in the same session. The reader negotiates `evidence/1` wherever a reference points.
- **Normative sources:** [CORE](../../spec/profiles/CORE.md), [EXECUTION](../../spec/profiles/EXECUTION.md), [EVIDENCE](../../spec/profiles/EVIDENCE.md), [CONTEXT](../../spec/profiles/CONTEXT.md), [KNOWLEDGE](../../spec/profiles/KNOWLEDGE.md), [VERIFICATION](../../spec/profiles/VERIFICATION.md), [STREAM](../../spec/bindings/STREAM.md) and [ENCODING](../../spec/bindings/ENCODING.md), together with `schemas/**` and the conformance fixtures.

## Inventory and checksums

[`inventory.json`](inventory.json) lists every file consumers pin, each with its SHA-256 and size:
- `LICENSE`;
- `docs/spec/**`;
- `schemas/**`;
- `conformance/schemas/**`;
- `conformance/fixtures/**`;
- `conformance/vectors/**`.

It also records an aggregate `listing_sha256` over the whole listing, so adding, removing or changing any file changes it.

## License

MIT ([LICENSE](../../../LICENSE)).

## Reproducing and verifying from a clean checkout

On macOS or Linux, with the Rust toolchain pinned in `rust-toolchain.toml` and Python 3:

```sh
git clone https://github.com/Combraton/protocol && cd protocol
git checkout <the accepted release tag>
python3 scripts/release_inventory.py --verify      # every pinned file matches its checksum
python3 scripts/check_docs.py && python3 scripts/check_operations.py
cargo build --workspace --locked && cargo test --workspace --locked
./target/debug/combraton-conformance self-test
./target/debug/combraton-conformance check-fixtures
./target/debug/combraton-conformance run --participant conformance/participants/reference-provider.json
./target/debug/combraton-conformance run --participant conformance/participants/reference-provider-unix.json --out conformance/results/reference-unix
./target/debug/combraton-conformance run --participant conformance/participants/independent-python-core.json --out conformance/results/independent-python-core
./target/debug/combraton-conformance check-mutants --participant conformance/participants/reference-provider.json
./target/debug/combraton-conformance check-mutants --participant conformance/participants/reference-provider-unix.json --out conformance/results/reference-unix
python3 conformance/scripts/build_pinned.py m5      # compatibility: the accepted M5 build and fixture set
./target/debug/combraton-conformance run --participant conformance/participants/reference-provider.json --fixtures target/pinned-m5/src/conformance/fixtures --out conformance/results/compat/m5-fixtures-reference
./target/debug/combraton-conformance run --participant conformance/participants/pinned/m5-reference-provider.json --filter compat. --out conformance/results/compat/pinned-m5-provider
```

CI runs the same steps on Ubuntu and macOS ([conformance workflow](../../../.github/workflows/conformance.yml)), plus the different-OS-user check and the deterministic race regression.

## Evidence

- **Per requirement:** the matrix rows in [MATRIX](../../work/release-0.1/MATRIX.md).
- **Per milestone:** [M2](../../work/release-0.1/M2.md), [M3](../../work/release-0.1/M3.md), [M4](../../work/release-0.1/M4.md), [M5](../../work/release-0.1/M5.md) and [M6](../../work/release-0.1/M6.md).
- **Independent-implementation divergence records:** [M2](../../work/release-0.1/M2-DIVERGENCES.md), [M3](../../work/release-0.1/M3-DIVERGENCES.md), [M4](../../work/release-0.1/M4-DIVERGENCES.md) and [M5](../../work/release-0.1/M5-DIVERGENCES.md), and the M6 record in [M6](../../work/release-0.1/M6.md).
- **Mixed-implementation composition (S-A, S-B):** [conformance/thirdparty](../../../conformance/thirdparty/README.md).

The verified numbers at the candidate head are recorded on the PR and in the issue checkpoint, from CI's uploaded artifacts.

## Accepted limitations

- **Real adapters.** All Execution evidence is scripted; adapter fidelity is PIO's gate.
- **Fairness.** There is no fairness bound among released executions.
- **OS users.** Root is the only other OS user tested for the Unix-socket peer check.
- **Reference-only checks.** Capability evidence-source and cursor-past-head checks are checked on the reference only.
- **Independent provider.** It serves stdio only.
- **Temporal reconstruction** views are deferred (M5-Q8).
- **Receipts.** No signing and no offline receipt verification (U2, Remote trust).
- **Out of scope for 0.1:** Coordination; Windows (U4).
- **Mixed-implementation coverage is exactly S-A and S-B.** Untested across implementations:
  - a second implementation in a serving role for compositions;
  - `execution/1` provider interoperability between implementations;
  - backpressure and fairness across implementations;
  - the barrier- and signal-synchronized fixtures for non-reference participants;
  - wording drift of `observed` strings;
  - real-adapter behavior.

## Deferred gaps, kept visible

**Evidence:**
- `evidence.availability.changed`;
- shrinking fetch limits;
- a sealed but unavailable manifest child;
- `released_holds` filtering;
- rarer credential-locator forms.

**Context:**
- script `end`/`unmet` in Context's own flows;
- `omit` naming an item;
- `dirty_snapshot` and `environment_digest` in Context's own conditions;
- cancel with no remaining subscriber;
- job-event visibility;
- shared jobs with different fallbacks;
- corrections on advisory items;
- `unverified_items` beyond `binding` and `evidence` items;
- resolved conflicts left out of snapshots.

**Revalidation timing:** advisory checks at dispatch; deadlines and inactivity due at release end.

**Knowledge:** the `conflict.resolve` step 7 order; duplicate `condition_id`.

**Verification:** rarer contract violations (unknown layer, non-boolean `required`, empty lists).

These are not release blockers. A newly found material defect is not covered by this list: it needs resolution or an explicit owner release decision.

## For consumers

[CONSUMERS](../../work/release-0.1/CONSUMERS.md) covers what each consumer implements and what it calls, which files to pin, and how to run conformance.
