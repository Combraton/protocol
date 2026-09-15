# Context profile `context/1` — proposed M4 draft

> **Status: proposed draft for Protocol 0.1 milestone M4.** Nothing here is normative until M4 is accepted together with its schemas, fixtures, mutants and independent evidence. Names marked *candidate* may change during M4. Architecture: [SPEC §2, §8, §9, §14](../SPEC.md). Task: [M4](../../work/release-0.1/M4.md). Requirement IDs refer to the [matrix](../../work/release-0.1/MATRIX.md). Sources: cbr `docs/spec/PREPARATION-AND-DELIVERY.md` and `docs/spec/SPEC.md` §7, §8 at `3278393`; pio `docs/spec/STANDALONE-CLIENT.md` §5, §6 at `e65b7c0`. Owner decisions M4-Q2, M4-Q3 and M4-Q4 (2026-09-15) are incorporated.

A **context provider** prepares exact, bounded context for a named consumer and reports what it could and could not supply. CBR is one context provider; any service that compiles cited context can implement this profile. A **caller** asks for context on behalf of work it will run. A **consumer** is the execution or harness the context is for.

The profile carries **exact packets and explicit gaps**, not understanding. A delivered packet never establishes comprehension, a static explanation never establishes runtime behavior, and a coverage frontier is never a simultaneous global snapshot (SPEC §14).

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119 and RFC 8174 when in capitals.

## 1. Dependencies and negotiation

- `context/1` depends on `core/1` with `core.events`, and on `evidence/1` as a **protocol dependency**: every packet revision is an exact sealed Evidence artifact (§5), so its identity, integrity, retention and fetch reuse EVIDENCE (owner decision M4-Q2). This does not require a separate running service: a standalone context provider MAY implement `evidence/1` for its own packets.
- **Timing semantics are negotiated explicitly** (SPEC §14): features `context.advisory`, `context.required_before_start` and `context.required_before_transition`. A request using an obligation whose feature was not negotiated is refused with `unsupported_required_feature` (CTX-9).
- Other optional features (*candidate*): `context.shared_jobs` (§4), `context.updates` (§8), `context.expand` (§6).
- A provider that cannot meet a requested boundary or delivery semantics refuses the request explicitly; it never silently degrades a required obligation (CTX-9).

## 2. Identities

| Identity | Scope and meaning | Must not be used as |
|---|---|---|
| Request subject `{ "kind": "context.request", "id" }` | One caller's request for context. The caller chooses the ID before any execution exists (CTX-1). | An execution identity or a job identity |
| Job `{ "kind": "context.job", "id" }` | The provider's preparation work. Several compatible requests may share one job (§4). | A request; cancelling it is not cancelling a request |
| Packet `{ "kind": "context.packet", "id" }` with `revision` | One immutable result revision for a request | A mutable document |
| Packet reference `{ packet, revision, artifact: { provider, artifact, digest } }` | How Execution context bindings cite a packet (EXECUTION §13). `artifact` is the exact Evidence reference of that revision's sealed bytes and the evidence provider holding them. | A reference without the artifact and digest |
| Item `item_id` | One required or advisory item within a request | A claim identity (Knowledge is M5) |

When preparation leads to admission, the execution's context binding names the request and the packet reference, so the causal link from request to execution is preserved (CTX-1).

## 3. Requests

`context.request.submit` (command, *candidate*) creates the request (precondition revision 0). Payload:

| Member | Meaning |
|---|---|
| `consumer` | `{ task, principal, executor? }`: who the context is for |
| `basis` | Target basis manifest (CTX-2): `repositories: [ { id, tree, dirty: { snapshot_digest } \| null } ]`, `environment`, `configuration`, `build`, and `completeness`. A commit-only basis for a workspace with uncommitted changes MUST declare `completeness: "partial"`. |
| `items` | Required and advisory items (CTX-3): `{ item_id, selector, obligation: "advisory" \| "required_before_start" \| "required_before_transition", transition?, reliance: "binding" \| "evidence" \| "hypothesis" \| "reference", selected_by, check }`. `selected_by` names the authority that chose the item. `check` states how satisfaction is decided, for example exact inclusion of a named source revision. "Understand the repository" is not a checkable item. |
| `fallback` | For advisory items only: `proceed_with_gap` or `wait_until_deadline` (CTX-7) |
| `limits` | Three separate limits (CTX-4): `deadline` (provider clock), `investigation` budget `{ units, amount }` for the provider's own work, and `output_capacity` `{ units, amount }` for the packet |
| `authority_content` | Optional binding content supplied directly by an authority: `{ item_id, evidence: reference, authority_revision }` (CTX-14) |
| `origin` | Optional, for requests made by a job's own investigation: `{ initiator, depth, call_budget }` (CTX-19) |

