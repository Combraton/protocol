# M4 divergences — independent pass resolution record

- **Task:** M4 step 7 of [Protocol 0.1](PLAN.md), [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Source:** section H of the independent implementation's [divergence log](../../../conformance/independent/python-core/DIVERGENCES.md). A spec-only helper wrote it from the documents at `39e9dc8`, without reading the reference provider, the runner source or the cross-checks.
- **Result before resolution:**
  - Before the pass: 196 pass, 30 unsupported, 20 skipped.
  - First complete run from the documents alone (`cd411d5`): 205 pass, 17 fail, 4 unsupported, 20 skipped.
  - After the helper aligned with fixtures where the documents are silent: 219 pass, 3 fail, 4 unsupported, 20 skipped. All three failures were the runner's retry classes.

Each entry is resolved in the specification, the schemas, the fixtures, the reference provider or the runner, and often in several of these. Where a fixture had encoded an expectation the documents did not state, the documents now state it, or the fixture was loosened. Fixtures whose meaning changed were re-versioned.

## A. Real defects

| Tag | Defect | Resolution |
|---|---|---|
| H-RETRY-RUNNER | The runner and the reference sent and required retry `no` for `upload_offset_mismatch`, `upload_incomplete` and `hold_active`, against CORE §12 and M4-Q5 | **runner and reference:** retry class `after_reconcile` for the three codes. The runner checks retry classes itself, so any provider sending another class fails. |
| H-CORE12-TABLE | A sentence inside the CORE §12 table ended it, so the rows after `hold_active` no longer rendered as registry rows | **spec:** the sentence follows the table, with the rule for subject-free profile limits |
| H-CHUNK-LIMIT | The reference computed `chunk_limit` as three quarters of the smallest limit, which is not always a multiple of 3, so base64 of a full chunk could exceed the string limit by one character | **reference:** the largest multiple of 3 whose base64 fits. **spec:** EVIDENCE §4 states it. |
| H-PURGE (event order) | CORE §16.3 puts a command's primary subject first; the reference appended released holds before the artifact's purge events | **reference:** artifact events first. **fixture:** `evidence.purge-appends-the-artifact-events-first`. **mutant:** `purge-events-holds-first`. **spec:** EVIDENCE §9 event order. |
| H3-CTX-SATISFACTION | The reference satisfied an item by any section for it, ignoring the item's `check`; CONTEXT §3 says the check decides | **reference:** checks decide (`source_included`, `evidence_included`, `authority_content_included`). **fixtures:** scripted sections name their source; new `context.items-are-satisfied-only-by-their-check`; context and composition fixtures re-versioned. **mutant:** `check-ignored`. **spec:** CONTEXT §12 states how scripted content meets checks. |
| H-CTX-SHARED | The reference attached a request from another principal to an existing job; CONTEXT §4 requires the same access scope | **reference:** same submitting principal. **fixture:** `context.shared-job-survives-one-subscriber-cancelling` v2. **mutant:** `shared-job-across-principals`. **spec:** CONTEXT §4. |
| H-HOLD-EXPIRY, H-HOLD-EXPIRED-STATE | EVIDENCE §9 mentioned expired holds, but no state, event or behavior existed; the reference ignored `expires_at` | **spec:** hold states `active`, `released`, `expired`; event `evidence.hold.expired`; an expired hold no longer blocks purge; `expires_at` must be in the future. **schema:** hold state `expired`. **reference:** expiry at the clock. **fixture:** `evidence.expired-hold-stops-protecting`. **mutant:** `hold-expiry-ignored`. |
| H-TRANSITION-EXACTLY | CONTEXT §3 says `transition` is present exactly for `required_before_transition`; the item schema only required it there | **schema:** `transition` forbidden for other obligations. **fixture:** `context.request-items-are-checkable-and-obligations-negotiated` v2 accepts the member or the item as the error path. |
| H-REVAL-QUEUE-REASON | With several bindings not current, the reference reported the last one's reason | **reference:** stale, then unsatisfied, then unknown. **spec:** EXECUTION §13.1. **fixture:** `execution.revalidation-reports-match-mismatch-and-unavailable-by-obligation` v2. |

## B. Documents silent where fixtures expected an answer

The documents now state what the fixtures, the reference and, after section H.3, the independent implementation agree on.

| Tag | Resolution |
|---|---|
| H3-STAGED-AVAILABILITY | **spec:** staged and sealed artifacts are `available` while stored bytes are intact; `partial` means sealed bytes are known lost. Abandoned artifacts are `unavailable` with their reason; **reference** aligned; **fixture** `evidence.seal-refuses-digest-mismatch-and-is-idempotent` v2. |
| H3-ABANDON-REASON | **spec:** `abandoned_by_producer` |
| H3-QUERY-ORDER, H-QUERY | **spec:** artifact ID order; `next_cursor` only when more remain; a foreign cursor is `invalid_cursor`. **reference:** `next_cursor` and `invalid_cursor` aligned. **fixture:** `evidence.integrity-failure-is-never-served` v2 pages through results. |
| H3-HOLD-VISIBILITY, H3-HOLD-ORDER, H3-HOLD-ACTIVE-DETAILS | **spec:** holds are visible to their owner, authorities and readers of the held artifact; listed in ID order; `hold_active` lists hold IDs |
| H3-LOSS-KINDS | **spec:** named dependency kinds (`evidence.hold`, `evidence.manifest_child`; *candidate* `context.packet_citation`, `execution.output`); `tracked` says which a provider tracks. **fixture:** `evidence.purge-requires-release-authority-for-holds` v2 no longer requires an exact `tracked` list. |
| H3-IMMEDIATE-DELETION | **spec:** a store confirming deletion within the purge transaction reports `purged` in the outcome, with both events |
| H3-PACKET-PRODUCER | **spec:** CONTEXT §5 names the producer of self-sealed and remotely sealed packets |
| H3-LIVE-ITEMS | **spec:** while preparing, items already met report `satisfied` |
| H3-AUTHORITY-ENTRIES | **spec:** `authority_revision` is the revision the packet was prepared against, after corrections; older content is historical |
| H3-PUBLISH-REVISION | **spec:** one request revision per publication |
| H3-JOB-ID, H-CTX-IDS | **spec:** the job ID and packet artifact ID are conformance conventions in CONTEXT §12, not requirements on callers (the compiler name was dropped from the conventions in C.1) |
| H-SUBMIT-STATES | **spec:** a submit outcome is `preparing` or `refused` |
| H-ARTIFACT-PROVIDER-OPTIONAL | **spec:** a reference without `provider` means the provider asked; references carried to other participants always name it |
| H-CONTENT-DIGEST-ALG | **spec:** content digests use the algorithms the provider supports for digests; others are `unsupported_digest_algorithm`, and wrong lengths `invalid_envelope`. **reference** aligned. **fixture:** `evidence.descriptor-provenance-coverage-and-locator-rules` v2. |
| H-CHUNK-STEP | **spec:** `chunk_limit` is decided at step 2 with the Core limits; undecodable base64 is `invalid_envelope`. **reference** moved it. **fixture:** `evidence.chunk-limit-accounts-for-encoding-overhead` v2 (an oversize chunk with a stale precondition). |
| H-PREPARE-VALIDATION (uncertainty) | **spec and reference:** `not_before` after `not_after` is `invalid_envelope`; **fixture** step added |
| H-HOLD-PLACE | **spec and reference:** a hold on an artifact whose purge was requested is `not_found`, and so is releasing a hold that is not active. **fixture:** step added to the purge fixture. |
| H-RELEASE-AUTH | **spec:** the "delegated by" clause now defers to CORE §15, which already bounds delegation |
| H-CTX-EXPAND | **spec and reference:** a readable citation with a digest other than the sealed one is `artifact_digest_mismatch` |
| H-CTX-PACKET-INSPECT-RIGHTS | **spec:** the packet-inspect excerpt belongs to the result facts under `context.packet.read`; complete bytes need `evidence.read` at the evidence provider |
| H-REVAL-STATE-VIEW | **spec:** a binding's `revalidation` is its latest check's state. Evaluating at every read would make queries call other providers. |

## C. Readings kept as the independent implementation chose, with no change needed

H-EVD-NEG, H-CTX-NEG, H-EVD-FEATURE-OPS, H-TERMINAL-COVERAGE, H-STAGING-TIMEOUT, H-SEAL-REPEAT, H-FETCH, H-DELETION, H-MANIFEST, H-BOUND-WORK, H-EVD-READ-RIGHTS, H-CTX-OBLIGATION-STEP, H-CTX-BUDGET, H-CTX-UNSATISFIED-REASONS, H-CTX-DEADLINE, H-CTX-INVESTIGATE, H-CTX-JOB-END, H-CTX-CANCEL, H-CTX-PUBLISH-EVENTS, H-CTX-UPDATES, H-CTX-CORRECTION, H-CTX-EVENT-VISIBILITY, H-REVAL-SCOPE, H-REVAL-HELD, H-REVAL-OBSERVE and H-REVAL-BLOCKS match the documents and the reference, or differ only where the documents leave the choice to the provider.

H-REVAL-BOUNDARIES: whether advisory bindings are also checked at dispatch is left to the provider. Advisory bindings never block, and checks are recorded only when their result changes.

## C.1 Seventh pass (realignment, base `8500948`)

The helper realigned the independent provider with the resolved documents (`6e9bebf`, `35f09a5`, merged). Its full suite gave 225 pass, 0 fail, 4 unsupported and 20 skipped; the Evidence, Context and revalidation subsets all pass. Its remaining open points:

| Tag | Resolution |
|---|---|
| H7-COMPILER-NAME | **spec:** the provenance `compiler` names the implementation; it is no longer a conformance convention (no fixture checks it) |
| H7-SHA512-ADVERTISED | **spec:** sha512 content digests are accepted when the `core.describe` manifest lists `core.digest-sha512`. The independent provider accepts sha512 without listing it, which no fixture exercises. |
| H7-DIGEST-ALG-STEP | **spec:** decided at step 2. The independent provider decides at step 7; the fixture cannot tell the two apart, which is a recorded coverage limit. |
| H7-REVALIDATION-NO-CHECK | No change: every binding under the feature is checked at admission, so it always has a latest check |
| H7-STAGED-INTEGRITY, H7-EXPIRED-HOLD-IN-LOSS, H7-PACKET-CITATION-TRACKED | No change: consistent with the documents |
| H7-PARTIAL-UNREACHABLE | Coverage limit: nothing in the reference store controls produces `partial` availability |

## C.2 Eighth pass (owner close-out, base `63f1bb0`)

The helper realigned the independent provider with the close-out pass (`76db7a0`, `dc2f8c8`, merged). Its full suite gave 228 pass, 0 fail, 4 unsupported and 23 skipped, and every close-out execution, evidence and context fixture it can run passes. `observe_host_basis`, `serve_altered_bytes` and `require_current` with fetch appear only in composition fixtures, which a stdio participant skips.

| Tag | Resolution |
|---|---|
| H8-RESUMED-REASON | **spec:** the event reason on reacquiring is `resumed`, and `scheduling` in inspect then carries no reason |
| H8-PACKET-FACTS-OBSERVED | **spec:** `observed` for `packet.facts` is `corrected: <item_id>[, …]`, and for `packet.current` `superseded by revision <n>` |
| H8-REQUIRE-CURRENT-WITHOUT-FETCH | **spec and reference:** `require_current` needs `fetch.context`, otherwise `invalid_envelope`. **fixture:** step added to `execution.revalidation-reports-match-mismatch-and-unavailable-by-obligation` v3. |
| H8-END-TIMING | No change: the released execution is evaluated at its dispatch boundary on every re-evaluation, which is what the spec requires |
| H8-RELEASED-TIMEOUT-OVERLAP | **spec:** a passed delivery or execution deadline ends released work with `scheduling.reason: "deadline_passed"`; the §8 delivery-timeout evidence may also be recorded |
| H8-BASIS-CHANGES-MERGE | **conformance README:** `basis_changes` and `observe_host_basis` both merge per member and per repository, in time order (the helper's `basis_changes` replaced; no fixture tells the two apart) |
| H8-CONSTRAINT-ADDITION | **spec:** a delegated grant may add constraints, which only narrow it |
| H8-INVALIDATED-ITEMS (`superseded_by`) | **spec:** `superseded_by` names the request's current revision |
| H8-CAPACITY-FAIRNESS | **spec:** the order between admitting queued work and resuming released work is unspecified; this is a coverage limit |

## C.3 Ninth pass (realignment, base `9591551`)

The helper realigned with C.2 (`36ca076`, merged): 228 pass, 0 fail, 4 unsupported and 23 skipped, on the first run after the changes.

| Tag | Resolution |
|---|---|
| H9-TIMEOUT-SCHEDULING-ORDER | **spec:** EXECUTION §15.1 fixes one order for released work ending before dispatch, whatever the reason: each `execution.timeout.passed`, then `core.effect.obligation.overdue` when a deadline ended the wait, then `execution.delivery.observed`, and last `execution.scheduling.changed`. The evidence-class table gains a row: `delivery_timeout_before_dispatch` when the delivery timeout passed, otherwise `scheduling` on the delivery record and `never_dispatched` on the effect. This replaces the C.2 reading that the §8 evidence "may also be recorded". **reference defect:** it ended released work at the dispatch boundary before its timeouts were evaluated, so no `execution.timeout.passed` was ever emitted for the delivery timeout that caused the ending; now fixed. The independent executor put the scheduling change first on the cancellation path. **fixture:** `execution.released-work-ending-emits-cause-first`; mutant `released-ending-before-cause` fails at the exact event read. |
| H9-OBSERVED-UNTESTED | No change: a coverage limit of the stdio binding; the strings are checked by the composition fixtures on the reference only |

## D. Still unchecked by fixtures (coverage limits)

From H.5, after this resolution:
- **Evidence:** `evidence.availability.changed`; fetch and excerpt shrinking to small receive limits; a manifest child that is sealed but unavailable; `released_holds` filtering; the less common credential-locator forms.
- **Context:** script steps `end` and `unmet`; `omit` naming an item; conditions of kind `dirty_snapshot` and `environment_digest`; a cancel that leaves no subscriber; job-event visibility; shared jobs with different fallbacks; corrections on advisory items.
- **Revalidation:** advisory checks at dispatch. (Blocks clearing at dispatch and transition are now covered by `execution.blocked-dispatch-releases-capacity-and-resumes` and `execution.transition-block-clears-when-current`.)
- **Capacity release:** fairness between admitting queued work and resuming released work (H8-CAPACITY-FAIRNESS); both deadline kinds passing at one evaluation of released work.
- **Composition `observed` strings** for `packet.facts` and `packet.current` are checked on the reference only (H9-OBSERVED-UNTESTED).
