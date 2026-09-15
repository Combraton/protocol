# M5 divergences and resolutions

The independent Python provider was extended to `knowledge/1`, `verification/1` and single-provider `context.claims` by a spec-only helper. The helper worked in a separate worktree, based on `950f3e0`, and never read the reference, the runner source or the cross-checks. It committed its implementation (`97b5471`) before opening any M5 fixture, and afterwards read only the five fixtures that failed. Its record is `conformance/independent/python-core/DIVERGENCES.md` §H.M5, merged from `release-0.1/m5-independent`.

**Runs:**
- Baseline: 229 pass, 24 skipped, 18 unsupported.
- First run: 238 pass, 5 fail, 24 skipped, 4 unsupported.
- Final: 243 pass, 0 fail, 24 skipped, 4 unsupported.

Every point is resolved against the written contract. Where the reference, a fixture or the documents were wrong, that side was changed, not the independent implementation merely made to copy the reference.

## A. Defects

| Tag | Defect | Resolution |
|---|---|---|
| HM5-DEFECT-OVERALL | VERIFICATION §7 never consulted the checks for `overall`. With only optional properties, a receipt whose checks all failed was `satisfied`. The reference had the same bug. | **spec and reference:** `overall` is `not_satisfied` whenever a check is `failed` or `unverifiable`. **fixture:** optional-properties steps in `verification.assessment-never-passes-missing-unavailable-or-outdated` v2. **mutant:** `overall-ignores-checks`. |
| HM5-DEFECT-UNLISTED-PREDICATE | The fixtures expected an evaluator version the snapshot does not list to be refused as `unsupported`. CORE §17.1 says a predicate without evidence of support is `unknown`, and Execution's adapter predicates follow that. The independent implementation first did this, from the documents. | **reference and fixtures follow CORE:** an unlisted version is `unknown`, and a listed one reports its own status (VERIFICATION §4). Fixtures v2: evaluate and reconnect. **mutant:** `absent-evaluator-unsupported`. |
| HM5-DEFECT-PACKET-BASIS-TARGET | CONTEXT §14 compared "a target equal to the packet's basis" in canonical JSON, but no packet basis can equal a Knowledge target. The fixture passed only through an unwritten conversion. | **spec:** CONTEXT §14 defines the conversion: `id`, `tree` and a non-null `dirty` per repository; `environment`, `build` and `completeness`; `workspace` and `configuration` dropped. |
| HM5-DEFECT-ASSESS-EXISTENCE | VERIFICATION §7 read as making a nonexistent receipt `permission_denied` for everyone | **spec:** unauthorized readers get `permission_denied` identically; an authorized reader naming a nonexistent receipt gets `not_found`. The reference was already correct. |
| HM5-DEFECT-AUTHORITY-GET | The prose result lacked `revision`, which the schema requires | **spec:** `revision` is the binding subject's Core revision |
| HM5-DEFECT-RECORDED | VERIFICATION §4 did not say whether results assigned at completion are "recorded" | **spec:** `recorded` lists only evaluator-reported results; assigned ones appear only in the receipt |
| HM5-BIND-GRANT (reference) | The reference let an authority principal bind a scope while acting under a grant. CORE §15.5 restricts it to that grant, and no right covers binding. | **reference:** `not_authority`; **spec:** KNOWLEDGE §6. **fixture:** step in `knowledge.only-the-bound-authority-decides` v2. **mutant:** `authority-binds-under-grant`. |
| HM5-OBSERVED-ENVIRONMENT (reference) | An issued receipt reported the requested environment as observed when the evaluator observed nothing | **reference and spec:** only observed anchors, `{ anchors: {} }` when none. **fixture:** evaluate v2. **mutant:** `issued-environment-requested`. |
| HM5-RECEIPT-EVENTS-ORDER (reference) | `receipt.record` appended the artifact's events before the receipt's own, against CORE §16.3 (primary subject first) | **reference and spec:** the receipt event first. **fixture:** event read in `verification.receipts-list-every-role-and-property` v2. **mutant:** `receipt-events-artifact-first`. |
| HM5-RECEIPT-ID-TAKEN (reference) | A job whose ID named an existing receipt would have overwritten that immutable receipt | **reference and spec:** `precondition_failed` naming the existing receipt or job, in both directions. **fixture:** evaluate v2. **mutant:** `receipt-id-collision-accepted`. |
| HM5-RECEIPT-UNREADABLE (reference) | `receipt.inspect` returned null content when the sealed bytes could not be read | **reference and spec:** `unavailable`. **fixture:** step in assessment v2. |
| HM5-ASSESS-UNREADABLE-RECEIPT (reference) | With the contract unreadable, the reference passed the environment and evaluator checks | **reference and spec:** `unverifiable`, with `contract_unavailable`, or with the receipt's own reason when the receipt cannot be used. **fixture:** assessment v2. **mutant:** `contract-unverifiable-checks-passed`. |
| HM5-SINGLE-ENTRY-OVERLAP / support vacuity | Rule 5 and the availability state were vacuously true for a rootless entry, or for no entries, and the reference classified a single rootless entry as `overlapping_lineages` | **spec and reference:** complete and partial ancestry must name at least one root; rule 5 needs two entries; no entries means availability `unknown`. **fixtures:** `knowledge.claim-revisions-are-immutable-with-base-checks` v2 and `knowledge.support-classes-follow-declared-ancestry` v2. **mutant:** `empty-roots-accepted`. |

