# Consumer handoff — Protocol 0.1 (draft for M6)

What the PIO and CBR sessions pin, negotiate and run once Protocol 0.1 is
released. **Draft:** exact tags and checksums are filled in by the release
record ([M6](M6.md) A6). Until a release exists, consumers pin nothing;
real PIO adapters, the real CBR memory engine and the Combraton
implementation stay outside this milestone and outside this repository's
scope.

## What to pin

- **The release:** the Protocol 0.1 tag (created only on the owner's
  authorization) or, failing that, an exact `main` commit. Never a branch.
- **Schemas:** the `schemas/<profile>/<major>/` trees for the profiles
  below, verified against the release record's per-file SHA-256 checksums.
- **Fixtures and runner:** the `conformance/fixtures/**` set and runner
  version from the same tag, so a consumer's conformance run is
  reproducible against the fixture inventory the release names.
- **The compatibility contract:** integer profile majors with named
  features on the wire (U11). A consumer requests majors and features at
  `core.negotiate` and relies only on what negotiation selected — never on
  a provider "probably" supporting a feature. Feature dependencies are
  published per the CMP-5 decision (M6-Q1).

## Who needs which profiles

| Consumer | Profiles (major 1) | Features to request | Role |
|---|---|---|---|
| **PIO** (kernel / executor adapters) | `core`, `execution`, `evidence`, `context` | `execution.context_revalidation`; `context.claims` + `execution.claim_revalidation` where claim-carrying packets gate work; socket sessions authenticate per U12 | Executor and evidence producer |
| **CBR** (memory engine) | `core`, `knowledge`, `context`, `evidence`; `verification` where receipts are consumed | `context.claims` for packet preparation; knowledge operations for claims, facets, ancestry, conflicts and history | Knowledge and context provider or client |

Required versus optional requests follow each consumer's own uselessness
test: mark a profile `required` only when the session is useless without it
(CORE §4.2), so older or narrower providers degrade predictably
(`dependency_not_selected`, unselected optional profiles) instead of
failing opaquely.

## Running conformance

From the pinned checkout (macOS or Linux):

1. `cargo run -p check-fixtures` — fixture and matrix inventory matches the
   release record.
2. `python3 scripts/check_docs.py` — document integrity.
3. The runner against the reference participants, stdio and Unix socket,
   as CI does — this validates the pinned toolchain itself.
4. The same runner against the consumer's own participant in its role
   (provider under test over stdio or socket). A consumer implementation
   claims only the fixtures it passes; skipped and unsupported outcomes are
   reported, never folded into passes.
5. `check-mutants` applies to implementations vendoring the reference; a
   fresh consumer implementation instead relies on the fixture set plus its
   own tests.

## What Protocol 0.1 does not guarantee

- **No truth:** a claim's content, an ancestry declaration's honesty and an
  evaluator's correctness are never established — only recorded, classified
  and checked structurally.
- **No semantics beyond structure:** conflict and comparability are
  structural; semantic equivalence is out of scope.
- **No memory or retrieval quality:** CBR's own evaluation plans own that.
- **No real-adapter evidence:** every Execution evidence path in 0.1 is
  scripted; adapter fidelity is PIO's gate, not the protocol's.
- **No Coordination, no Remote trust:** unsupported in 0.1; receipts are
  unsigned; offline verification is deferred.
- **No fairness bound** among released executions; no Windows support (U4).
- **Bounded interoperability evidence:** mixed-implementation coverage is
  exactly the S-A/S-B topology recorded in [M6](M6.md); everything listed
  there as remainder is untested across implementations.
- **Temporal reconstruction** views are deferred (M5-Q8); history offers
  immutable records with explicit links and recorded order instead.

## When something is missing

A contract gap found while building a consumer costs a versioned revision
of the protocol (a new feature or major), never an in-place change: closed
objects (decision 002) make silent widening a compatibility break by
design. File it on the protocol repository with the fixture that
demonstrates the need.
