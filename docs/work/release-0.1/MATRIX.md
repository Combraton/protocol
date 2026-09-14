# Protocol 0.1 requirements-to-acceptance matrix

**Status:** accepted with the [release plan](PLAN.md) scope on 2026-09-13; rows are satisfied only by the evidence described. Requirement text summarizes the cited source and does not replace it. Fixture IDs are assigned when fixtures are written; until then the acceptance column states what the evidence must distinguish.

## How to read this

**Source keys.** All sources are pinned at the commits in [PLAN §2](PLAN.md#2-pinned-sources).

| Key | Source |
|---|---|
| P | protocol `docs/spec/SPEC.md` |
| PIO | pio `docs/spec/SPEC.md` |
| PIO-I | pio `docs/spec/INTERNALS.md` |
| PIO-C | pio `docs/spec/STANDALONE-CLIENT.md` |
| CBR | cbr `docs/spec/SPEC.md` |
| CBR-P | cbr `docs/spec/PREPARATION-AND-DELIVERY.md` |
| CBR-INT | cbr `docs/spec/INTERNALS.md` |
| REL | combraton `docs/STANDALONE-RELEASES.md` |
| PLAN | combraton `docs/architecture/PLAN.md` |
| DEV | combraton `docs/DEVELOPMENT.md` |

**Participant keys** say who produces the evidence:

- **R:** reference participant.
- **M:** mutant provider that must fail.
- **A:** adversarial peer (malformed, hostile or out-of-order input).
- **F:** fault injection through environment test controls only: launch configuration, the scripted executor, the clock file, runner-driven process kills, pipelined sends and declared barriers ([decision 007](../../decisions/007-execution-test-controls.md), proposed). Never through a protocol operation.
- **I:** independent spec-only implementation.
- **X:** cross-profile scenario with several reference participants.

**Milestones** are defined in [PLAN §5](PLAN.md#5-milestones).

A row is satisfied only when its positive **and** negative evidence exists and CI runs it. Documentation checks never satisfy a row.

## Release deliverables

| ID | Requirement | Source | Acceptance evidence | M |
|---|---|---|---|---|
| REL-1 | Machine-readable schemas for every in-scope message | REL Protocol gate; P §3 note | Schemas validate against their metaschema. Every positive fixture message validates, and each schema-invalid negative fails for the stated reason. A coverage check shows no in-scope operation lacks a schema. | M1–M6 |
| REL-2 | Profile and dependency manifests | P §2; PLAN Phase 1 | Manifest schema. R advertises dependencies. A: manifest claiming Knowledge without Evidence is rejected by a conforming caller. | M2 |
| REL-3 | Normative operation semantics | REL; DEV §5 | Each operation section lists preconditions, results, errors, idempotency, observations and non-claims, and links its fixtures. A coverage check shows ≥1 positive and ≥1 negative fixture per operation. | M1–M6 |
| REL-4 | Compatibility and migration policy | REL; DEV §5 | Policy document plus CMP rows. | M6 |
| REL-5 | Reproducible conformance commands | REL; P last § | One documented command with a pinned toolchain runs the suite against a named participant, with a meaningful exit status; CI runs it. | M1 |
| REL-6 | Independent and adversarial participants | REL; P §13, last § | Every negative fixture fails ≥1 named M. R passes. I (different language, built from documents only) passes the claimed profiles. Divergences are resolved in spec or fixtures, not runner exceptions. | M1–M6 |
| REL-7 | Third-party minimal executor and non-Combraton evidence publisher | P §13; PLAN Phase 1 exit | Both examples use only published schemas and documents and pass their relevant fixtures. | M6 |
| REL-8 | Scoped conformance claims | P last §; REL | The result manifest names implementation and version, suite/schema/fixture versions, claimed profiles and features, and per-case outcome (`pass`, `fail`, `timeout`, `harness_error`, `unsupported`, `skipped`). CI uploads manifests, transcripts and mutant summaries from every job. | M1, M2 |
| REL-9 | Unsupported profiles explicitly unimplemented | REL; PLAN Phase 1 | Manifest lists Coordination and Remote trust as unsupported. A request for them gets an explicit unsupported error. M: provider that accepts an unknown profile. | M1 |
| REL-10 | Versioned release recorded for consumers | REL | Tag, checksums and release record with a pin instruction. Requires the owner's authorization. | M6 |
| REL-11 | License | README | LICENSE present, as selected by the owner. | M6 |
| REL-12 | Test controls stay outside the production protocol | CORE §13.1; decision 001; decision 007 | No operation, method, event type or error is reserved for testing; every fault fixture uses only environment controls (decision 007). Clock robustness: `core.grants.test-clock-never-moves-backward`, with M `clock-follows-backward-time` and `clock-malformed-resets`. Unsupported controls are recorded as `coverage_limits`, never passes. A: a provider launched without a conformance configuration exposes no test behavior. | M1–M3 |
| REL-13 | Races found in an implementation get deterministic regressions | M2 close-out; decision 007 | `socket.subscription-recheck-race-regression` reproduces the M2 idle re-check race with barrier `subscription.recheck.after_authorization` and signal `processing.lock.contended`. The correct provider passes every repeated run; mutant `recheck-outside-lock` fails every run at step 13 by delivering the put committed after the revoke (`repeat_fixture.py`, CI). Implementation-specific: `unsupported` coverage limit for participants without the barrier. | M3 |

## Core: identity, negotiation and command processing

| ID | Requirement | Source | Acceptance evidence | M |
|---|---|---|---|---|
| CORE-1 | Message, command, subject, request-response and native identities are distinct | P §3, §11 | R: retransmit with a new message ID and new JSON-RPC ID returns the prior result. A: reuse a JSON-RPC ID with a different command. M: dedupes by JSON-RPC ID. | M1 |
| CORE-2 | Causation references preserved; timestamps do not order or prove causality | P §3 | R: `caused_by` round-trips. Events with non-monotonic recorded time are ordered by sequence. M: orders by timestamp. | M2 |
| CORE-3 | Negotiate major versions, profiles, required features, limits and optional methods | P §11 | R: compatible negotiation. A: no common major gives an explicit version error. A: an unknown required feature is refused. | M1 |
| CORE-4 | Unknown required semantics fail closed; unknown optional metadata may be preserved | P §4, §11; README | A: an unknown required-marked extension is refused. R: an unknown optional extension is accepted and preserved where the operation promises preservation. M: ignores the required marker. | M1 |
| CORE-5 | Bounded decoding and declared payload limits | P §4, §11 | A: at the limit is accepted; limit + 1 is rejected with the declared behavior. M: unbounded reader. | M1 |
| CORE-6 | Canonical encoding and algorithm-qualified digests | P §3, §10; PLAN Phase 1 | Test vectors: key order, escapes, number forms, duplicate keys, invalid UTF-8. A: digest mismatch and an unsupported algorithm are rejected. M: digests non-canonical bytes. | M1 |
| CORE-7 | Matching duplicate returns the prior result; a conflicting payload is `idempotency_conflict` | P §4 | R and A, both cases. M: re-executes the duplicate. M: ignores the payload digest. | M1 |
| CORE-8 | Expired deduplication horizon returns `dedupe_history_unavailable`, never "new" | P §4 | F: expire the horizon, then replay. M: treats the replay as new. | M1 |
| CORE-9 | Expected revision applies per owner subject; zero means "must not exist" after the idempotency lookup; multi-object preconditions | P §3, §4 | A: stale revision rejected with the current revision. Duplicate of a successful create still returns the prior result, which catches the ordering mistake. A: a multi-object precondition with one stale object is rejected atomically. M: checks revision before idempotency. | M1 |
| CORE-10 | Stale authority epoch rejected | P §4, §12 | A: command with an old epoch after an epoch advance. M: ignores the epoch. | M1 |
| CORE-11 | Scoped grants: principal, audience, scope, rights, epoch, expiry, delegation bounds; checked per operation | P §12 | A: wrong audience, missing right, expired grant, over-broad delegation, revoked grant used after revocation. R: valid grant succeeds. M: namespace-only check. | M2 |
| CORE-12 | No cross-scope existence leak through deduplication or fetch | P §12 | A: an unauthorized existing object and a nonexistent object give indistinguishable results. M: returns "forbidden" versus "not found". | M2 |
| CORE-13 | Durable acknowledgment with owner revision and operation/effect references | P §4 | F: provider restart after acknowledgment; the duplicate returns the same acknowledgment. M: in-memory acknowledgment. | M2 |
| CORE-14 | Typed errors with retry class; semantic rejection distinct from transport failure | P §4 | Error registry schema. Every negative fixture asserts a registered code. A rejected query creates no operation record. | M1 |
| CORE-15 | Unauthenticated traffic is refused before semantic processing | P §4, §11 | A: connection failing channel authentication gets no semantic response. Provider-local audit records are outside conformance; this limit is documented. | M2 |
| CORE-16 | Versioned capability snapshots; capability loss prevents new dependent admission and prompts reconciliation | P §8, §11 | F: capability withdrawn. A dependent command is refused and existing work reports reconciliation needed. M: admits after loss. | M2 |
| CORE-17 | Active work keeps the version that defined its meaning | P §11; P §13 | F: provider reconnects with a changed capability version. Existing work keeps its pinned meaning. | M2 |

## Core: observation, effects and transport

| ID | Requirement | Source | Acceptance evidence | M |
|---|---|---|---|---|
| OBS-1 | Events carry stream ID, stream epoch, sequence, owner revision, recorded time and normalized payload | P §3 | Schema plus R stream fixtures. | M2 |
| OBS-2 | Subscribe with filter and cursor; response identifies snapshot/checkpoint basis | P §7 | R resume from a cursor. A: invalid or foreign cursor rejected. | M2 |
| OBS-3 | Reconnect may replay duplicates; consumers handle them without repeating transitions | P §7 | Provider side: F reconnect replays within declared rules. Consumer side: reference-caller fixture receives duplicates and exposes no double transition. | M2, M6 |
| OBS-4 | Sequence gaps pause dependent interpretation until repaired or declared unavailable | P §7 | A: runner-as-provider sends a gap to the reference caller. R provider never emits undeclared gaps. | M2 |
| OBS-5 | Retention gap returns a typed snapshot plus coverage boundary, never an incomplete "complete" replay | P §7 | F: compact history, then subscribe from an old cursor. M: silent partial replay. | M2 |
| OBS-6 | Stream epoch change is explicit, with recovery coverage | P §3 | F: epoch change. The new epoch is announced with its coverage. M: silent sequence reset. | M2 |
| OBS-7 | Semantic events never dropped; telemetry loss has an explicit range and byte count where known | P §7 | F: the scripted executor exceeds a bounded output spool. `execution.output.read` reports lost ranges with offsets, byte counts where known and coverage effect, while every semantic event is still delivered. M: drops output silently; M: reports a lost range without byte coverage when bytes are known. | M3 |
| OBS-8 | Opaque cursors; bounded metadata reads with declared coverage; unavailable is not empty | PIO-I §7 | A: unavailable projection reported as unavailable, not an empty list. | M2 |
| OBS-9 | A subscription whose authorization lapses ends within a bound, even while idle, and nothing committed after the lapse is delivered | CORE §16.5; P §12 | Revocation from another connection: `socket.idle-subscription-ends-on-revocation-elsewhere` (M2). Expiry while idle: `socket.idle-subscription-ends-at-grant-expiry`, where the clock file passes `expires_at` and the idle subscriber gets `ended` within 2 s with no later item. Expiry at the next request on any binding: `core.events.subscription-ends-at-grant-expiry`. M: `clock-file-ignored`, `idle-subscriptions-not-rechecked`. The deterministic race regression is under REL-13. | M2, M3 |
| EFF-1 | Effect descriptor: ID, kind, target, payload digest, authorization, retry class, status evidence | P §6 | Schema for effect descriptors and status observations (CORE §19). R: a prompt submission records its effect before external I/O, observable through `core.effects.get`. M: records the effect after dispatch, so a scripted crash between journal and dispatch leaves no effect. Fixtures (M3 step 3–4): `execution.submit-records-effect-and-links-retries`, `execution.crash-before-dispatch-is-never-delivered`. | M3 |
| EFF-2 | On response loss, query by the same effect ID; an unanswerable provider leaves an unresolved obligation | P §6 | F: the scripted executor loses the submit response, then the caller queries by effect ID and sees the same effect. A: the script makes the outcome unanswerable, so status stays `unknown` with an open obligation. M: reports `not_found` or `failed` for a recorded effect. Fixtures (M3 step 3–4): `execution.unknown-effect-outcome-is-not-not-found`, `execution.crash-during-dispatch-stays-ambiguous-then-reconciled`. | M3 |
| EFF-3 | Retry permitted only per retry class | P §6 | Fixtures per class through the scripted executor: `read` retry allowed; `idempotent_key` retried only with the same key; `non_repeatable` after `unknown` is never retried automatically. M: blind retry of a non-repeatable effect. | M3 |
| EFF-4 | Obligations and terminal coverage; a closed wait (`aborted`) does not prove the effect did not happen | P §6 | F: the clock file moves past an obligation deadline with no other events; a provider-origin event marks it `overdue`. Aborting the wait leaves the effect `unknown`. M: aborting marks the effect `failed`; M: no overdue event without other traffic. Fixtures (M3 step 3–4): `execution.timeouts-are-distinct-and-prove-nothing` (overdue; explicit abort pending). | M3 |
| TRN-1 | JSON-RPC 2.0 mapping; events as notifications; JSON-RPC ID is not the idempotency key | P §11 | Binding document plus CORE-1 fixtures. A: JSON-RPC-level errors versus domain errors. | M1 |
| TRN-2 | Pinned framing: encoding, max frame length, oversized rejection, malformed frames, disconnect | P §11 | A: frame at the limit, limit + 1, invalid UTF-8, truncated frame then EOF, garbage between frames, peer closing mid-response. | M1 |
| TRN-3 | Authenticated local channel (Unix socket) and a stdio binding | P §11 | Binding document. A: peer credential mismatch refused on macOS and Linux CI. | M2 |
| TRN-4 | Separate backpressure policies for semantic events and telemetry | P §7 | F: the runner pauses reading a subscriber session while the scripted executor produces events and output. The semantic subscription ends with `consumer_too_slow` and resumes from its cursor with no skipped event. Telemetry coalescing is declared with lost ranges. Both bindings. M: drops semantic events under pressure. | M3 |
| TRN-5 | Disconnect is not cancellation; the caller reconciles in-flight commands by command ID | PIO-C §2, §6 | F: disconnect after sending a command; reconnect, query, and see exactly one effect. | M2 |

## Execution

| ID | Requirement | Source | Acceptance evidence | M |
|---|---|---|---|---|
| EXE-1 | Submit returns an execution reference. A retry is a new execution linked to its predecessor. | P §3, §8; PIO §4; PIO-I §2 | R: submit creates the execution at revision 1 with `predecessor` linkage for a retry; a retransmitted submit returns the same reference. M: reuses the predecessor's execution ID for a retry. Fixtures (M3 step 3–4): `execution.submit-records-effect-and-links-retries`, `execution.requires-core-features`. | M3 |
| EXE-2 | Admission (queued, admitted, refused, with reason); a required restriction that cannot be enforced is refused with an alternative | PIO §5, §7 | A: a restriction requiring `enforced` against a script advertising `cooperative` is `refused` with `enforcement_unavailable` and an alternative. R: queued with a reason under a capacity limit set by launch configuration. M: admits with weaker enforcement. Fixtures (M3 step 3–4): `execution.admission-refuses-unenforceable-or-unknown-requirements`. | M3 |
| EXE-3 | Delivery axis and proof classes; ambiguity preserved, later reconciliation appended | PIO §5 | F: the script crashes after writing the prompt; after restart delivery is `ambiguous`, then reconciliation appends `delivered`, with both events retained. Proof-class fixtures for `provider_ack_id`, `echo`, `transport_only` and `bytes_written`. M: overwrites the ambiguity; M: labels `bytes_written` as acknowledged. Fixtures (M3 step 3–4): `execution.delivery-proof-classes-stay-distinct`, `execution.crash-during-dispatch-stays-ambiguous-then-reconciled`, `execution.crash-before-dispatch-is-never-delivered`. | M3 |
| EXE-4 | Runtime axis includes `requires_action` (action ID, owner) and `unknown` | P §5; PIO §5 | R: the script requests an action, so runtime is `requires_action` with `action_id` and owner, and `unknown` after an unobservable host. Schema. M: represents `requires_action` as a progress level. | M3 |
| EXE-5 | Result, exit and evaluation are separate; evaluation is never inferred from exit | PIO §5 | R: exit 0 with `evaluation: not_requested`. M: sets evaluation from exit 0. Fixtures (M3 step 3–4): `execution.evaluation-is-never-derived-from-exit`. | M3 |
| EXE-6 | Immutable completion receipt: matching duplicate returns it, conflict is rejected, an old attempt cannot finalize its successor | PIO §5; PIO-I §6 | F through the script: duplicate completion returns the receipt; conflicting completion is rejected and retained; a late old-attempt result is retained as `superseded_attempt` and does not finalize. M: last write wins; M: an old attempt finalizes its successor. Fixtures (M3 step 3–4): `execution.completion-receipts-resist-conflicts-and-old-attempts`. | M3 |
| EXE-7 | Inspect and watch execution facts with cursors | P §8; PIO §10 | R: Core events and subscriptions with `kinds: ["execution"]`, reusing OBS fixtures on execution streams, on both bindings. | M3 |
| EXE-8 | Cancel returns a request receipt; `cancel_requested` is not `cancelled`; lost cancel acknowledgment is recoverable | P §5, §13; PIO §8 | F: the script drops the cancel acknowledgment; retransmitting returns the same receipt. The script refuses cancellation, so the outcome is `refused`, not `cancelled`. M: reports `cancelled` on request. Fixtures (M3 step 3–4): `execution.cancel-returns-a-request-receipt` (refusal and replayed receipt). | M3 |
| EXE-9 | Steering: recorded, native-acknowledged and observed behavior are separate; unsupported is explicit | P §12; PIO §7 | A: steer against a script without live steering gives explicit `not_supported` with an alternative. R: recorded, acknowledged and observed facts stay separate. M: claims delivery without evidence. Optional feature `execution.steering`. | M3 |
| EXE-10 | Reconcile by command or delivery ID returns scoped observations and never resubmits | P §8; PIO §8 | F: the script loses the submit acknowledgment; `execution.reconcile` by command ID shows exactly one execution and one effect. M: reconcile resubmits. Pairs with SCN-2. Fixtures (M3 step 3–4): `execution.reconcile-after-lost-acknowledgment-never-resubmits`. | M3 |
| EXE-11 | Capability predicates with adapter/harness/protocol versions, probe time, limits and enforcement level; unknown is not supported | PIO §3; PIO-I §3 | Schema for adapter predicates. A: a predicate missing from the script's capability report is `unknown`, and a dependent submit is refused. M: defaults to supported. Fixtures (M3 step 3–4): `execution.admission-refuses-unenforceable-or-unknown-requirements`. | M3 |
| EXE-12 | Discovery facts kept separate: detected, recognized, version supported, authentication known or unknown, reachable, last verified | PIO-C §3 | Schema and R for `execution.discovery`. A: detected with authentication unknown is not offered as usable. Optional feature; may be deferred to PIO's own gate if no executor-neutral fixture is meaningful. | M3 |
| EXE-13 | One mutating controller lease with epoch; stale controller refused; read-only observers coexist | PIO §6; P §13 | A: two sessions claim the controller lease of one scripted host; the stale controller's commands get `stale_authority_epoch`. F: reconnect increments the epoch. Observers unaffected. M: accepts the stale controller. Optional feature `execution.controller`. | M3 |
| EXE-14 | A native action request belongs to its execution; answering one authorizes nothing else | PIO-C §4 | A: answer execution A's `action_id` in execution B, which is `not_found` and authorizes nothing. M: global action namespace. Optional feature `execution.actions`. | M3 |
| EXE-15 | Workspace lease descriptor and checkpoint coverage (dirty and untracked state); agent-reported SHAs are annotations | PIO §7, §10 | Schema. A: checkpoint with incomplete coverage is declared incomplete. M: reports an agent-reported commit ID as a receipt. Optional feature `execution.workspaces`. | M3 |
| EXE-16 | Usage basis (observed, estimated, unknown, enforced bound); unresolved liability after timeout; unenforceable hard ceilings refused | PIO §9; PIO-I §5 | F: the clock passes the execution deadline with an unknown-usage invocation; liability stays unresolved and the budget stays reserved. A: a hard dollar ceiling without an enforced bound is refused. M: refunds on timeout. Optional feature `execution.usage`. | M3 |
| EXE-17 | Context binding at submit; required-before-start without a satisfied binding is not admitted as ready | PIO context §; CBR-P §4, §6 | A: `required_before_start` with an unmet binding is not admitted as ready. R: advisory proceeds with a visible gap. Binding and admission only; Context profile semantics are M4. Optional feature `execution.context`. | M3 |
| EXE-18 | Packet delivery observation: exact digests, target, boundary, outcome including late, unavailable and unknown | CBR-P §6, §7; PIO context § | F: the script delivers an update after its dependent boundary, recorded `late`; unsupported boundary is `unavailable`. M: no late state. Cross-profile evidence with Context in M4. | M3 |
| EXE-19 | Queue, delivery, execution-deadline, inactivity and reconciliation timeouts are distinct | PIO §9 | F: the clock file passes each of the five timeouts separately; each yields its own event, and execution-deadline expiry does not claim remote work ended. Schema. M: collapses timeouts; M: marks the effect failed on deadline. Fixtures (M3 step 3–4): `execution.timeouts-are-distinct-and-prove-nothing` (delivery and execution deadline; queue, inactivity and reconciliation pending). | M3 |
| EXE-20 | Opaque caller correlation without a Combraton model | PIO §10 | R: arbitrary correlation round-trips through submit, inspect and events. Fixtures (M3 step 3–4): `execution.submit-records-effect-and-links-retries`. | M3 |
| EXE-21 | Outcome references exported artifacts and evidence | P §9; PIO §10 | X with Evidence in M4: outcome references exported artifacts and evidence descriptors. | M3–M4 |
| EXE-22 | Detach and close are not cancellation; a restarted client queries rather than replays | PIO-C §2 | F: disconnect or kill the client session while the script runs; reconnect observes the same execution without replay. M: cancels on disconnect. | M3 |
| EXE-23 | Fresh continuation is labeled differently from native resume | PIO §6 | A: resume without native support yields a `fresh_continuation`, never `resumed`. M: labels a fresh continuation as resumed. Optional feature `execution.continuation`. | M3 |

## Evidence

| ID | Requirement | Source | Acceptance evidence | M |
|---|---|---|---|---|
| EVD-1 | Descriptor with qualified digest, media type, size, owner/locator, producer, capture basis, visibility, retention class; the locator is not identity and has no credentials | P §10; CBR §3 | Schema. A: credential-bearing locator rejected. M: identity by locator. | M4 |
| EVD-2 | Staged upload and seal verifies bytes; partial upload cannot be sealed; seal is idempotent | P §8, §13; CBR §3 | A: partial upload, digest mismatch, duplicate seal. M: seals unverified. | M4 |
| EVD-3 | Compound manifests declare required children; incomplete bundles detected | P §10 | A: missing child. M: reports complete. | M4 |
| EVD-4 | Provenance: authenticated producer, source identity, scope, capture time uncertainty, anchors, completeness | P §10; CBR §3 | Schema plus A: producer not matching the channel principal. | M4 |
| EVD-5 | Authorized query and fetch without existence leaks | P §8, §12 | Reuses CORE-12 on evidence. | M4 |
| EVD-6 | Hold and release obligations; availability states; proof-loss report on purge | P §8; CBR §4, §10 | F: purge. Availability becomes purged and the report lists affected references. M: silent disappearance. | M4 |
| EVD-7 | Ingestion coverage declared; terminal output is not a complete tool trace | PIO-C §5 | Schema: coverage field required. A: omitted coverage rejected. | M4 |
| EVD-8 | Work binding constrains evidence destination and producer identity | P §9 | X: executor publishes evidence outside its binding and is refused. | M4 |

## Knowledge

| ID | Requirement | Source | Acceptance evidence | M |
|---|---|---|---|---|
| KNW-1 | Claim lineage and immutable revisions; revise checks the base revision | CBR §4; CBR-INT §2 | A: stale base rejected. M: overwrites revision. | M5 |
| KNW-2 | Normative, observed and interpreted content stay distinguishable | CBR §4 | Schema plus A: drift represented without deleting either side. | M5 |
| KNW-3 | Reliance, applicability, subject health and evidence availability are independent | CBR §4 | A: accepted-for-use plus invalid-for-target coexist. M: single status field. | M5 |
| KNW-4 | Support references evidence; repetition with shared lineage is not corroboration | CBR §6; CBR-P §9 | A: repeated claims with one lineage do not raise support class. | M5 |
| KNW-5 | Conflicts preserve competing values; derivation cannot choose a winner | CBR §6 | A: model-origin conflict resolution refused without authority. | M5 |
| KNW-6 | Reliance decisions need an explicit authority binding; producers cannot self-accept; one accepting authority per bound scope | CBR §6; CBR-INT §1 | A: self-acceptance and a second authority for the same scope are refused. | M5 |
| KNW-7 | Applicability evaluated against a target basis; incomplete coverage gives `needs_check` or `unknown` | CBR §5 | A: missing dependency coverage. M: reports applicable. | M5 |
| KNW-8 | History inspection; supersession retains history | CBR §11 | R: superseded revision still retrievable with its state. | M5 |
| KNW-9 | Extracted instructions do not become policy without an authority operation | CBR §3 | A: derived "rule" has no binding reliance until decided. | M5 |

## Context

| ID | Requirement | Source | Acceptance evidence | M |
|---|---|---|---|---|
| CTX-1 | Request identity precedes execution identity; causal link kept on admission | P §14; CBR-P §6 | X: request, packet, submit; the execution references the request. | M4 |
| CTX-2 | Target basis manifest: repository trees, dirty snapshots, environment/configuration/build, multi-repository | P §14; CBR-P §6 | Schema. A: commit-only basis for a dirty workspace is flagged incomplete. | M4 |
| CTX-3 | Required items name an obligation kind (advisory, before named transition, before start), reliance label and selecting authority; they must be checkable | CBR-P §4 | Schema. A: unknown obligation kind refused. | M4 |
| CTX-4 | Deadline, investigation budget and output capacity are separate limits | P §14; CBR-P §6 | Schema. A: one limit exhausted leaves the others reported independently. | M4 |
| CTX-5 | Result: exact bytes and digest, selected revisions, provenance, inclusion/omission report, citations, labels | CBR-P §6; CBR §7 | R: fetched bytes match the digest. A: digest mismatch detected. M: regenerates bytes on fetch. | M4 |
| CTX-6 | Per-producer coverage frontiers and gaps; no global cursor | CBR-P §6 | Schema plus A: coverage from two producers kept distinct. | M4 |
| CTX-7 | Missing required item at deadline stays unmet; advisory degrades only per declared fallback | CBR-P §4, §6 | F: deadline passes. The required item is unmet and advisory follows its fallback. M: downgrades required to advisory. | M4 |
| CTX-8 | Mandatory constraints that do not fit give `budget_insufficient` | CBR §7 | A: capacity below the mandatory size. M: silently drops. | M4 |
| CTX-9 | Unsupported required timing or delivery semantics are rejected explicitly | CBR-P §6 | A: unsupported boundary requested. | M4 |
| CTX-10 | Shared preparation job; subscriber cancellation is separate | CBR-P §2, §9; PIO-C §6 | X: two subscribers, one cancels, and the other still receives its packet. M: cancels the job. | M4 |
| CTX-11 | `inspect_packet` returns exact bytes and omissions; `expand` returns only newly authorized cited content | P §8 | A: expand beyond grant refused. | M4 |
| CTX-12 | Updates are immutable versioned deltas with basis; earlier packets retained; old bytes never relabeled current | CBR-P §7; PIO-C §5 | R: initial plus update, both retrievable. M: mutates the packet in place. | M4 |
| CTX-13 | Correction during preparation: an older derivation cannot replace the current binding | CBR-P §9 | F: correction mid-job. The result is historical, rebuilt or refused, never the stale current. | M4 |
| CTX-14 | Authority-supplied binding content recorded with authority revision and coverage; no claim of unobserved events | CBR-P §2 | A: packet claims a frontier beyond what was ingested and is refused by the checker. | M4 |
| CTX-15 | Applicability and invalidation conditions; stale basis detected before admission | CBR-P §6, §7 | X: basis changes between packet and submit, so admission revalidation fails. | M4 |
| CTX-16 | Direct fetch under a bound read grant scoped to the packet and audience | P §9 | X: executor fetches with the grant. A: other audience or other packet refused. | M4 |
| CTX-17 | Late delivery recorded as late; no prevention claim | CBR-P §7, §9 | Reuses EXE-18 across profiles. | M4 |
| CTX-18 | Preparation does not require the consumer to hold an execution reservation | CBR-P §5; PIO-I last § | X: capacity-one executor. The consumer waits without a reservation while the preparation investigation runs and completes. | M4 |
| CTX-19 | Execution initiated for a memory job is not automatically re-enriched; depth and call budgets declared | PIO-C §6 | X: CBR-origin submit carries its initiator and no implicit enrichment request. | M4 |
| CTX-20 | Packet labels distinguish binding, observation, hypothesis, unknown, declared requirement, source inspected, runtime observed, inferred and stale | CBR §7; CBR-P §8 | Schema plus A: an unlabeled claim in a packet is refused. | M4 |

## Verification

| ID | Requirement | Source | Acceptance evidence | M |
|---|---|---|---|---|
| VER-1 | Receipt with subject digests, contract reference and digest, evaluator, environment, per-property results including unavailable, scope and validity | P §8, §10 | Schema. A: missing unavailable-check listing refused. M: collapses into pass/fail. | M5 |
| VER-2 | A failed verification is a scoped observation, not a stop command | P §12 | X: failed receipt does not change execution state. | M5 |
| VER-3 | `evaluate_contract` returns a job reference; the report is a separate receipt | P §8 | R plus schema. | M5 |

## Cross-profile scenarios and compatibility

| ID | Requirement | Source | Acceptance evidence | M |
|---|---|---|---|---|
| SCN-1 | Direct executor–context composition flow | P §9; DEV §8 | X with reference caller, executor and context provider. | M4 |
| SCN-2 | Lost submit acknowledgment creates no duplicate | PLAN §13; PIO §8 | F: the script loses the submit acknowledgment; retransmission and reconcile show one execution and one prompt effect. EXE-10. Fixtures (M3 step 3–4): `execution.reconcile-after-lost-acknowledgment-never-resubmits`. | M3 |
| SCN-3 | Delayed old-attempt result after replacement | P §13; PIO §8 | F: the script returns an old attempt's result after the controller replaced it; retained under the old identity, successor not finalized. EXE-6. Fixtures (M3 step 3–4): `execution.completion-receipts-resist-conflicts-and-old-attempts`. | M3 |
| SCN-4 | Missing required context at deadline; advisory proceeds with a gap | CBR-P §9; REL combined gate | X plus CTX-7. | M4 |
| SCN-5 | Correction during preparation and before dispatch | CBR-P §9; REL combined gate | X plus CTX-13 and CTX-15. | M4 |
| SCN-6 | Preparation needs the consumer's slot | CBR-P §9; PLAN §13 | X plus CTX-18. | M4 |
| SCN-7 | Provider reconnect with changed capability | P §13 | F plus CORE-16 and CORE-17. | M5 |
| SCN-8 | Context provider outage: advisory versus required | PIO-C §6; REL combined gate | F: provider unavailable. Advisory proceeds visibly; required waits at its named boundary. | M4 |
| SCN-9 | Wrong packet revision or stale source at admission | DEV §8 | X plus CTX-15. | M4 |
| SCN-10 | Crash between journal and dispatch | DEV §8; PIO §4 | F: the provider is killed at the barrier or script point between journal and dispatch; after restart the effect is recorded and delivery is `pending` or `ambiguous`, never silently re-sent. EXE-3, EFF-1. Fixtures (M3 step 3–4): `execution.crash-before-dispatch-is-never-delivered`, `execution.crash-during-dispatch-stays-ambiguous-then-reconciled`. | M3 |
| SCN-11 | Shared-subscriber cancellation across executor and context provider | PIO-C §6; REL combined gate | X plus CTX-10. | M4 |
| SCN-12 | A native action wait survives an executor restart | PIO §10 | F: kill during `requires_action`; after restart the action is still pending under the same `action_id`, and answering it once works. EXE-4, EXE-14. | M3 |
| SCN-13 | Executor restart reattaches to a surviving host instead of respawning | PIO §8, §10 | F: kill the provider while the scripted host runs; after restart the execution continues with the same host generation and no second prompt effect. EXE-22. | M3 |
| SCN-14 | Budget exhaustion with unresolved liability | PIO §10; PIO-I §5 | F: invocation count exhausted with one unknown-outcome invocation; further cost-constrained admission is refused while unrelated grants proceed. EXE-16. | M3 |
| SCN-15 | Cancellation refusal and cancellation acknowledgment loss | P §13; PIO §10 | F: script refuses cancel; separately drops the cancel acknowledgment. EXE-8. | M3 |
| CMP-1 | Old caller with a newer provider that adds an optional feature | DEV §5; P §11 | Two manifest versions of R; the old caller's fixtures still pass. | M6 |
| CMP-2 | New caller requiring a feature against an old provider | DEV §5; P §11 | Explicit refusal at negotiation, no partial behavior. | M6 |
| CMP-3 | Major version mismatch | P §11 | Explicit version error on both sides. | M1, M6 |
| CMP-4 | Fixtures are versioned; changed meaning is a new fixture version | P last §; DEV §5 | Fixture schema requires suite version and applicable profile versions. A check refuses an edited fixture without a version change. | M1 |