## B. Documents silent where fixtures or implementations needed an answer

| Tag | Resolution |
|---|---|
| HM5-VALIDITY-ORDER, HM5-GAP-VALIDITY | **spec:** `from` must be before `until`; an empty or inverted interval is `invalid_envelope` at `/payload/validity`. **fixture:** equal-bounds step added. |
| HM5-LISTING-DETAILS | **spec:** `missing`, `duplicated` and `unknown` are lists, always all three present |
| HM5-ASSESS-REFERENCE-MISMATCH | **spec:** a reference differing from its mapping makes the receipt unusable. Properties are empty, and the dependent checks are `unverifiable` with `receipt_reference_mismatch` (fixture v2). |
| HM5-CORE-PRECONDITION-ORDER, HM5-GAP-STEP7 | **spec:** the Core revision preconditions come before the profile's own step 7 checks. **fixture:** a reused decision ID gives `precondition_failed` before the authority checks (authority v2). The first resolution also put the preconditions before the authority epoch and the evaluator capability, which contradicted CORE §10. That is corrected in §D (HM5B-STEP7-CORE-ORDER). |
| HM5-NAMED-REVISION-RIGHTS | **spec (KNOWLEDGE §10):** an explicit list: `decision.record` needs read on its claim, `conflict.open` on both claims, and `applicability.evaluate` on its claim and its local dependency claims, because findings reveal existence. **reference:** dependency read rights added. **fixture:** `knowledge.dependencies-resolve-only-exact-references` v2. **mutant:** `dependency-read-unauthorized`. |
| HM5-RESOLVE-EPOCH | **spec:** `authority_epoch` is required for `conflict.resolve` too |
| HM5-LATEST-DECISION-NULL | **spec:** `supersedes_decision` present when no decision exists is `precondition_failed` with `latest_decision: null` |
| HM5-OBSERVED-ORDER | **spec:** `observed_from` after `observed_until` is `invalid_envelope` at `/payload/observed_until` |
| HM5-CONTRACT-FORMAT, HM5-CONTRACT-READ-ORDER | **spec and reference:** `format` is required; roles and properties are non-empty and unique; `required` is boolean; layers are known; the read order is stated; the media type is not checked. **fixture:** duplicated-property contract step (receipts v2). |
| HM5-JOB-STATES, HM5-GAP-ISSUED-RECEIPT, HM5-VERIFIER-PRINCIPAL | **spec:** job states and events are defined, and so are the issued receipt's members (`environment` observed only, `inputs`, `provenance`, `scope` from the contract's `outcome`, observation period) and its event order. **fixture:** the `receipt.issued` `job` member. |
| HM5-SCRIPT-END, HM5-SCRIPT-REASON | **spec (§11) and launch schema:** a script without `complete` leaves the job `running`; a scripted reason is required exactly for `not_evaluated` and `indeterminate`. **fixture:** stalled-script step (evaluate v2). |
| HM5-RECEIPT-INSPECT-FEATURE | **spec:** `receipt.inspect` and `receipt.assess` are base-profile operations |
| HM5-CLAIM-SECTION-ITEM | **spec:** only a section that is not `historical` satisfies a `claim_included` item, as for every M4 check. The reference already did this; the independent implementation must realign. |
| HM5-CLAIM-CHANGES | **spec:** `applicability_changed` compares the result only, and there is at most one entry per kind per section, in a fixed order. **fixture:** a same-result re-evaluation step (claims v2). |
| HM5-UNVERIFIED-BY-TABLE, HM5-GAP-UNVERIFIED-RELIANCE | **spec:** `applicability_not_established` applies only to `binding` and `evidence` items. **fixture:** an evaluator-change phase (claims v2). |
| HM5-FACET-ORDER | **spec:** inspect lists open and resolved conflicts in recorded order; the packet snapshot lists open ones |
| HM5-CLAIM-READ-TIME | **spec:** the snapshot reflects the claim as read during preparation, no later than publication; the instant is provider-defined |
| HM5-CLAIMS-FEATURE-ORDER | **spec:** the order of `features` is not significant |
| HM5-DUPLICATE-IDS | **spec:** duplicate `support_id` or `condition_id` is `invalid_envelope`, as the reference already refused. **fixture:** duplicate-support step. |
| HM5-AUTHORITY-SCOPES | No change: knowledge authority scopes are not grant `authority_binding` scopes |
| HM5-SCRIPT-REASON (independent) | **spec (§11) and launch schema:** the helper filled a missing scripted reason with the result. A script without a required reason, or with one on `pass` or `fail`, is now an invalid launch configuration, so no provider fills it in. |
| HM5-RECEIPT-ID-TAKEN (independent) | The helper left such a job `running`. The resolution refuses the submission instead (§A), so an ID collision never reaches completion. |
| HM5-VERIFIER-PRINCIPAL (independent) | The helper used scope `verification job <id>`. VERIFICATION §4 now takes `scope` from the contract's `outcome`. |
| HM5-TIME-BOUNDS | **spec:** the boundaries are written exactly, as the helper read them: `stale` at or after `valid_until`, or strictly after `observed_until` plus `max_age_seconds`. |
| HM5-UNKNOWN-ROOTS-PATH, HM5-SELECT-EXACT, HM5-REASON-STEP, HM5-LABEL-DEMOTION | Confirmed as written. The helper's reading matches the documents and the fixtures; no change. |
| HM5-RECORDED-FILL, HM5-UNLISTED-EVALUATOR | The helper's fixture-driven changes. They are resolved by HM5-DEFECT-RECORDED (the spec now says it) and HM5-DEFECT-UNLISTED-PREDICATE (the fixtures were wrong, so the helper's first reading, `unknown`, was right). |
| Format `/2` without `context.claims` (probe) | **fixture:** the fetched packet bytes' `format` is checked with the new runner pattern `$base64_json` (claims v2). **mutant:** `claims-format-without-negotiation`. |

