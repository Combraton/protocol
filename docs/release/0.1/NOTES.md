# Combraton Protocol v0.1.0

The first release of Protocol 0.1: independent semantic contracts for controllers, agentic execution, evidence, context, knowledge and verification services.
- **Release form:** a source release. There are no prebuilt binaries and no package-registry publication.
- **Owner acceptance:** the release candidate was accepted by the repository owner (issue #1).

## What is released

**Six protocol profiles**, each at major version 1:

| Profile | Responsibility |
|---|---|
| `core/1` | Sessions, negotiation (including `core.feature_dependencies`), envelopes, idempotency, preconditions, authority epochs, errors, grants, events and subscriptions, capabilities, credentials, effects |
| `execution/1` | Execution admission, delivery, lifecycle, cancellation, reconciliation, usage, context bindings with revalidation and claim revalidation, evidence outputs |
| `evidence/1` | Artifact descriptors, in-band upload and sealing, fetch, manifests, holds, retention and purge |
| `context/1` | Context requests and preparation, sealed packets, obligations, corrections and read-time facts, claims in packets |
| `knowledge/1` | Claim revisions, reliance decisions under one authority per scope, support ancestry, conflicts and drift, applicability, history |
| `verification/1` | Contracts, jobs with pinned evaluators, receipts, and assessments that never pass missing or outdated verification |

`core-test/1` exists for conformance only.

**Local bindings:**
- `stream/1`: JSON-RPC 2.0 over stdio, and over Unix domain sockets with application-level credentials;
- `encoding/1`: canonical JSON and digests.

**Supported operating systems:** macOS and Linux. Windows is not supported. CI runs on Ubuntu and macOS.

The providers declare `coordination` and `remote-trust` unsupported, as not in this release.

## Assets

| Asset | Contents |
|---|---|
| `combraton-protocol-0.1.0-source.tar.gz` | The exact tagged commit: specs, schemas, fixtures, vectors, the runner and reference provider source, the independent Python provider, third-party example clients, release and consumer documents. It also contains `RELEASE-SOURCE.json` and `BUNDLE-SHA256SUMS`. |
| `combraton-protocol-0.1.0-BUNDLE-SHA256SUMS` | A detached copy of the checksum of every file in the bundle |
| `combraton-protocol-0.1.0-evidence.tar.gz` | CI conformance artifacts at the tested head: result manifests and transcripts, mutant summaries, compatibility runs against the accepted M5 release, and composition results. Run-scoped test credentials are redacted. |
| `release-manifest.json` | Tag, source commit and tree, tested head, versions, normative inventory digest, bundle digest, CI runs |
| `SHA256SUMS` | SHA-256 of every other downloadable asset |

**Two checksum scopes:**
- **Normative inventory** (`docs/release/0.1/inventory.json`): the files consumers pin, which are the specs, schemas, fixtures, vectors and the license.
- **`BUNDLE-SHA256SUMS`**: every file of the bundle, including the toolchain.

## Verifying

**From the archive**, which has no Git metadata:

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing        # Linux: sha256sum -c --ignore-missing SHA256SUMS
tar -xzf combraton-protocol-0.1.0-source.tar.gz && cd combraton-protocol-0.1.0
shasum -a 256 -c BUNDLE-SHA256SUMS                    # every bundle file
python3 scripts/release_inventory.py --verify         # normative inventory (file-system mode)
python3 scripts/check_docs.py && python3 scripts/check_operations.py
cargo build --workspace --locked && cargo test --workspace --locked
./target/debug/combraton-conformance version && ./target/debug/combraton-conformance self-test
./target/debug/combraton-conformance check-fixtures
./target/debug/combraton-conformance run --participant conformance/participants/reference-provider.json
./target/debug/combraton-conformance run --participant conformance/participants/reference-provider-unix.json --out conformance/results/reference-unix
```

**From Git:**

```sh
git clone https://github.com/Combraton/protocol && cd protocol
git checkout v0.1.0 && git verify-tag v0.1.0 2>/dev/null; git rev-parse v0.1.0^{commit}   # compare with release-manifest.json source_commit
python3 scripts/release_inventory.py --verify
```

Then run the same build and conformance commands. [docs/VERIFICATION.md](../../VERIFICATION.md) has the full list, including mutant checks and the compatibility runs against the accepted M5 build. The Rust toolchain is pinned in `rust-toolchain.toml`; the tools report version 0.1.0.

## Mixed-implementation boundaries that were tested

These are the only compositions in which different implementations interoperate, and both put the second implementation in a client role:

- **S-A: an independent execution kernel.** It is a client only, not an `execution/1` provider, written from the contracts. It works against the reference Context provider (which reads claims at a reference Knowledge provider) and reference Evidence providers.
  - It negotiates Context at the Context provider and Evidence at each Evidence provider.
  - It enforces its own required dispatch boundary from CONTEXT §14 read-time claim facts, hashing the packet bytes itself.
  - Shown by its dispatch records:
    - a valid required context dispatches;
    - an advisory-only change does not stop required work;
    - required work whose claim lost its permitted use is withheld, while advisory work proceeds with its gap;
    - wrong or altered bytes are never held;
    - unavailable knowledge is never valid.
  - Broken kernels fail at their intended steps: one ignoring the whole boundary, one trusting the provider's advertised digest, and one ignoring claim invalidation while still verifying bytes.
- **S-B: an independent Evidence publisher.** It uploads and seals a contract and a subject artifact at the reference Evidence provider, checking digests of the bytes it sent and fetched back. The reference Verification provider then consumes the contract by exact reference and digest. A publisher whose bytes differ from its declared digest fails.

The independent Python provider passes the single-provider fixtures over stdio. That is independent evidence for each profile, not a composition.

## Not established by this release

- **Execution-provider interoperability** between implementations. No second implementation serves `execution/1` in a composition.
- **A second implementation in any serving role** in compositions; backpressure and fairness across implementations.
- **Real adapters.** Every Execution evidence path is scripted; adapter fidelity is PIO's gate.
- **Memory, retrieval or context quality.** Those are CBR's own evaluation.
- **The truth of any claim**, the honesty of declared ancestry, the correctness of an evaluator, or semantic equivalence beyond structural comparison.

## Accepted limitations

- No fairness bound among released executions.
- Root is the only other OS user tested for the Unix-socket peer check.
- Capability evidence-source and cursor-past-head checks are checked on the reference only.
- The independent provider serves stdio only, and does not support `execution.claim_revalidation`; feature-triggered dependency forms are checked on the reference.
- Temporal reconstruction views are deferred.
- No signing and no offline receipt verification.
- Coordination and Windows are unsupported.

## Deferred gaps

These are listed with their impact in the [release record](README.md#deferred-gaps-kept-visible):
- Evidence edge cases;
- Context edge cases;
- revalidation timing edges;
- Knowledge: the `conflict.resolve` step 7 order and duplicate `condition_id`;
- Verification: rarer contract violations.

## For PIO and CBR

- **Pin.** Pin tag `v0.1.0` at the commit named in `release-manifest.json`. Never pin a branch or a later `main` commit.
- **Verify** with either procedure above, and record the tag, commit, normative inventory `listing_sha256` and bundle SHA-256.
- **PIO** serves `core/1` and `execution/1`. It calls Context and Evidence only where bindings use them (negotiating each at the provider a binding or reference names), and runs without CBR.
- **CBR** serves `core/1`, `knowledge/1` and `context/1` with `context.claims`, and serves `evidence/1` only if it holds its own bytes. It calls `execution/1` as a client for PIO-backed investigations.
- **Behavior.** Use `core.feature_dependencies` where available, and fall back to the pinned documents on `method_not_found`. Rely only on what negotiation selected.
- **Conformance.** Run the fixtures for the profiles you serve against your own participant descriptor. Claim only the fixtures you pass.
- **Gaps.** A needed behavior the pinned contracts do not define becomes a proposed new feature or major on the protocol repository, never a local widening.

Details: [CONSUMERS](../../work/release-0.1/CONSUMERS.md).
