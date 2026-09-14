# Execution profile `execution/1` — proposed M3 draft

> **Status: proposed draft for Protocol 0.1 milestone M3.** Nothing here is normative until M3 is accepted together with its schemas, fixtures, mutants and independent evidence. Names marked *candidate* may change during M3. Architecture: [SPEC §5–§8, §12–§14](../SPEC.md). Task: [M3](../../work/release-0.1/M3.md). Requirement IDs refer to the [matrix](../../work/release-0.1/MATRIX.md). Sources: pio `docs/spec/SPEC.md`, `docs/spec/INTERNALS.md` and `docs/spec/STANDALONE-CLIENT.md` at `e65b7c0`; cbr `docs/spec/PREPARATION-AND-DELIVERY.md` at `3278393`.

An **executor** runs work in an existing agent harness and reports what actually happened. PIO is one executor; a third-party minimal executor must be able to implement this profile without PIO, Combraton or any private library (REL-7). A **caller** submits work and reads execution facts. The profile covers identity, admission, delivery proof, runtime state, results and completion receipts, cancellation, reconciliation, and timeouts, plus optional features for steering, native actions, controller leases, workspaces, usage, context bindings, discovery and output telemetry.

The profile reports **execution truth**, not project acceptance. A result, a zero exit or a delivered prompt never establishes that work is correct, adopted or understood (SPEC §5).

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119 and RFC 8174 when in capitals.

## 1. Dependencies and negotiation

- `execution/1` depends on `core/1` with features `core.events`, `core.capabilities` and `core.effects` (CORE §19, M3 draft). `core.grants` is optional; without it only authority principals operate.
- **Required Core features (owner decision, 2026-09-14).** `execution/1` requires `core.events`, `core.capabilities` and `core.effects`. This document is where that requirement is published. Negotiation enforces it:
  - **Required request.** If a required `execution/1` request lacks one of those Core features, negotiation is refused with `unsupported_profile`. `details.unsatisfied` lists one actionable item per missing feature: `{ "profile": "execution", "feature", "reason": "dependency_not_selected" }`.
  - **Optional request.** An optional `execution/1` request without them is not selected. The same items appear in `unselected`, and execution operations are then `profile_not_negotiated`.
  - **Manifest unchanged.** `core.describe` keeps its accepted shape: `depends_on` names only `core`.
  - **Deferred.** Machine-readable advertising of feature dependencies is an explicit M6 compatibility decision (matrix CMP-5). No incompatible manifest field is added before then.
  - **Older participants.** A caller that does not request `execution/1` gets unchanged results. A provider without `execution/1` reports an optional request as `unknown_profile` in `unselected`, and refuses a required one as `unsupported_profile` (CMP-7).
- A provider advertises `execution/1` and the optional features it implements (§12). A caller lists the features it requires; an unsupported required feature is refused at negotiation (CORE §4).
- Every execution operation follows the Core command path (CORE §10): deduplication before authorization, authorization before capabilities, authority epoch and preconditions, then one owner transaction that commits the state change, its events and its effect records.

## 2. Identities

Identities are distinct and never reused for one another (PIO-I §2, SPEC §3).

| Identity | Scope and meaning | Must not be used as |
|---|---|---|
| `execution` subject `{ "kind": "execution.execution", "id" }` | One admitted unit of work. The caller chooses the ID and creates it with precondition revision 0. The kind `execution.execution` follows Core's `<profile>.<kind>` grammar and is used consistently in schemas, events, `kinds` filters, grant resources and fixtures; human-facing labels say "Execution" (owner decision, 2026-09-14). | A retry counter |
| `predecessor` | The execution this one retries or continues. A retry is always a new execution (EXE-1). | Permission to reuse the predecessor's identity, receipts or effects |
| `command_id` | Core command identity. Retransmission reads or completes the prior operation; it never runs work again. | A new attempt |
| `invocation_id` | One executor-visible external invocation inside an execution. Opaque harness-internal calls are outside it and declared as a coverage limit. | Whichever result was selected |
| `delivery_id` | One prompt, steering or context delivery episode. It is a Core effect ID (CORE §19). | A fresh ID per network retry |
| `native` | Harness session and turn references, where the harness exposes them. | Proof of delivery |
| `host` | `{ "id", "generation" }`: an owned host slot and its process generation. | A bare process ID |
| `completion_id` | One immutable result submission (§6). | Permission to overwrite a later attempt |
| `correlation` | Caller-owned opaque metadata, returned unchanged (EXE-20). | A Combraton workflow model |

