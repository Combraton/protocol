# Knowledge profile `knowledge/1` — proposed M5 draft

> **Status: proposed draft for Protocol 0.1 milestone M5.** Nothing here is normative until M5 is accepted together with its schemas, fixtures, mutants and independent evidence. Names marked *candidate* may change. Choices that need an owner decision before implementation are marked with their question number (M5-Q1 to M5-Q9, [M5](../../work/release-0.1/M5.md#proposed-decisions)). Architecture: [SPEC §2, §8, §9](../SPEC.md). Requirement IDs refer to the [matrix](../../work/release-0.1/MATRIX.md). Sources: cbr `docs/spec/SPEC.md` §3–§6, §9–§11 and `docs/spec/INTERNALS.md` §1–§3 at `3278393`; combraton `docs/architecture/MODEL.md` §3–§5 at `9af69ce`.

A **knowledge provider** keeps scoped claims with their lineage, support, conflicts, applicability evaluations and reliance decisions. CBR is one knowledge provider; any service that keeps challengeable claims can implement this profile. A **producer** proposes and revises claims. An **authority** records reliance decisions for a bound scope. A **reader** inspects claims and their history.

The profile keeps **separate records for separate questions**:
- evidence says which bytes were received (EVIDENCE);
- a claim revision says what someone proposed, on what support;
- a reliance decision says what an authority permits the claim to be used for;
- an applicability evaluation says how a revision relates to one target basis.

None of these is derived from another, and none is stored as one status field (KNW-3).

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119 and RFC 8174 when in capitals.

## 1. Dependencies and negotiation

- `knowledge/1` depends on `core/1` with `core.events`, and on `evidence/1` as a protocol dependency: support is cited by exact Evidence reference (§5). As for Context, this does not require a separate running service, and negotiating `knowledge/1` does not require `evidence/1` in the same session.
- Required Core features are published here and enforced at negotiation, as for Execution and Evidence: a required `knowledge/1` request without `core.events` is refused with `unsupported_profile` and a `dependency_not_selected` item. `core.grants` is optional; without it only authority principals operate, and deciding still needs the authority binding (§6).
- Optional features (*candidate*): `knowledge.conflicts` (§7), `knowledge.applicability` (§8), `knowledge.history` (§10).
- Every command follows the Core command path (CORE §10). A claim revision, a decision or an evaluation commits in one owner transaction with its events.

## 2. Identities

| Identity | Scope and meaning | Must not be used as |
|---|---|---|
| Claim `{ "kind": "knowledge.claim", "id" }` | One stable claim lineage at one provider. The producer chooses the ID. | A statement's content, or a revision |
| Claim revision reference `{ provider?, claim, revision, digest }` | One immutable revision. `digest` is the canonical digest (ENCODING) of the revision content (§3). References carried to other participants always name `provider`. | A reference to "the latest" revision |
| Decision `{ "kind": "knowledge.decision", "id" }` | One immutable reliance decision about one claim revision (§6) | A property of the claim |
| Evaluation `{ "kind": "knowledge.evaluation", "id" }` | One immutable applicability evaluation of one revision against one target basis (§8) | A current status of the claim |
| Conflict `{ "kind": "knowledge.conflict", "id" }` | One conflict or drift record over named revisions (§7) | A deletion or a winner |
| Authority binding `{ "kind": "knowledge.authority", "id" }` | The single accepting authority for one scope, with its epoch (§6, M5-Q3) | A grant |

- **Revisions are immutable.** A revision's content never changes once proposed. Supersession, rejection, challenge and corrected validity are later records; they never edit the revision (KNW-1, KNW-8).
- **References grant nothing.** Possessing a claim reference authorizes no read, as for evidence.

## 3. Claim revisions (M5-Q1)

`knowledge.claim.propose` creates revision 1; `knowledge.claim.revise` creates the next revision. The revision content:

| Member | Meaning |
|---|---|
| `plane` | `normative` (a decision or requirement: what should hold), `observed` (what an observation reports), or `interpretive` (an explanation or inference). Planes stay distinguishable (KNW-2). An unknown plane is `invalid_envelope`. |
| `statement` | `{ subject: { kind, id }, predicate, value }`: structured where the claim is structured. `value` is bounded canonical JSON. Prose belongs in `content`. |
| `content` | Optional Evidence reference to longer content, for example a design section. The claim does not copy it. |
| `scope` | `{ id, qualifiers }`: the scope the claim is about. `id` names a scope known to the provider; `qualifiers` narrow it, for example a repository, branch or environment. |
| `validity` | `{ from?, until? }`: when the claim is valid or was observed, where known. Unknown stays absent; it is never filled from the recorded time. |
| `basis` | The target anchors the claim concerns: `repositories: [ { id, tree } ]`, optional `environment` and `build`, as in a context request basis |
| `support` | `[ { support_id, evidence: { provider, artifact, digest }, lineage } ]` (§5) |
| `derivation` | `{ kind: "deterministic" \| "model_assisted" \| "human", record?: evidence reference, inputs: [ references ] }`. A `model_assisted` derivation names its recorded inputs and output record; replay uses the record and never calls a model. |
| `dependencies` | `[ { claim, revision, digest } ]`: revisions this claim's applicability depends on (§8) |
| `conditions` | Typed applicability conditions, reusing the Context condition kinds (`repository_tree`, `dirty_snapshot`, `environment_digest`, `authority_revision`, CONTEXT §8) |
| `health` | Only for `observed` claims about a runtime subject: `healthy`, `degraded`, `failing` or `unknown`. Every other claim reports `not_applicable`. It is what the observation reports, not a judgment of the claim. |
| `supersedes` | On revise: `{ revision, digest }`, which must be the lineage's current revision |

- **Base check** (KNW-1). `revise` carries a Core precondition on the claim subject's current revision, and `supersedes` must name that revision and its digest. A stale base is `precondition_failed`; a matching revision with a different digest is `invalid_envelope` at `/payload/supersedes/digest`. A provider MUST NOT overwrite a revision or merge concurrent revisions silently.
- **Producer.** The producer is the session principal of the command, as for Evidence. A revision naming another producer is `invalid_envelope`.
- **New revisions start unaccepted.** No member of `propose` or `revise` sets reliance. A new revision's reliance is `proposed` until a decision names it (§6). This also covers extracted instructions: a `normative` claim a model derived from a transcript is still only `proposed` (KNW-9).

Outcome of both commands: `{ reference }`, the new revision's reference.

## 4. Independent states (KNW-3)

`knowledge.claim.inspect` `{ claim, revision? }` returns the revision content, its reference, and four independent facets. Each facet names the record it comes from, so any cached projection can be rebuilt:

| Facet | Values | Comes from |
|---|---|---|
| `reliance` | `proposed`, `accepted_for_use`, `rejected`, `superseded` | The latest decision for this revision in the scope (§6), or `proposed` when none. `superseded` means an authority decision replaced it, not merely that a newer revision exists. |
| `applicability` | `[ { evaluation, target, result } ]` | Evaluations of this revision (§8). There is no applicability without a target. |
| `health` | `healthy`, `degraded`, `failing`, `unknown`, `not_applicable` | The revision content (§3) |
| `availability` | `complete`, `partial`, `unavailable`, `purged` | The support evidence, as the reader may observe it at the read (§5) |

It also reports `current_revision` of the lineage and `open_conflicts` naming this revision. A revision that is `accepted_for_use` and `invalid_for_target` for some target reports both. A historically accepted claim whose evidence was purged stays `accepted_for_use` with availability `purged`. A provider MUST NOT collapse these facets into one status.

## 5. Support and lineage (KNW-4, M5-Q2)

- Each support entry cites exact evidence. `lineage` names the roots this support derives from: `[ { kind: "evidence", digest } | { kind: "claim", claim } | { kind: "derivation", record } ]`. A support entry with no declared lineage has its own evidence digest as its only root.
- **Support class** is reported at inspect as `support: { class, roots }`:
  - `unsupported`: no support entries;
  - `single_lineage`: every entry shares at least one root;
  - `multiple_lineages`: at least two entries have no root in common.
- **Repetition is not corroboration.** Proposing the same statement again with support that shares a root never raises the class. `multiple_lineages` says only that the declared lineages do not overlap. It is not proof of independence, and the provider does not call it independent.
- **Availability** is `complete` when every support artifact is available to the reader, `partial` when some are, `unavailable` when none are, and `purged` when every one was purged. Support the reader may not read counts as unavailable to that reader and is not named.

## 6. Authority bindings and reliance decisions (KNW-6, M5-Q3)

- **Binding.** `knowledge.authority.bind` (command, provider authority principals only) creates the binding for one scope: `{ scope, authority: principal, epoch }`. There is at most one binding per scope. Binding a scope that already has one is refused with `authority_scope_bound`. Changing the authority is an explicit transfer that raises the epoch (CORE §8). A decision command carrying an older epoch is `stale_authority_epoch`. Decisions recorded under an earlier epoch stay in effect until the current authority records a later decision about the same revision; a transfer does not reset reliance (M5-Q3 alternative: a transfer returns every revision in the scope to `proposed`).
- **Decision.** `knowledge.decision.record` (command) records:

  | Member | Meaning |
  |---|---|
  | `claim` | The exact revision reference |
  | `decision` | `accepted_for_use`, `rejected` or `superseded` |
  | `permitted_use` | For `accepted_for_use`: `binding`, `evidence`, `hypothesis` or `reference`, the reliance labels a packet may carry (CONTEXT §3) |
  | `authority` | `{ binding, epoch }`, checked as a Core authority epoch |
  | `validation_basis` | What the decision rests on: evidence and receipt references, target basis, policy reference |
  | `rationale` | Free text |

- **Only the bound authority decides.** The session principal must be the scope's bound authority at the current epoch. Anyone else is `permission_denied` with reason `not_authority`, including a principal holding every Knowledge right. A grant never makes a principal an authority.
- **No self-acceptance.** The producer of a revision MUST NOT record a decision about it; this is `permission_denied` with reason `self_decision` (M5-Q3 alternative: allow it for `human` derivations and mark the decision `self_stated`).
- **Nothing else decides.** Producing a claim, deriving it, opening a conflict, receiving a passing verification receipt (VERIFICATION §9), or appearing in several summaries never records a decision.
- Decisions are immutable. A later decision about the same revision supersedes it for current reliance; both stay in history.

## 7. Conflicts and drift (KNW-2, KNW-5; `knowledge.conflicts`)

- `knowledge.conflict.open` (command) names two or more revisions. They must share the statement's `subject`, `predicate` and scope `id`, and have overlapping or unknown validity. Otherwise the request is refused with `claims_not_comparable`. Similar wording across unrelated subjects is not a conflict.
- **Conflict or drift.** Revisions in the same plane with different `value` form a `conflict`. A `normative` revision against an `observed` one forms `drift`: "must use queue v2" against "currently uses queue v1" is a gap, not a contradiction. Neither record deletes, rewrites or demotes a revision.
- **Competing values stay visible.** While a conflict is open, its selected slot holds every competing value. `knowledge.claim.inspect` lists the conflict for each involved revision.
- **Resolution needs authority.** `knowledge.conflict.resolve` (command) records `{ resolution: "select" | "narrow_scope" | "reject_support" | "supersede" | "request_observation", selected?, rationale }` and is permitted only to the scope's bound authority (`not_authority` otherwise). Any principal with `knowledge.propose` may open a conflict, including a model derivation, and may attach a suggested distinguishing observation. None of them can resolve it. Recency and producer confidence never resolve it.

## 8. Applicability (KNW-7, M5-Q4; `knowledge.applicability`)

`knowledge.applicability.evaluate` (command) evaluates one revision against a target basis and records an evaluation:

| Member | Meaning |
|---|---|
| `claim` | The exact revision reference |
| `target` | A basis manifest, as in a context request |
| `evaluator` | `{ id, version }`, pinned in the record (SCN-7) |
| `result` | `applicable`, `needs_check`, `invalid_for_target` or `unknown` |
| `coverage` | `{ completeness: "complete" \| "partial" \| "unknown", missing: [ … ] }`: which conditions and dependencies the evaluator could check against the target |
| `causes` | `[ { condition_id? , dependency?, finding: "match" \| "mismatch" \| "unavailable" } ]` |

- `applicable` requires complete coverage: every condition is `match` against the target, and every dependency is itself `applicable` for the same target.
- `invalid_for_target` requires a `mismatch` that the evaluator actually observed.
- `needs_check`: the target carries the relevant anchors, but a condition or dependency could not be checked, for example a dependency with no evaluation for this target.
- `unknown`: the target lacks the anchors the conditions need, or the evaluator cannot evaluate the claim's condition kinds.
- An unavailable input never compares equal to an earlier one. Incomplete coverage MUST NOT yield `applicable`.
- A cycle among dependencies is not support for itself. The provider reports `needs_check` naming the cycle.

Evaluations never change reliance, and reliance never changes an evaluation.

## 9. Inspect, history and query

| Operation | Kind | Semantics |
|---|---|---|
| `knowledge.claim.inspect` | query | §4 |
| `knowledge.claim.history` | query | `{ claim }` → every revision reference in order, every decision, evaluation and conflict naming the lineage, each with its recorded stream position (KNW-8). A superseded revision stays retrievable with its content and facets. |
| `knowledge.claim.query` | query | `{ scope?, subject?, predicate?, plane?, cursor?, limit }` → revision references the reader may read, in claim ID order, with `filtered` as for Evidence query. It reveals no existence (CORE-12). |

- **History and present knowledge are different questions** (MODEL §3). `history` returns what was recorded, in recorded order. Reconstructing what was known at a past stream position, and what is now known about a past validity interval, are candidates for `knowledge.history` (M5-Q8).

## 10. Rights

| Right | Covers |
|---|---|
| `knowledge.propose` | `propose`, `revise`, `conflict.open` and `applicability.evaluate` on covered claims |
| `knowledge.read` | `inspect`, `history`, `query` and events of covered claims and their records |
| `knowledge.decide` | Required, with authority binding, for `decision.record` and `conflict.resolve` |

A right is necessary but never sufficient to decide: deciding also requires being the bound authority (§6).

## 11. Events

*Candidate* types, as Core event records: `knowledge.claim.revised` `{ reference }`, `knowledge.decision.recorded` `{ decision, claim, decision_value }`, `knowledge.evaluation.recorded` `{ evaluation, claim, result }`, `knowledge.conflict.opened` / `knowledge.conflict.resolved` `{ conflict, kind, revisions }`, `knowledge.authority.bound` `{ scope, epoch }`.

## 12. Claims in context packets

A packet that carries claims preserves their identity, reliance and applicability (SPEC §2). The Context-side extension is proposed in [CONTEXT §14](CONTEXT.md#14-claims-in-packets-proposed-m5) (feature `context.claims`, M5-Q7).

## 13. Conformance and test controls

- **Reference participants only.** A reference knowledge provider keeps claims in its own store and evaluates applicability over typed conditions and declared dependencies only. No fixture depends on CBR's memory engine, model runtime or retrieval.
- **Evaluator control** (launch configuration `knowledge`, *candidate*): the evaluator `id` and `version`, and the condition kinds it can evaluate. This lets fixtures show `unknown` for kinds it cannot evaluate, and a version change across restart (SCN-7).
- Support availability is read from the reference evidence provider's store controls (EVIDENCE §14).

## 14. What knowledge does not establish

A claim revision establishes that a producer proposed this content with this support and lineage. It does not establish:
- that the claim is true;
- that multiple lineages are independent;
- that an accepted claim applies to a target it was not evaluated against;
- that a decision recorded here is project acceptance anywhere else; Coordination is unsupported in 0.1, and the local authority binding is the standalone mode.
