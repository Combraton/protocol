# Verification profile `verification/1` — proposed M5 draft

> **Status: proposed draft for Protocol 0.1 milestone M5.** Nothing here is normative until M5 is accepted together with its schemas, fixtures, mutants and independent evidence. Names marked *candidate* may change. Choices that need an owner decision before implementation are marked with their question number ([M5](../../work/release-0.1/M5.md#proposed-decisions)). Owner decision U2 (release plan §6) bounds the depth: receipts plus `evaluate_contract` job references, with no signing and no evaluator orchestration. Architecture: [SPEC §2, §5, §8, §10, §12](../SPEC.md). Requirement IDs refer to the [matrix](../../work/release-0.1/MATRIX.md). Sources: combraton `docs/architecture/VERIFICATION.md` §1–§3, §5–§7 and `docs/architecture/MODEL.md` §5 at `9af69ce`.

A **verifier** evaluates named properties of exact subjects under a contract and reports each property's result. A **receipt holder** keeps receipts and answers assessments; a verifier usually holds its own receipts. A **reader** checks whether a receipt still says something about the subjects it cares about.

A receipt is a **scoped observation**. It says which properties an identified evaluator found to pass, fail, go unevaluated or stay indeterminate, for exact subjects, in a stated environment and period. It is not a stop command, not project acceptance, and not a reliance decision (VER-2, SPEC §12).

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119 and RFC 8174 when in capitals.

## 1. Dependencies and negotiation

- `verification/1` depends on `core/1` with `core.events` and `core.capabilities` (`core.grants` optional, as for Evidence), and on `evidence/1` as a protocol dependency: contracts, receipts and cited evidence are exact Evidence artifacts (§2, M5-Q5). Required Core features are enforced at negotiation, as for Evidence.
- Optional features (*candidate*): `verification.jobs` (§4, a verifier that runs evaluations), `verification.record` (§5, a holder that records receipts from external verifiers) and `verification.assess` (§7).
- A provider may implement jobs, recording, or both. A CI publisher that only records receipts needs neither Execution nor Knowledge.

## 2. Identities

| Identity | Scope and meaning | Must not be used as |
|---|---|---|
| Contract reference `{ provider, artifact, digest }` | The exact sealed contract artifact (§3). A changed contract is a new artifact with a new digest. | A contract name, or "the latest" contract |
| Property `property_id` | One property within one contract | A property of another contract, even with the same name |
| Job `{ "kind": "verification.job", "id" }` | One requested evaluation (§4) | A receipt; a job's state proves nothing about properties |
| Receipt `{ "kind": "verification.receipt", "id" }` and receipt reference `{ provider, receipt, digest, artifact }` | One immutable receipt, sealed as an Evidence artifact (M5-Q5) | A verdict on anything other than its subjects, contract and environment |
| Evaluator `{ id, version, principal }` | The implementation that evaluated, its version, and the principal it authenticated as | An authority |

## 3. Contracts

A contract is a sealed Evidence artifact in format `combraton-verification-contract/1` (media type `application/vnd.combraton.verification-contract+json`, canonical JSON):

| Member | Meaning |
|---|---|
| `outcome` | Free text: the outcome the contract is about |
| `subjects` | `[ { role, kind } ]`: the subject roles a receipt must identify, for example `{ role: "tree", kind: "repository_tree" }` or `{ role: "output", kind: "evidence.artifact" }` |
| `properties` | `[ { property_id, layer, required, statement } ]`, with `layer` one of `static`, `component`, `integration`, `journey`, `runtime_path` or `qualitative`. A lower layer never stands in for a higher one. |
| `environment` | Required environment anchors, if any, for example an environment digest |
| `evaluators` | Optional list of evaluator `id`s whose results this contract accepts |
| `freshness` | `{ max_age_seconds? }`: how long after `observed_until` a receipt may be assessed as current |

The protocol defines how a contract is identified and checked, not how a property is evaluated.

## 4. Evaluating a contract (`verification.jobs`, VER-3)

`verification.evaluate_contract` (command) creates a job:

| Member | Meaning |
|---|---|
| `contract` | Contract reference |
| `subjects` | `[ { role, digest, subject? } ]`: exact digests (a tree ID, an artifact digest), and optionally the protocol subject they came from |
| `environment` | `{ anchors, digest? }` requested |
| `evaluator` | Optional evaluator `id` |

- **Outcome: a job reference only.** `{ job, state: "queued" | "refused", reason? }`. A command never returns property results; the report is a separate receipt (VER-3).
- **Capability.** The evaluator is advertised as the capability predicate `verification.evaluator.<id>`, whose evidence names its version (CORE §17). If its status is not `supported`, the command is refused with `capability_unavailable` at step 7.
- **Pinned evaluator.** When a job is admitted it records `evaluator: { id, version }`. Its receipt names that evaluator (SCN-7, M5-Q9).
- `verification.job.inspect` (query) `{ job }` → `{ state: "queued" | "running" | "completed", evaluator, receipt? }`. A job is `completed` exactly when its receipt is issued.
- **Losing the evaluator.** If the pinned evaluator becomes unavailable while a job runs, including across a provider restart, the job still completes with a receipt. Every property not yet evaluated is `indeterminate` with reason `evaluator_unavailable`. It is never `pass`.
- **A changed evaluator** version is a different evaluator for later jobs. A running job either finishes under its pinned version or treats it as lost. Old receipts are never rewritten.

## 5. Receipts (VER-1)

Receipt content, sealed in format `combraton-verification-receipt/1`:

| Member | Meaning |
|---|---|
| `subjects` | `[ { role, digest, subject? } ]` for every role the contract names |
| `contract` | Contract reference, with its digest |
| `evaluator` | `{ id, version, principal }` |
| `environment` | `{ anchors, digest? }` as observed |
| `inputs` | Evidence references and build identities the evaluation used |
| `properties` | `[ { property_id, result, reason?, evidence, coverage } ]`, exactly one entry for **every** contract property. `result` is `pass`, `fail`, `not_evaluated` or `indeterminate`. `reason` is required unless the result is `pass` or `fail`. |
| `observed_from`, `observed_until` | The observation period, on the verifier's clock |
| `validity` | `{ valid_until? }`: an explicit end, if the verifier states one |
| `scope` | What the receipt covers, for example a named journey on one environment |
| `job` | The job, when the verifier ran one |
| `provenance` | Execution and evidence references for how the evaluation ran |

- **Every property is listed.** A receipt missing a contract property, listing one twice, or naming a property not in the contract is `invalid_envelope` at `/payload/properties`, with details `missing`, `duplicated` or `unknown`. Unavailable checks are listed as `not_evaluated` or `indeterminate` with their reason; they are never omitted and never collapsed into `pass` or `fail` (VER-1).
- **Recording** (`verification.record`). `verification.receipt.record` (command) records a receipt from an external verifier, such as CI. The evaluator `principal` is the session principal. The holder reads the contract at the named provider and checks the digest; a mismatch is `artifact_digest_mismatch`.
- Receipts are immutable. Re-evaluating produces a new receipt; a failed receipt stays visible after a later pass.
- `verification.receipt.inspect` (query) `{ receipt }` → the content and reference. `verification.receipt.query` (query) `{ contract?, subject_digest?, cursor?, limit }` lists receipts the reader may read, revealing no existence.

## 6. Results are scoped observations (VER-2)

- A `fail` result changes nothing outside the receipt:
  - it does not change any execution axis;
  - it does not cancel or stop work;
  - it does not revoke grants or change reliance.

  A consumer that wants to act on a failure does so through its own authorized operations.
- A receipt about an execution's output is not the execution's `evaluation` axis. An executor MAY record an attributed reference to it there (EXECUTION §3), and never derives it.

## 7. Assessment: missing, unavailable or outdated is never a pass (`verification.assess`, M5-Q6)

`verification.receipt.assess` (query) checks one receipt against what the reader needs now:

`{ receipt: reference, contract: reference, subjects: [ { role, digest } ], environment?, at? }`, where `at` defaults to the provider's clock

The result is per property: `{ property_id, required, status: "satisfied" | "failed" | "not_satisfied", reasons }`, plus `overall`. It records nothing and emits no event.

A property is `satisfied` only when all of these hold:
1. the receipt is readable and its digest matches the reference;
2. the receipt's contract digest equals the requested contract's digest (otherwise `contract_mismatch`);
3. every requested subject digest equals the receipt's digest for that role (otherwise `subject_mismatch`);
4. the environment matches where the contract requires it (otherwise `environment_mismatch`);
5. the evaluator is permitted by the contract's `evaluators`, if listed (otherwise `evaluator_not_permitted`);
6. `at` is before `validity.valid_until` and within `freshness.max_age_seconds` of `observed_until` (otherwise `stale`);
7. the property's result is `pass`.

Otherwise:
- a `fail` under conditions 1–6 is `failed`;
- `not_evaluated` or `indeterminate` is `not_satisfied`, with that result as its reason;
- any failed condition 1–6 makes the property `not_satisfied`, with that condition as its reason, whatever its result.

`overall` is `failed` if any required property is `failed`, otherwise `not_satisfied` if any required property is not `satisfied`, and otherwise `satisfied`. A receipt the reader may not read is `permission_denied`, identical for a nonexistent receipt (CORE-12). A readable receipt whose bytes are unavailable is `not_satisfied` with reason `receipt_unavailable`.

## 8. Rights

| Right | Covers |
|---|---|
| `verification.evaluate` | `evaluate_contract` on covered jobs |
| `verification.record` | `receipt.record` on covered receipts |
| `verification.read` | `job.inspect`, `receipt.inspect`, `receipt.query`, `receipt.assess` and events of covered jobs and receipts |

Reading a receipt's cited evidence needs Evidence rights at its provider.

## 9. Verification grants no authority

- Issuing, recording or assessing a receipt never records a reliance decision (KNOWLEDGE §6), resolves a conflict, or accepts project direction.
- A verifier principal is not an authority because its receipts pass.
- A `satisfied` assessment is an input that an authority MAY cite in a decision's `validation_basis`; the decision is still the authority's own operation.
- A human override of a failing receipt is an authority decision with its own rationale and scope; it never rewrites the receipt.

## 10. Events

*Candidate* types: `verification.job.changed` `{ state, evaluator? }`, `verification.receipt.issued` `{ reference, job }`, `verification.receipt.recorded` `{ reference }`.

## 11. Conformance and test controls

- **Reference participants only.** A reference verifier drives a **scripted evaluator** from launch configuration (`verifier`, *candidate*):
  - evaluator `id` and `version`;
  - per-job property results and reasons;
  - delays against the clock file;
  - an unavailable environment;
  - evaluator availability across restarts.

  The script is test environment, as EXECUTION §15.
- No fixture depends on a real browser, CI system or model reviewer.

## 12. What verification does not establish

A receipt establishes that an identified evaluator reported these property results for these exact subjects, under this contract, in this environment and period. It does not establish:
- properties the contract did not name, or higher layers than those evaluated;
- anything about a different subject, including an integration of individually verified parts;
- independence of evaluators that share sources;
- permission to rely on a claim or to accept project direction.