## 3. Status axes

Execution state is six independent axes. Each axis changes only through an execution event carrying its **evidence class** and source (§9). Later observations append; they never rewrite an earlier observation (EXE-3). A provider MUST NOT manufacture earlier states to make progress look linear (SPEC §5).

| Axis | Values | Establishes |
|---|---|---|
| `admission` | `queued` (with `queue_reason`), `admitted`, `refused` (with `reason` and optional `alternative`) | The executor's capacity and policy decision (EXE-2) |
| `delivery` | `pending`, `acknowledged`, `delivered`, `not_delivered`, `failed_before_delivery`, `ambiguous` (§3.1) | The current determination of whether the brief reached the harness, with its evidence |
| `runtime` | `preparing`, `active`, `requires_action` (with `action_id` and `owner`), `quiescent`, `exited`, `unknown` | Current observed state (EXE-4) |
| `result` | `absent`, `partial`, `returned` | Output availability |
| `exit` | `{ "code" }`, `{ "signal" }`, `forced_termination`, `unavailable` | Process outcome, where meaningful |
| `evaluation` | `not_requested`, or an attributed external reference | A caller or verifier's assessment, never derived from `exit` or `result` (EXE-5) |

**Delivery proof classes** (EXE-3):

| Class | Meaning |
|---|---|
| `provider_ack_id` | The harness returned an acknowledgment identity for this delivery |
| `echo` | The harness echoed the submission; what an echo proves is defined by the adapter's capability predicate |
| `transport_only` | The transport accepted the bytes; the harness said nothing |
| `bytes_written` | Bytes were written to a terminal or stream. Never reported as `acknowledged` |

### 3.1 Delivery determinations

Owner decision, 2026-09-14. Each delivery has a **current determination** and a **history**. The execution's `delivery` axis is the current determination of its delivery.

| Determination | Meaning | Terminal |
|---|---|---|
| `pending` | Still awaiting evidence. Dispatch may not have begun, or it began and no establishing evidence has arrived. | no |
| `acknowledged` | A correlated harness acknowledgment for this delivery (`provider_ack_id`), or an `echo` whose meaning the adapter's evidenced capability contract establishes | yes |
| `delivered` | Later evidence, such as reconciliation, established that the brief reached the harness, without an acknowledgment | yes |
| `not_delivered` | Later evidence established that a dispatched brief did not reach the harness | yes |
| `failed_before_delivery` | Dispatch provably never began and the executor declared the delivery finished. It is never reopened; a later attempt is a new execution. | yes |
| `ambiguous` | Dispatch may have begun and the outcome is unknown, for example after a crash with the dispatch marker written, or when the evidence wait ends | no |

**Rules:**
- **Transport acceptance and terminal writes** (`transport_only`, `bytes_written`) alone establish nothing. They are recorded as evidence while the determination stays `pending`.
- **Waits end.** When the delivery wait ends without establishing evidence, `pending` becomes:
  - `ambiguous` if dispatch began;
  - `failed_before_delivery` if it provably did not.

  A delivery never stays `pending` after its wait ends (§8).