## C. Found while resolving

- **Mixed directive patterns.** An expectation object with a `$` directive is that directive alone. Two M5 patterns that mixed `$canonical_sha256` with members checked only the digest, so the receipt-environment mutant survived. The patterns are split, and `check-fixtures` now refuses any pattern that mixes a directive with members.
- **Dependency identity** (owner follow-up): exact provider-qualified dependency resolution, recorded in M5 status. The helper's base predates it, so the realignment pass covers it.
- **Receipt artifact events** (found when the independent provider ran the version 2 fixtures).
  - **Divergence.** The receipts fixture expected exactly `receipt.recorded`, `artifact.staged`, `artifact.sealed`. The independent provider's own publication also appends `evidence.artifact.appended`, and no document fixes which events an internal publication appends.
  - **Resolution:**
    - VERIFICATION §5 now says so, and readers must not depend on the artifact's event count or revision;
    - the fixture checks only that the receipt's event comes first, which still kills `receipt-events-artifact-first`.

## D. Twelfth pass: realignment (H.M5b)

- **The pass.** The helper realigned the independent implementation with the resolved documents from `6cec5f6`. Its readings were committed before any fixture was opened.
- **Result.** It passed every fixture on the first run (245 pass, 24 skipped, 4 unsupported), so no fixture was read. Its sensitivity probe found 16 unguarded deviations.
- **Resolutions.** Every open point is resolved against the written contracts. Each resolution either follows an accepted contract or writes down a reading, and each carries a distinguishing fixture step and mutant.