Outcome: `{ request, state, job }`. Request states: `preparing`, `ready`, `partial`, `unmet`, `refused`, `cancelled`.

- An unknown obligation, reliance label or check kind is `invalid_envelope`; an obligation whose feature was not negotiated is `unsupported_required_feature`.
- A request whose mandatory items cannot fit `output_capacity` ends `refused` with reason `budget_insufficient` and the size the mandatory items need (CTX-8). A provider MUST NOT drop mandatory items to fit.

## 4. Jobs, subscribers and cancellation

- A provider MAY attach a request to an existing compatible job: same basis, compatible access scope and compatible items. Each attached request is a separate subscriber (`context.shared_jobs`).
- `context.request.cancel` (command) cancels **that request only**. The job continues while any other request still needs it (CTX-10). Cancellation of the job itself is a provider decision reported as `context.job.ended` with its reason, for example an exhausted investigation budget.
- A consumer waiting for context MUST NOT be required to hold an execution reservation (CTX-18). If preparation needs an execution, the provider submits it as an ordinary, separately authorized caller, with `origin` naming the job, its depth and call budget. Such an execution is never automatically re-enriched through the same path (CTX-19).

## 5. Results and packets

When a request reaches `ready`, `partial` or `unmet`, the provider publishes a packet revision (revision 1, or a later one under §8) with these result facts:

| Member | Meaning |
|---|---|
| `packet` | The packet reference (§2) |
| `selected` | Selected source revisions and artifact references (CTX-5) |
| `provenance` | Compiler and job identity, and the input manifest |
| `inclusions` / `omissions` | What was included, and what was omitted with the reason (budget, authorization, applicability, unavailability) |
| `citations` | For each cited section, the Evidence references it rests on |
| `labels` | Per section: `binding`, `observation`, `hypothesis`, `unknown`, `declared_requirement`, `source_inspected`, `runtime_observed`, `inferred`, `stale` (CTX-20). A section without a label is invalid. |
| `coverage` | Per-producer frontiers and gaps: `[ { producer, frontier, gaps } ]` (CTX-6). There is no global cursor. |
| `items` | Each item's result: `satisfied`, `unmet` (with reason), or `degraded` for advisory items under their fallback |
| `applicability` | The basis the packet is valid for, and invalidation conditions (CTX-15) |
| `authority` | For authority-supplied content: the authority revision included and the coverage it rests on (CTX-14) |

- **Packet-to-artifact mapping** (M4-Q2). Each packet revision maps immutably to exactly one sealed Evidence artifact, named by provider, artifact and digest. The mapping is recorded when the revision is published and never changes. A later revision maps to a different artifact.
- **Exact bytes** (CTX-5). The complete packet bytes are the sealed artifact's bytes, fetched with `evidence.fetch` at the named provider. A provider MUST NOT regenerate them.
- **Missing required items stay unmet** (CTX-7). When the deadline passes, every required item not supplied is `unmet`. It is never reported `satisfied`, and it is never downgraded to advisory. Advisory items follow their declared fallback.
- **No unobserved claims** (CTX-14). Coverage frontiers name only what the provider actually ingested. A packet MUST NOT claim a frontier beyond it.

## 6. Inspect and expand

| Operation | Kind | Semantics |
|---|---|---|
| `context.request.inspect` | query | Request state, job, packet references and item results |
| `context.packet.inspect` | query | `{ packet, revision }` → the result facts of §5 and an **excerpt**: `{ offset, data_base64, length, complete }`, bounded, with omission markers (CTX-11). An excerpt is never the complete sealed bytes: it carries no digest of its own, and it is never presented under the artifact's digest. Complete bytes come from `evidence.fetch`. |
| `context.expand` | query | `{ packet, revision, citation }` → cited content the principal is **newly** authorized to read under its grant. Content outside the grant is refused with `permission_denied`, indistinguishable from nonexistent content (CTX-11, CORE-12). |