- **Resolution replaces the current view.** When later evidence resolves a delivery, the current determination changes to the resolved value with that evidence, for example `ambiguous` → `delivered`. The previous determination, with its evidence, is appended to `history`, which is never rewritten. The original `delivery.observed` event stays in the event stream. The axis does not stay `ambiguous` merely to preserve history.
- **Resolution never submits.** Reconciliation reads evidence; it never causes another submission.
- **What delivery does not establish.** `acknowledged` and `delivered` are distinct facts, and neither establishes comprehension, compliance or task success.
- **Public record.** `execution.inspect` shows each delivery as `{ delivery_id, delivery, evidence, proof_class?, determined_at, history: [ { delivery, evidence?, recorded_at } ] }`.
- **Effect status.** The delivery effect's status (CORE §19) follows the determination: `acknowledged` and `delivered` give `succeeded`; `not_delivered` and `failed_before_delivery` give `failed`; `ambiguous` gives `unknown`; `pending` gives `pending`.
- **Refused admission** records the execution with no delivery and no effect.

`requires_action` is a runtime condition, not a progress level. `cancel_requested` is not `cancelled` (§7).

## 4. Operations (base profile)

A minimal executor implements these. *Candidate* names.

| Operation | Kind | Semantics |
|---|---|---|
| `execution.submit` | command | Creates the execution (precondition revision 0). Payload: `brief` (digest and media type, or an inline bounded brief), `adapter` requirements, `restrictions` with required enforcement levels (§10), `timeouts` (§8), optional `predecessor`, `correlation`, `budget` (§12) and `context_bindings` (§13). Outcome: execution reference and the admission observation. |
| `execution.inspect` | query | Current axes, receipts, open obligations and an events cursor for this execution. Bounded: large output and transcripts are never inlined (PIO-I §7). |
| `execution.cancel` | command | Records a cancellation request and returns a **request receipt** (`cancel_requested`). The outcome is observed later as `cancelled`, `refused`, `not_supported` or `unknown` (EXE-8). |
| `execution.reconcile` | query | Given a `command_id` or `delivery_id`, returns the scoped observations the executor holds and any open obligations. It MUST NOT submit, resubmit or restart anything (EXE-10). |

**Watching** uses Core subscriptions with `kinds: ["execution.execution"]` (CORE §16). Executor observations appear no later than the provider's next request or idle re-check. Execution events are ordinary Core events; Core cursors, gaps, epochs, filtering and idle-lapse rules apply unchanged (EXE-7).

**Detaching is not cancellation** (EXE-22). Closing a session, disconnecting or restarting a client leaves executions running. A reconnecting caller queries or reconciles; it does not replay a remembered submit as new work.

## 5. Admission

- Admission is decided in the submit's owner transaction: command identity, grant, authority epoch, adapter capability requirements, restriction enforcement, workspace reservation (§11), queue policy and budget (§12).
- A **required restriction** whose enforcement level exceeds what the adapter advertises is `refused` with reason `enforcement_unavailable` and, where one exists, an actionable `alternative`. It is never admitted with weaker enforcement (EXE-2).
- **Missing capability information is `unknown`, and `unknown` is not `supported`** (CORE §17, EXE-11).
- If canonical persistence fails, no external invocation is authorized (PIO-I §3). The caller sees `unavailable`; nothing is bound.

## 6. Completion receipts

- A completion is an immutable result record bound to execution, `invocation_id`, `host` generation, claim token, output descriptors and capture coverage (EXE-6).
- **Duplicate:** the same `completion_id` with the same digest returns the existing receipt.
- **Conflict:** the same `completion_id` with different content is rejected and retained as a conflict observation.
- **Old attempt:** a result from a superseded claim or host generation is retained under its original identity, but it cannot finalize the current execution or its successor.
- Receipt durability, publication elsewhere, effect reconciliation and external verification are separate facts. Publication never implies acceptance.
- **Submission is internal to each executor in Protocol 0.1** (owner decision Q1, 2026-09-14). No public `execution.complete` operation exists.
  - Every completion outcome MUST be observable through `execution.inspect` and `execution.completion.recorded` events: recorded, duplicate, conflict and superseded attempt.
  - An executor MUST NOT require a PIO-specific host, library or storage format to produce or expose receipts.
  - Conformance drives duplicates, conflicts and late old attempts through the scripted executor (§15).

## 7. Cancellation and fencing

