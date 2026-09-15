# Consumer handoff — Protocol 0.1 (release candidate)

What the PIO and CBR sessions pin, negotiate and run once the owner accepts the Protocol 0.1 release candidate. The release record is [docs/release/0.1](../../release/0.1/README.md): profiles, features, dependencies, inventory with checksums, limitations. The accepted commit and tag are recorded at acceptance.
- **Until then, pin nothing.** A release candidate under review is not a release.
- **Out of scope here:** real PIO adapters, the real CBR memory engine and the Combraton implementation stay outside this milestone and this repository.

## What to pin

- **Only the accepted release.** Pin the commit the owner accepts, reached through the Protocol 0.1 tag created on the owner's authorization.
  - Any other `main` commit, branch or later merge is not equivalent to the accepted release, even when the files you use look unchanged.
  - The release record names the accepted commit.
- **Schemas.** The `schemas/<profile>/<major>/` trees for the profiles you implement or call, verified against the release record's per-file SHA-256 checksums.
- **Fixtures and runner.** The `conformance/fixtures/**` set and runner from the same commit, verified against the release record's inventory, so your conformance run is reproducible.
- **The compatibility contract.** Integer profile majors with named features on the wire (U11).
  - Request majors and features at `core.negotiate`, and rely only on what negotiation selected.
  - For in-session dependencies, `core.feature_dependencies` (CMP-5) reports what negotiation enforces. When a provider answers `method_not_found`, it predates the query: use the requirements in the profile documents you pinned and request them at negotiation anyway. A fallback never drops a required feature.

## Who implements what, and who calls what

A consumer implements (serves) a profile only when other participants call it for that profile. Calling a provider, publishing to an Evidence service or reading packets does not make a consumer a provider of those profiles.

### Implements / serves

| Consumer | Serves (major 1) | Features | Notes |
|---|---|---|---|
| **PIO** (kernel and executor adapters) | `core`, `execution` | Its Execution features, for example `execution.context`, `execution.context_revalidation`, and `execution.claim_revalidation` (which requires `execution.context_revalidation`) where claim-carrying packets gate work | **Optional:** `execution.evidence_outputs`, when it publishes outputs as Evidence references. PIO does not serve `context/1`, `knowledge/1` or `evidence/1` by consuming them. |
| **CBR** (memory engine) | `core`, `knowledge`, `context` | `context.claims` and the obligation features it supports | **Optional:** `evidence/1`, when CBR hosts its own packet and support bytes (a standalone provider MAY serve Evidence for its own packets, CONTEXT §1); `verification/1`, only if CBR records or issues receipts. CBR does not serve `execution/1`. |

### Calls / consumes

| Consumer | Calls (major 1) | Negotiated at | Required or optional |
|---|---|---|---|
| **PIO** | `context` with the obligation features its bindings use, and `context.claims` when it enforces claim boundaries | the Context provider a binding names, for example CBR | **Optional integration.** PIO runs without CBR: work without context bindings needs no Context provider, and a binding names whichever Context provider the caller chose. |
| **PIO** | `evidence` | the Evidence provider a packet or output reference names | Optional; needed to fetch packet bytes or publish outputs |
| **CBR** | `execution` | a PIO-backed executor | **Optional integration**, for PIO-backed investigations: CBR is an Execution client and never an execution provider |
| **CBR** | `evidence` | Evidence providers named by support citations it reads | Optional |
| **CBR** | `verification` | a verifier that holds the receipts it cites | Optional; only when receipts are consumed |

**Required versus optional requests.** Each consumer applies its own uselessness test: mark a profile `required` only when the session is useless without it (CORE §4.2). Older or narrower providers then degrade predictably, through `dependency_not_selected` or unselected optional profiles, instead of failing opaquely.

## Running conformance

From the pinned release checkout (macOS or Linux):

1. **Inventory:** `python3 scripts/release_inventory.py --verify`. Schemas and fixtures must match the release record's checksums.
2. **Documents:** `python3 scripts/check_docs.py`.
3. **Toolchain:** build, then run the runner against the reference participants over stdio and the Unix socket, as CI does. This validates the pinned toolchain itself.
4. **Your implementation:** run the same runner against your own participant descriptor, in the role you serve.
   - Claim only the fixtures you pass.
   - Report skipped and unsupported outcomes; never fold them into passes.
   - A client-only consumer, such as a kernel that calls Context and Evidence, instead runs the third-party composition fixtures with its own client descriptor ([conformance/thirdparty](../../../conformance/thirdparty/README.md)). The minimal kernel there is an example of the enforcement a consumer owns, not a library to depend on.
5. **Mutants:** `check-mutants` applies to implementations that vendor the reference. A fresh implementation relies on the fixture set plus its own tests.

## Starting after acceptance

Do none of this before the owner accepts the release candidate and the tag exists.

1. **Pin.** Check out the accepted tag. Run `python3 scripts/release_inventory.py --verify`, and record the tag, its commit and the inventory's `listing_sha256` in your repository.
2. **Vendor or reference.** Take the `schemas/**` trees for the profiles you serve and call, and the fixture set, from that commit only. Keep `docs/spec/**` from the same commit as the normative text.
3. **PIO session** (Execution provider; Context and Evidence client only when bindings use them):
   - Serve `core/1` and `execution/1` with the features you implement.
   - Run the conformance fixtures against your provider descriptor, over stdio or the Unix socket.
   - Where bindings carry packets: when opening a session to the Context provider a binding names, call `core.feature_dependencies` and negotiate `context/1` with `context.claims` there; negotiate `evidence/1` at the Evidence provider the packet reference names; hash the bytes yourself. Re-read the packet facts at each required boundary.
   - PIO without any Context provider is a supported configuration.
4. **CBR session** (Knowledge and Context provider; Execution client only for PIO-backed investigations):
   - Serve `core/1`, `knowledge/1` and `context/1` (with `context.claims`). Serve `evidence/1` only if CBR holds packet or support bytes itself.
   - Run the Knowledge and Context fixtures against your provider descriptor.
   - When CBR calls a PIO executor, negotiate `execution/1` there as a client and request its required Core features.
5. **Gaps.** A behavior you need that the pinned contracts do not define goes to the protocol repository as a proposed new feature or major, with a demonstrating fixture. Never widen a pinned schema locally.

## What Protocol 0.1 does not guarantee

- **No truth.** A claim's content, an ancestry declaration's honesty and an evaluator's correctness are never established. They are only recorded, classified and checked structurally.
- **No semantics beyond structure.** Conflict and comparability are structural; semantic equivalence is out of scope.
- **No memory or retrieval quality.** CBR's own evaluation plans own that.
- **No real-adapter evidence.** Every Execution evidence path in 0.1 is scripted. Adapter fidelity is PIO's gate, not the protocol's.
- **No Coordination and no Remote trust.** Both are unsupported in 0.1. Receipts are unsigned, and offline verification is deferred.
- **No fairness bound** among released executions, and no Windows support (U4).
- **Bounded interoperability evidence.** Mixed-implementation coverage is exactly the S-A and S-B topology recorded in [M6](M6.md). Everything listed there as remainder is untested across implementations.
- **No temporal reconstruction.** Those views are deferred (M5-Q8). History offers immutable records with explicit links and recorded order instead.

## When something is missing

A contract gap found while building a consumer costs a versioned revision of the protocol (a new feature or major), never an in-place change. Closed objects (decision 002) make silent widening a compatibility break by design. File the gap on the protocol repository with the fixture that demonstrates the need.