## 7. Direct fetch by an executor

Grants are provider-local (CORE §15.2): a grant's audience is the provider that issued it. A grant at the context provider therefore never authorizes a read at a different evidence provider (owner decision M4-Q3). For an executor to read a packet directly (CTX-16), the caller arranges **one grant per audience**, each held by the executor's principal and bound to the exact object:

| Audience | Right | Resource |
|---|---|---|
| The context provider | `context.packet.read` | `{ kind: "context.packet", id }` of that packet |
| The evidence provider named in the packet's artifact reference | `evidence.read` | `{ kind: "evidence.artifact", id }` of that revision's artifact |

- The executor reads the result facts at the context provider under the first grant, then fetches the bytes at the evidence provider under the second. It checks the fetched digest against the binding.
- When the context provider implements `evidence/1` for its own packets, both grants are issued there.
- A grant for another packet or artifact, a grant presented at the other provider, or a grant held by another principal is refused. Possessing a packet reference authorizes nothing.

## 8. Updates, corrections and staleness

- **Updates are new revisions** (CTX-12, `context.updates`). A finding during work is published as a new packet revision with its own basis, digest and `supersedes: { revision }`. Earlier revisions stay retrievable, and old bytes are never relabeled current. Delivery of an update to a running execution is an Execution observation (EXECUTION §13); a late update is recorded `late` (CTX-17).
- **Correction during preparation** (CTX-13). When an authority supplies a correction to a binding item while a job runs, a derivation made from the older binding MUST NOT become the current result for that item. The provider rebuilds within budget, marks the older result `historical`, or refuses current reliance for that item.
- **Stale basis** (CTX-15, owner decision M4-Q4). A packet's `applicability` carries typed, checkable **conditions**, for example `{ condition_id, kind: "repository_tree", repository, expected }`, `dirty_snapshot`, `environment_digest` or `authority_revision`. An executor bound to the packet checks, at each relevant boundary, only the conditions it can actually observe, and reports each as `match`, `mismatch` or `unavailable` (EXECUTION §13.1). The executor decides nothing about semantic truth and is not an applicability engine: conditions it cannot observe are `unavailable`, which is neither fresh nor necessarily stale.

## 9. Provider unavailability

- A consumer whose context provider is unavailable proceeds according to its obligations (SCN-8): advisory bindings proceed with a visible gap, and required bindings keep the work from being admitted as ready until they are satisfied, the deadline passes (`unmet`), or the authorized caller changes the requirement.
- A provider outage never turns a required item into `satisfied`.

## 10. Rights

| Right | Covers |
|---|---|
| `context.request` | `context.request.submit` and `context.request.cancel` on covered requests |
| `context.read` | `context.request.inspect`, events of covered requests |
| `context.packet.read` | `context.packet.inspect` and `context.expand` on covered packets |

Expanded content and packet bytes are additionally subject to the Evidence rights of their artifacts.

## 11. Events

Context events use Core event records. *Candidate* types:

| Type | Subject | Payload |
|---|---|---|
| `context.request.changed` | request | `{ state, job?, reason? }` |
| `context.packet.published` | request | `{ packet reference, items }` |
| `context.job.ended` | job | `{ reason }` |
| `context.request.cancelled` | request | `{ job_continues }` |

## 12. Conformance and test controls

- **Reference participants only.** A reference context provider drives a **scripted preparation** selected by launch configuration: packet content per request, delays against the clock file, coverage frontiers, unmet items, mid-job corrections, budget exhaustion and outages. The script is test environment, normative only for its conformance tests (as EXECUTION §15, owner decision Q4). No fixture depends on CBR's memory engine.
- **Separate participants** (owner decision M4-Q3). Cross-profile scenarios (SCN-1, SCN-4 to SCN-6, SCN-8, SCN-9, SCN-11) run separate reference participants, one per role: context provider, evidence provider and executor, each with its own isolated store. They talk only over public protocol connections, authenticated as their own principals and authorized by grants. A shared executable is acceptable; shared stores and private calls are not.

## 13. What context does not establish

A packet establishes which exact bytes, with which labels, coverage and gaps, the provider produced for a request. It does not establish:
- that the consumer read, understood or followed it;
- that a labeled observation still holds after its basis changed;
- that coverage frontiers from different producers describe one moment;
- project acceptance of any claim; Knowledge reliance is M5.