- `execution.cancel` commits the request first, then forwards it through the harness's supported mechanism, then observes the outcome.
- A lost cancel acknowledgment is recovered by retransmitting the same command, which returns the same receipt through Core deduplication (EXE-8).
- Escalation to process termination requires verified host ownership and generation; a process number alone is insufficient (PIO §8).
- Cancellation, a stopped process and reconciled external effects are separate observations. Outstanding external effects survive cancellation, timeout and abandonment as open obligations (CORE §19).
- Fences stop stale mediated commands and results from cooperating hosts. They cannot retract an effect already sent by an unrestricted process; that limitation is part of the contract.

### 7.1 Restart recovery

Owner decision, 2026-09-14. After a restart, an accepted execution whose delivery is still `pending` MAY be recovered and dispatched under the **same execution and delivery identity**, but only when the executor can establish that dispatch never began. That requires all of the following:

1. **Write-ahead dispatch marker.** Before any harness write, the executor durably records a dispatch marker for the delivery, bound to the host generation that will send. This is mandatory for every executor.
2. **Intact durable history.** The absence of a marker is proof only if the journal is intact. If the executor cannot vouch for journal continuity, for example after a restore or a new stream epoch (CORE §16.1), a missing marker is not proof: the delivery becomes `ambiguous`.
3. **Fencing.** Recovery advances the host generation. A dispatcher from an older generation MUST NOT send afterwards, and its attempt is recorded (`execution.dispatch.fenced`).
4. **Revalidation before dispatch.** The executor revalidates at the recovery point, and dispatches only if all of these still hold:
   - no cancellation has been requested;
   - the submitter is still authorized (an authority, or an active, unexpired grant bound to a current epoch);
   - no delivery or execution deadline has passed;
   - applicable execution constraints still hold.
5. **Recorded decision.** Every recovery records `execution.recovery.decided` and an entry in the execution's `recovery` list:
   - `{ delivery_id, decision, reason, recorded_at }`;
   - `decision` is `dispatch_resumed`, `ambiguous` or `failed_before_delivery`;
   - `reason` is one of `provably_not_dispatched`, `dispatch_may_have_begun`, `journal_not_intact`, `cancelled`, `authorization_lost`, `deadline_passed` or `recovery_policy`.

**Outcomes:**
- **Marker present.** If dispatch might have begun, the delivery becomes `ambiguous` and is reconciled. It is never resent blindly.
- **Terminal.** A delivery already durably declared `failed_before_delivery` is never reopened.
- **Termination policy.** An executor may terminate provably undispatched work as a declared recovery policy (`recovery_policy`). That policy is allowed, not required by the Protocol.


## 8. Timeouts

Five distinct timeouts, each with its own event when it passes (EXE-19):

| Timeout | Closes |
|---|---|
| `queue` | Waiting for admission |
| `delivery` | Waiting for delivery evidence |
| `execution_deadline` | The permitted wall-clock execution window |
| `inactivity` | Waiting for any runtime observation |
| `reconciliation` | Waiting for an open obligation to be resolved |

When the `delivery` timeout passes, the evidence wait ends: a `pending` delivery becomes `ambiguous` if dispatch began, or `failed_before_delivery` if it provably did not (§3.1). A timeout closes a permitted wait or triggers a control action. It never proves that remote work ended, never refunds unresolved liability (§12), and never marks an effect as not having happened (CORE §19).

Timeouts are evaluated against the provider clock. Conformance controls that clock from the environment (§15).

## 9. Execution events

Execution events use Core event records (CORE §16.2) with subject `{ "kind": "execution.execution", "id" }`. *Candidate* types:

