# Protocol 0.1 release record

> **Status: Protocol 0.1, released as tag `v0.1.0` after the owner's acceptance.**
> - **Where the release is identified.** The tagged commit, the asset checksums and the CI evidence are in the GitHub release's `release-manifest.json` and `SHA256SUMS`, and in [issue #1](https://github.com/Combraton/protocol/issues/1). This file cannot name its own commit.
> - **What is not equivalent.** No other commit, branch or later `main` merge is equivalent to the release.
> - **Release notes.** [NOTES](NOTES.md) is the body of the GitHub release notes.

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

## Reproducing and verifying

**Two supported procedures:**
- **Extracted release archive** (no Git metadata): see [NOTES](NOTES.md#verifying) and [VERIFICATION](../../VERIFICATION.md). Check `SHA256SUMS`, then `BUNDLE-SHA256SUMS` inside the bundle, then run `scripts/release_inventory.py --verify` (file-system mode) and the build and conformance commands.
- **Git checkout** of tag `v0.1.0`: `release_inventory.py --verify` uses `git ls-files`. The pinned-M5 compatibility commands work only here.

The release assets are produced reproducibly by [`scripts/release.py`](../../../scripts/release.py) from the exact tagged commit. The archive is deterministic: sorted entries, the commit time, fixed ownership, and gzip without a timestamp.

```sh
python3 scripts/release.py bundle --commit v0.1.0 --version 0.1.0 --out dist
python3 scripts/release.py verify-bundle --archive dist/combraton-protocol-0.1.0-source.tar.gz --work "$(mktemp -d)" --full
python3 scripts/release.py evidence --run <CI run id> --out dist/combraton-protocol-0.1.0-evidence.tar.gz
python3 scripts/release.py finalize --tag v0.1.0 --version 0.1.0 --commit v0.1.0 --tested-head <PR head> --ci-run <CI run id> --assets dist
```

CI builds the bundle from the pushed commit and runs the archive path from a fresh extraction on Ubuntu and macOS (conformance workflow, job "Source bundle from a fresh extraction"). It also runs the full checkout commands, the different-OS-user check and the deterministic race regression.

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