| Tag | Resolution |
|---|---|
| HM5B-STEP7-CORE-ORDER | **A contradiction introduced by the first resolution.** CORE §10 orders step 7 as capabilities, then authority epoch, then preconditions, and CORE §17 checks a capability "before its preconditions". The first resolution put the Core preconditions first in KNOWLEDGE and VERIFICATION. CORE is accepted, so the profiles now follow it. **KNOWLEDGE §6 and §7:** a decision's or resolution's authority epoch (the binding's epoch for the named revision's scope, when that revision or record exists and the scope is bound), then the preconditions, then the profile checks. **VERIFICATION §4:** the evaluator capability, then the preconditions, then the ID collision, contract and roles. The reference had placed both after the preconditions and is fixed. **fixtures:** a stale epoch with a reused decision ID is `stale_authority_epoch` (authority v3); an unlisted evaluator version for an existing job ID is `capability_unavailable` (evaluate v3). **mutants:** `knowledge-epoch-after-preconditions`, `evaluator-after-preconditions`. |
| HM5B-SELF-OR-REMOTE | **spec (KNOWLEDGE §8):** reasons are decided in table order, and `remote_dependency` comes first. `self_reference` needs this provider. The reference checked `self_reference` first and is fixed. **fixture:** a remote reference repeating the evaluated claim ID and revision (dependencies v3). **mutant:** `self-reference-before-provider`. |
| HM5B-HISTORICAL-CLAIM-REASON | **spec (CONTEXT §14):** a historical section gives `invalid_for_target` for every reliance, and at the read an item of any reliance whose claim is now `invalid_for_target` is invalidated, with `permitted_use_lost` first. The reference had reported `unavailable` and ignored hypothesis items at the read; it is fixed. **fixture:** hypothesis items in `r-h` (claims v3). **mutants:** `historical-claim-reason-unavailable`, `hypothesis-invalidation-unreported`. |
| HM5B-SCRIPTED-UNMET-SCOPE | **spec (CONTEXT §12):** a scripted `unmet` never makes a satisfied item unmet. For an unsatisfied item it outranks every check reason, including `knowledge_unavailable` and `claim_digest_mismatch`. The reference let the script override a satisfied item; it is fixed. No earlier fixture used scripted `unmet`. **fixture:** claims v3. **mutant:** `scripted-unmet-overrides-satisfied`. |
| HM5B-COLLISION-DETAILS | **spec (VERIFICATION §4):** the collision entry carries `current` where readable, as any precondition entry does (CORE §7). The reference omitted it and is fixed. **fixture:** evaluate v3. |
| HM5B-QUEUED-LOSS | **spec (VERIFICATION §4):** a job whose evaluator is lost before its first evaluation never ran. It goes from `queued` straight to `completed`, and `observed_from` equals `observed_until`. Neither implementation did exactly this: the reference used the submission time, and the independent passed through `running`. **Unguarded;** see §E. |
| HM5B-ISSUED-EVENTS | Confirmed as written. **fixture:** property recorded, then `receipt.issued`, then `job.changed` (evaluate v3). **mutant:** `issued-events-job-first`. The issued `scope` is also checked now (**mutant:** `issued-scope-from-job`). |
| HM5B-UNUSABLE-REASON-ORDER | **spec (VERIFICATION §7):** a check lists every applicable reason, `contract_unavailable` first. Top-level `reasons` follow check order without duplicates. The reference dropped one of the reasons and is fixed. **fixture:** assessment v3. **mutant:** `unusable-reasons-dropped`. |
| HM5B-TIME-WITHOUT-CONTRACT | **spec (VERIFICATION §7):** without the contract, only `valid_until` and `observed_until` are compared, as both implementations did. **fixture:** assessment v3 (`time` passed). |
| HM5B-CONTRACT-MEMBERS | **spec (VERIFICATION §3):** a contract is a closed object (CORE §5.1). Every member is required, and a wrongly typed or unlisted member is `invalid_format`. The reference accepted unlisted members and is fixed. **fixture:** receipts v3. **mutant:** `contract-members-open`. |
| HM5B-DUPLICATE-PATH-ORDER | **spec (KNOWLEDGE §3):** each entry is validated before duplicates are checked, as the helper read it |
| HM5B-EVALUATE-DEPENDENCY-RIGHTS | Confirmed. The difference matters only for commands that then fail `not_found` at step 7. |

## E. Still unchecked by fixtures (coverage limits)

- **Knowledge:**
  - `decision.record` without `knowledge.read` on its claim (the right is implemented and specified);
  - the step 7 order for `conflict.resolve` (decisions are checked);
  - duplicate `condition_id`.
- **Verification:**
  - a job losing its evaluator before its first evaluation (HM5B-QUEUED-LOSS);
  - the rarer contract violations (unknown layer, non-boolean `required`, empty lists);
  - the exact `max_age_seconds` boundary (the `valid_until` boundary and later instants are checked).
- **Context:**
  - `unverified_items` limited to `binding` and `evidence` items;
  - resolved conflicts left out of snapshots.
- **Independent coverage:** `context.knowledge_provider` peers and `execution.claim_revalidation` need the Unix-socket binding, so SCN-16 and the Execution side of CMP-9 are checked only on the reference. The composition runs one implementation for every participant; it is not mixed-implementation proof.