| Type | Payload |
|---|---|
| `execution.admission.changed` | `{ "admission", "reason"?, "alternative"? }` |
| `execution.delivery.observed` | `{ "delivery_id", "delivery", "proof_class", "evidence" }` |
| `execution.delivery.reconciled` | `{ "delivery_id", "outcome": "delivered" \| "not_delivered" \| "unknown", "delivery", "evidence" }`; `delivery` is the resulting current determination |
| `execution.recovery.decided` | `{ "delivery_id", "decision", "reason", "host" }` (§7.1) |
| `execution.dispatch.fenced` | `{ "delivery_id", "generation", "current_generation" }`: an older dispatcher was refused (§7.1) |
| `execution.runtime.changed` | `{ "runtime", "action_id"?, "owner"? }` |
| `execution.result.changed` | `{ "result" }` |
| `execution.exit.observed` | `{ "exit" }` |
| `execution.completion.recorded` | `{ "completion_id", "digest", "invocation_id", "host", "status": "recorded" \| "duplicate" \| "conflict" \| "superseded_attempt" }` |
| `execution.cancel.requested` / `execution.cancel.observed` | `{ "receipt" }` / `{ "outcome" }` |
| `execution.timeout.passed` | `{ "timeout" }` |
| `execution.host.changed` | `{ "host" }`, when the host generation changes |
| `core.effect.obligation.overdue` | `{ "effect", "obligation" }`, on the execution subject whose effect it is |

`origin` is `command` for events caused by a caller command, and `provider` for observations from the harness or host (CORE §16.2). Provider-origin events never carry a `command_id`.

## 10. Restrictions and enforcement

- Every restriction names a scope and a required **enforcement level**: `enforced` (OS or sandbox), `mediated` (tool boundary) or `cooperative` (instruction only). These are the same three levels as CORE §17 (SPEC §7).
- Adapter capability predicates reuse CORE §17 and add `adapter_version`, `harness_version`, `protocol_version`, `probed_at`, `limits` and `enforcement` (EXE-11).
- A worktree is not a sandbox. An executor MUST NOT claim `enforced` for a restriction that only instructions implement.

## 11. Optional features

| Feature | Operations and rules | Rows |
|---|---|---|
| `execution.steering` | `execution.steer` returns a request receipt. Recorded request, native delivery acknowledgment and later observed behavior are three separate facts. Without live steering, the refusal is explicit: `not_supported` with a supported alternative. Delivery never proves comprehension. | EXE-9 |
| `execution.actions` | `execution.respond_action` answers a native action request. `action_id` belongs to one execution; answering it in another is `not_found` and authorizes nothing. | EXE-14 |
| *(base rights)* | Under a grant, `execution.submit` and `execution.cancel` cover the execution subject; `execution.read` covers `execution.inspect`, `execution.reconcile`, execution events and the execution's effects through `core.effects.get`. | CORE-11, CMP-6 |
| `execution.controller` | `execution.controller.claim` takes the single mutating controller lease for a host. It advances the Core authority epoch of scope `execution.controller:<host id>`, and mutating execution commands carry that epoch. A stale controller is refused with `stale_authority_epoch`. Read-only observers are unaffected. | EXE-13 |
| `execution.workspaces` | Workspace lease descriptor: repository, base, writer identity, lease epoch, permitted paths and effects, cleanup policy. `execution.workspace.checkpoint` records coverage the executor probed itself, including dirty and untracked state. Agent-reported commit IDs are annotations, not receipts. | EXE-15 |
| `execution.usage` | Usage observations with a basis of `observed`, `estimated`, `unknown` or `enforced_bound`. Liability for an unknown outcome is unresolved and is not refunded by a timeout. A hard ceiling the adapter cannot enforce is refused at admission. | EXE-16 |
| `execution.context` | Context bindings at submit and delivery observations for packets and updates (§13). | EXE-17, EXE-18 |
| `execution.discovery` | Optional, executor-neutral (owner decision Q2). `execution.discovery.list` (*candidate*) returns installations and endpoints with separate facts: detected, adapter recognized, version supported, authentication known or unknown, reachable, last verified. Detection is never offered as usable capability. Schemas plus positive and adversarial fixtures use scripted installations. Probing real installations is PIO work. | EXE-12 |
| `execution.output` | Output telemetry, spooled separately from semantic events (§14). | OBS-7, TRN-4 |
| `execution.continuation` | Native resume and fork where supported. Without native support, a continuation is labeled `fresh_continuation`, never `resumed`. | EXE-23 |

## 12. Usage and liability

