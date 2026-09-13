# Protocol 0.1 standalone release plan

- **Status:** proposed by the Protocol session on 2026-09-13; **scope accepted** by the owner the same day (Owner approval on 2026-09-13 ("yes things looks good"), with the instruction to merge PR #2). Owner decisions are recorded in §6. Accepting the scope is not a release: nothing here is a released contract until the owner accepts the 0.1 release itself.
- **Tracking issue:** [Combraton/protocol#1](https://github.com/Combraton/protocol/issues/1) owns live progress. This file explains how the work fits together; it is not a second task board.
- **Requirements-to-acceptance matrix:** [MATRIX](MATRIX.md). **Handoff:** [HANDOFF](HANDOFF.md).
- **Working branch:** `release-0.1/foundation`, based on `main` at `f654a29bd6574a75d8ce7c7b76d6a67ef22b45ea`.

## 1. In plain language

Protocol is a rulebook shared by programs that do not trust each other's internals. PIO runs coding agents. CBR keeps evidence and prepares context. Other programs can play either role. The rulebook is more than a list of message shapes. It also says what happens when a message is repeated, arrives late, refers to an old revision, needs a feature the other side lacks, or when nobody can tell whether an action happened.

The release is useful only if someone other than its author can check an implementation against it. So the release ships three things together: exact message schemas, written operation semantics, and a conformance suite. The suite contains scripted positive and hostile exchanges plus a runner that plays them against any implementation. PIO and CBR will later pin this release and build against it.

## 2. Pinned sources

Cross-repository requirements in this plan and the matrix were read at these commits. Links elsewhere that track `main` are navigation only.

| Repository | Commit | Documents used |
|---|---|---|
| protocol | `f654a29bd6574a75d8ce7c7b76d6a67ef22b45ea` | README, docs/spec/SPEC.md, docs/VERIFICATION.md, AGENTS.md |
| combraton | `9af69ce966bfacf0deb03606d99f28a355d1f944` | docs/DEVELOPMENT.md, docs/STANDALONE-RELEASES.md, docs/decisions/001, docs/architecture/BASELINE.md, docs/architecture/PLAN.md, templates |
| pio | `e65b7c02318e71e848ab7c8b3f8efab3489fb2d2` | docs/spec/SPEC.md, docs/spec/STANDALONE-CLIENT.md, docs/spec/INTERNALS.md |
| cbr | `32783934b4e52d38a67f4bcc770f16e14b5993e3` | docs/spec/SPEC.md, docs/spec/PREPARATION-AND-DELIVERY.md, docs/spec/INTERNALS.md |
| benchmarks | `c8d5878ab655d090942ad613cd932b44b5e62929` | docs/METHODOLOGY.md |

The historical archive outside these repositories was not consulted for this plan. Research inputs for technical choices are cited in each [decision record](../../decisions/README.md).

## 3. Proposed release scope

### 3.1 What the release contains

| Deliverable | Meaning |
|---|---|
| Profile manifest | Released profiles, their versions and dependencies, feature flags (required or optional) and limits. Coordination and Remote trust are listed as unsupported. |
| Schemas | Machine-readable schemas for every in-scope envelope, operation request/result, event, error and descriptor. |
| Operation semantics | One normative section per operation: preconditions, results, errors, idempotency, authority checks, observations, recovery and what the result does **not** prove. |
| Encoding, digest and transport bindings | Canonical encoding and digest rules with test vectors. A pinned local stream binding: encoding, framing, frame limits, oversized/malformed handling, disconnect and channel authentication. |
| Compatibility and migration policy | Version rules, additive versus breaking changes, feature negotiation, deprecation, and how fixtures change with meaning. |
| Conformance suite | Language-neutral fixtures (positive, negative, adversarial, fault-injected and cross-profile scenarios), a runner, a result manifest format and reproducible commands. |
| Reference and adversarial participants | Reference providers and a reference caller. Deliberately broken "mutant" providers show that each negative fixture detects a wrong implementation. At least one implementation is built from the published documents only, in a different language from the reference. |
| Minimal third-party examples | A minimal executor and a non-Combraton evidence publisher that pass their relevant fixtures using only published material. |
| Release record | Version tag, checksums of schemas/fixtures, tested combinations and untested profiles, known limitations, and a consumer pin instruction for PIO and CBR. |

### 3.2 Profiles in scope

Every profile depends on Core. The dependency rules come from [SPEC §2](../../spec/SPEC.md#2-profiles-over-a-small-core).

| Profile | In scope for 0.1 | Main consumers |
|---|---|---|
| **Core** | Identity and envelope; version, profile and feature negotiation; canonical digests; command processing (idempotency, expected revision, authority epoch); scoped grants; typed errors; capability snapshots; events, subscriptions, cursors, gaps and coverage; effect identity and reconciliation; local transport binding. | Every participant |
| **Execution** | Submission and attempt identity; admission, delivery, runtime, result, exit and evaluation as separate axes; delivery proof classes; completion receipts; inspect/watch; cancel; steer (or explicit unsupported); reconcile; adapter capability predicates and enforcement levels; harness discovery facts; controller leases; native action requests; workspace leases and checkpoints; usage and unresolved liability; context binding and packet delivery observations. | PIO, third-party executors, PIO client, CBR as investigation caller |
| **Evidence** | Descriptors; staged upload and seal; compound manifests; provenance and capture coverage; authorized query/fetch; hold/release; availability and proof-loss reporting. | CBR, PIO as producer, CI publishers |
| **Knowledge** | Claim lineage and immutable revisions; semantic planes; independent reliance, applicability, health and availability states; support and conflict records; reliance decisions under an explicit local authority binding; applicability evaluation; history. | CBR, third-party knowledge services, packet consumers |
| **Context** | Request identity and basis; required items with obligation kinds and reliance labels; separate deadline, investigation budget and output capacity; exact packets with citations, omissions, coverage and unmet items; shared jobs with separate subscriber cancellation; inspect/expand; immutable updates; corrections during preparation; authority-supplied binding content. | CBR, PIO client, other context providers |
| **Verification** | Receipts with subject digests, contract reference, evaluator, environment, per-property results including unavailable checks, scope and validity; `evaluate_contract` job reference. | Verifier providers, CI, CBR |

### 3.3 Explicitly deferred or unsupported

These are absent from 0.1. Absence is declared in the manifest and returns an explicit unsupported result. It is never silently treated as covered.

| Capability | Status in 0.1 | Why |
|---|---|---|
| Coordination profile (projects, workflows, templates, gates, branch adoption) | Unsupported | Combraton implementation follows the standalone gates ([ADR 001](https://github.com/Combraton/combraton/blob/main/docs/decisions/001-standalone-first-and-evaluation.md)). |
| Remote trust profile (TLS binding, cross-boundary delegation, revocation propagation, remote artifact transfer) | Unsupported | Standalone services are local. Remote use needs its own trust model. |
| Signed envelopes and offline signature verification | Deferred with Remote trust | [SPEC §10](../../spec/SPEC.md#10-artifacts-and-attestations) requires signatures only across trust boundaries. Local authenticated provenance stays in scope. |
| Observing an external controller's reliance decisions (`observe_decision` from a Coordination authority) | Deferred with Coordination | Standalone 0.1 uses a local authority binding. |
| in-toto/SLSA export shapes | Deferred optional feature | Useful for exchange, not needed by standalone PIO/CBR interoperability. |
| Windows (named pipes or AF_UNIX) transport and Windows CI | Unsupported in 0.1: the owner selected macOS and Linux only (U4) | Needs a consumer that targets Windows and platform tests. |
| Generated language SDKs as normative artifacts | Proposed deferral; see open choices | Schemas and fixtures are the contract. Bindings are versioned separately. |
| MCP/ACP wrappers, OTLP telemetry mapping | Out of scope | Integration options at other boundaries ([SPEC §1](../../spec/SPEC.md#1-purpose)), not protocol profiles. |
| Performance or task-quality thresholds | Out of scope | They belong to declared evaluation plans in benchmarks. |
| Generated-program worker semantics, collaboration and authority transfer, advanced retrieval | Deferred | Later capabilities through the same contracts ([PLAN Phase 8](https://github.com/Combraton/combraton/blob/main/docs/architecture/PLAN.md)). |

### 3.4 What the release does not claim

Conformance to 0.1 shows that an implementation obeyed the tested exchanges for its claimed profiles and features. It does not prove real-adapter reliability, memory quality, task outcomes, usability, security of an unrestricted harness, or support for untested profiles. A PIO–CBR pass does not certify Coordination or Remote trust. A deterministic fake provider passing the suite says nothing about a native harness's actual guarantees.

## 4. How the conformance suite earns trust

- **Fixtures are data, not code.** They are language-neutral files with stable IDs, the requirement IDs they cover, the profiles and features they need, scripted steps and expected outcomes. Tolerated variations are flagged explicitly rather than hidden in runner logic.
- **The runner treats implementations as black boxes.** It talks to them over the published transport binding only. Test setup, such as restarting a provider or expiring a deduplication horizon, uses a separate, clearly non-normative control channel. Control operations can never appear in the product protocol or grant authority.
- **Negative fixtures must bite.** Each negative fixture names at least one mutant provider it must fail, for example "re-executes duplicates" or "uses the JSON-RPC id as the idempotency key". CI shows that the suite fails every mutant and passes the reference.
- **Two implementations must not share one mistake.** The reference provider and an independent spec-only implementation are built separately, in different languages. Ambiguities they expose become spec fixes, not runner exceptions.
- **Claims are scoped.** The result manifest records implementation identity and version, suite and schema versions, claimed profiles and features, and per-case outcomes (passed, failed, skipped as unsupported, not run).

## 5. Milestones

Near-term work is detailed; later milestones stay coarse and are refined when reached. No milestone completes the release except M6, and M6 needs the owner's acceptance.

### M0 — Scope, matrix and decisions (this session)

This plan, the [matrix](MATRIX.md), proposed [decision records](../../decisions/README.md) for the technical choices, the tracking issue and the handoff. Documentation only.

### M1 — Core command path and conformance harness (first bounded implementation milestone)

**Outcome:** a runnable conformance suite that checks the Core command path against a reference provider and fails a set of mutant providers.

**In scope:**
- Canonical encoding and digest rules with test vectors ([MATRIX](MATRIX.md) CORE-6).
- The stdio form of the local stream binding: framing, frame limit, oversized and malformed frames, disconnect (TRN-1, TRN-2).
- Envelope, error, and version/profile/feature negotiation schemas (CORE-1, CORE-3, CORE-4, CORE-14).
- Core command semantics: bounded decoding, idempotency, deduplication horizon, expected revision, authority epoch, and explicit unsupported profiles (CORE-5, CORE-7 to CORE-10, REL-9).
- Fixture format, runner, result manifest and test-control channel (REL-5, REL-8).
- A reference Core provider. It uses a conformance-only test subject, clearly labeled as not part of any product profile.
- Mutant providers for every M1 negative fixture (REL-6, partial).
- Reproducible commands in [VERIFICATION](../../VERIFICATION.md) and a CI job that runs the suite.

**Out of scope for M1:** grants, subscriptions and events, the Unix-socket binding, effects, and every non-Core profile.

**Acceptance:**
- The documented suite command exits 0 against the reference provider.
- The same suite exits nonzero against each mutant, each failing on its named fixture.
- Every M1 matrix row has at least one positive and one negative fixture.
- The documentation checks still pass.
- CI shows the same results on a clean runner.

### M2 — Core authority, observation, effects and first independent implementation

Grants and scope (issue, audience, expiry, delegation bounds, revocation), existence non-leakage, durable acknowledgment across provider restart, capability snapshots and capability loss, events/subscriptions/cursors, sequence and retention gaps, telemetry lost ranges, effect identity and reconciliation obligations, the Unix-socket binding with peer-credential authentication, and backpressure. A spec-only Core implementation in a second language starts here.

### M3 — Execution profile

All EXE rows with a deterministic reference executor (fake host), mutants and scenarios for lost acknowledgment, crash between journal and dispatch, delayed old-attempt result, two controllers and cancellation acknowledgment loss.

### M4 — Evidence and Context profiles

All EVD and CTX rows plus the cross-profile scenarios. These cover direct composition, missing required context at the deadline, correction during preparation, stale basis, CBR outage, shared-subscriber cancellation and the preparation/resource cycle.

### M5 — Knowledge and Verification profiles

All KNW and VER rows. Adds scenarios for a provider reconnecting with a changed capability and for packets carrying claims.

### M6 — Release candidate

Compatibility fixtures across versions, the minimal third-party executor and evidence publisher, consumer compatibility matrix, complete operation documentation, license, release record and checksums. Owner review and acceptance. Publishing or tagging the release requires the owner's authorization.

## 6. Choices that affect scope or public semantics

Outcomes recorded on 2026-09-13. Choices marked **owner** were decided by the owner; the others are accepted decision records that can still change through a new versioned decision on evidence.

| # | Choice | Outcome | Authority |
|---|---|---|---|
| U1 | Profile scope in §3.2, including Knowledge and Verification | **Accepted** as recommended | owner approval |
| U2 | Verification depth | **Accepted:** receipts plus `evaluate_contract` job references; no signing or evaluator orchestration | owner approval |
| U3 | Grant representation for local 0.1 | **Accepted:** provider-held grant records referenced by ID, with audience, scope, rights, epoch, expiry and delegation bounds. Bearer tokens wait for Remote trust. | owner approval |
| U4 | Supported platforms and transports | **macOS and Linux only** for 0.1. Windows is unsupported; the binding stays abstract so it can be added later. | owner, explicit |
| U5 | Normative language bindings | **Accepted:** none normative in 0.1 | owner approval |
| U6 | Project license | **MIT** ([LICENSE](../../../LICENSE)) | owner, explicit |
| U7 | Schema language and fixture format | Accepted: [decision 002](../../decisions/002-schema-language-and-extensibility.md) | owner approval of PR #2 |
| U8 | Canonical encoding and digest grammar | Accepted: [decision 003](../../decisions/003-canonical-encoding-and-digests.md) | owner approval of PR #2 |
| U9 | Framing and local channel authentication | Framing accepted: [decision 004](../../decisions/004-local-stream-binding.md). The application-level principal credential for Unix sockets is still open; see U12. | owner approval of PR #2 |
| U10 | Runner and reference implementation language | Accepted: Rust, with non-Rust cross-checks ([decision 001](../../decisions/001-conformance-suite-architecture.md)) | owner input and approval |
| U11 | Release and profile version numbering | Accepted: semantic versioning for the release; integer profile majors plus named features on the wire | owner approval |
| U12 | Unix-socket principal credential form | **Open.** The M2 task will bring a concrete recommendation to the owner before sockets are implemented. | owner |

## 7. Risks

- **Scope inflation.** Six profiles is a large first release. Milestones keep each step independently checkable. If the owner narrows U1, the matrix rows move to "deferred" explicitly rather than disappearing.
- **One author's assumptions.** Reference, mutants and fixtures written by one session can share a mistake. The spec-only second implementation and consumer review by the PIO and CBR sessions are the mitigation, so they are scheduled early (M2), not at the end.
- **Test-control leakage.** A convenient control channel could become a hidden production path. It stays in a separate namespace and connection, and product profiles must reject it.
- **Waiting consumers.** The agreed sequence has PIO and CBR build after the release. Late discovery of a contract gap costs a versioned revision, which the sequence already allows.