Usage is recorded once per invocation with its basis (§11, `execution.usage`). Admission considers settled consumption, active reservations and unresolved liability. A reservation is released only with durable evidence that the invocation was never dispatched and can no longer be dispatched under its identity; a crash, timeout or lease expiry is not such evidence (PIO-I §5).

## 13. Context bindings and delivery observations

The Context profile is M4. M3 defines only the binding and the delivery observation, so that an executor can enforce and report them.

- **Binding at submit:** packet or update reference, exact digest, obligation (`advisory`, `required_before_start`, `required_before_transition` with the transition named), and the authority that selected it.
- **Admission:** an execution whose `required_before_start` binding is unsatisfied is not admitted as ready. Advisory bindings add no startup barrier, and a missing advisory item stays visible as a gap (EXE-17).
- **Delivery observation:** exact digest, target execution and native session, boundary (initial prompt, supported turn steering, explicit context request), and outcome: `queued`, `delivered`, `acknowledged`, `late`, `unavailable` or `unknown`. A packet arriving after its dependent boundary is `late` (EXE-18). No observation proves comprehension.

## 14. Output telemetry and backpressure

- **Separate channel.** Output chunks travel in a telemetry channel separate from semantic events, read with `execution.output.read` (*candidate*) using byte-offset cursors.
- **Explicit telemetry loss.** A lost range records `from` and `to` offsets, the byte count where known, the reason, and its effect on evidence coverage (OBS-7). Telemetry may be coalesced or discarded only with declared lost ranges.
- **Semantic events are never dropped from the stream.** Backpressure ends a *delivery connection*; it never skips an event (TRN-4). The rules are in CORE §16.5 "Backpressure" (proposed M3 draft):
  - memory and pending notification bytes are bounded per connection;
  - an ending notice `consumer_too_slow` is attempted only for consumers that negotiated `core.events.backpressure`, and only within a bounded write time;
  - the connection is then closed;
  - recovery is by reconnecting from the consumer's last durably processed cursor, with retention gaps reported explicitly.
- **Separate reporting.** The telemetry policy and the semantic-event policy are negotiated and reported separately.

## 15. Conformance and test controls

Execution fixtures need harness behavior, faults and time that ordinary operations cannot produce on demand. All such control stays outside the production protocol, extending CORE §13.1 ([decision 007](../../decisions/007-execution-test-controls.md), accepted with refinements).

- **Scripted executor.**
  - The reference executor drives a scripted fake harness selected by launch configuration: acknowledge, echo only, write bytes only, crash after write, never exit, return late, refuse cancellation, request an action, exceed an output spool.
  - The script vocabulary is normative **only for the conformance tests that use it** (owner decision Q4). A production executor may satisfy it through a test adapter; it does not need a production scripting engine.
  - Real harness adapters are PIO's work and PIO's evidence.
- **Controllable clock.**
  - Launch configuration can name a clock file that the runner replaces atomically. The provider takes its protocol-visible time from it: expiry, timeouts, deadlines, `recorded_at`.
  - Virtual time never moves backward. A missing or malformed file at start refuses the launch; during a run, the provider keeps its last good instant.
  - Real-time watchdogs in the runner and in any barrier stay independent of it, so a frozen clock cannot hang a suite.
- **Process faults.** The runner kills the provider at points the script or a barrier reaches, then restarts it over the same data directory.
- **Barriers, implementation-specific.**
  - A participant may declare named pause points. The runner waits for a point to be reached, synchronizes on explicit provider signals, and releases it. That gives a deterministic interleaving for concurrency regressions, with bounded failure handling instead of sleeps.
  - Fixtures that use barriers apply only to participants declaring those names. Others report `unsupported`, recorded as a coverage limit and never as a pass.

No execution operation, method name, event type or error is reserved for testing. A provider launched without a conformance launch configuration exposes none of these controls.

## 16. What execution does not establish

An execution fact is the executor's observation with its stated evidence. It does not establish:
- correctness, project acceptance or comprehension;
- that a cancelled execution's external effects stopped;
- that a timeout ended remote work;
- that usage without an enforced bound is complete;
- that an enforcement level holds beyond what the adapter actually enforces.
