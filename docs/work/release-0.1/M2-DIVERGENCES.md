# M2 — resolving the independent implementation's findings

The independent Python Core provider ([README](../../../conformance/independent/python-core/README.md)) was written from the published documents at `f42d21a` by a helper that did not read the reference provider, the cross-check code or the runner source. It passed all 57 M1 fixtures on its first run. That was not evidence of a good suite: its [divergence log](../../../conformance/independent/python-core/DIVERGENCES.md) showed that 19 deliberate violations of stated requirements also passed.

This record maps each finding to its resolution. Tags match the divergence log.

**Resolution kinds:**
- **spec:** normative text changed to remove a contradiction or pin a decision.
- **fixture:** a new or changed fixture now distinguishes the behavior, with a mutant it must fail.
- **reference:** the reference provider was wrong or looser than the decided behavior.
- **deferred:** not resolvable in M2; the owning milestone is named.

Several decisions are pinned "as the independent implementation chose". Its reading of the text was usually the more literal one. Either implementation can therefore fail a fixture written from this record; fixtures, not either implementation, are the arbiter.

## A. Contradictions

| Tag | Resolution |
|---|---|
| D-STREAM-FRAC | **spec:** STREAM §3 no longer calls fractional IDs `invalid_request`. The value domain makes them a frame-level `parse_error`. |
| D-STREAM-ID | **spec:** request ID strings are 1–128 Unicode code points, matching the schema. **reference:** counts code points. **fixture:** a 65×`é` ID is accepted. |
| D-TESTCTL | **spec:** CORE §13.1 describes launch configuration, not a control channel. Keys are defined in [conformance README](../../../conformance/README.md#launch-configuration) and `conformance/schemas/launch-config.schema.json`. `retain_generations` means `oldest_retained = max(oldest_retained, current − R + 1)`. **fixture:** advance 2 with retain 2 distinguishes this from `current − R`. |
| D-RESULT-SUBJECT | **spec:** CORE §13 shows the full `get` and `applied_count` results, including `subject`. |
| D-MANIFEST-FIELDS | **spec:** CORE §4.1 lists `unknown_extensions`, per-profile `depends_on` and the unsupported `reason` values. |

## B. Unstated expectations

| Tag | Resolution |
|---|---|
| D-ERR-RETRY | **spec:** STREAM §2 and §3 give `retry: "no"` and empty `details` for `parse_error`, `invalid_utf8`, `frame_too_large` and `invalid_request`. |
| D-LIMIT-SCOPE | **spec:** CORE §9 says limits are measured over the whole `params` object; `params` is depth 1, each nested object or array adds one, scalars add nothing; member names count as strings; JSON-RPC `id` and `method` are bounded by the binding. **fixture:** values exactly at each limit are accepted; limit + 1 is refused for depth, string bytes, member names, array items and payload bytes. |
| D-APPLIED-ABSENT | **spec:** `applied_count` of a subject that never existed is 0. |
| D-CTL-NAME | **spec:** the unverifiable "refuse control-channel method names" requirement is removed; no test operations are reserved. **fixture:** `stream.control-methods-not-served` becomes version 2, "unknown operations get `method_not_found` before and after negotiation", with a mutant that answers `negotiation_required` before negotiation. |
| D-EXIT | **spec:** STREAM §5 says a provider exits with status 0 after end of input or after closing on a frame-level failure. **fixture:** the runner's `stop` step requires exit status 0. |

## C. Open questions — decisions

| Tag | Decision (spec unless noted) |
|---|---|
| D-STEP1 | Step 1 uses the JSON-RPC `method`. Method/operation equality is the first check of step 2, after limits. |
| D-NEG-AGAIN | `already_negotiated` is decided after steps 1–3. |
| D-NEG-RETRY | A refused negotiation leaves the session unnegotiated and may be retried. **fixture** plus mutant. |
| D-NEG-CORE | A `core` entry is always required, whatever its `required` flag. |
| D-NEG-OPTREQ | An optional profile with a missing required feature is unselected, with one `unknown_feature` entry per feature; it is not a refusal. **fixture.** |
| D-NEG-FEATPREFIX | A feature named under a different profile is `unknown_feature` for the profile it was listed under. |
| D-NEG-DUP | The same profile listed twice is `invalid_envelope`. **reference.** **fixture.** |
| D-NEG-UNSAT | `unsatisfied` lists only the items that caused the refusal. |
| D-NEG-LIMITS | Negotiated `limits` are the provider's receive limits. A response that would exceed the caller's receive limit is replaced by `internal_error` (the outcome may be unknown). **deferred:** no fixture until a profile can produce responses that large (M4 packets). |
| D-FRAME-NEG | The post-negotiation frame limit applies to every frame after the negotiate request. **fixture:** a 1.5 MiB frame is refused before negotiation and accepted after negotiating a 2 MiB limit, with a mutant that never raises the limit. |
| D-SHA512 / D-DIGEST-ALG | sha512 digests are accepted only when `core.digest-sha512` is negotiated; otherwise `unsupported_digest_algorithm`. The length check for a known algorithm is step 2. A retransmission under another algorithm is `idempotency_conflict`. |
| D-DIGEST-EXPECTED | `digest_mismatch.details.expected` is the provider's recomputed digest under the caller's algorithm. |
| D-LIMIT-ORDER | Inside step 2: limits (depth, array items, string bytes, payload bytes), then method/operation equality, then closed objects and types, then envelope semantics. |
| D-ORDER-7 | Capabilities, then authority epoch, then preconditions. **fixture:** a stale epoch plus a failing precondition gives `stale_authority_epoch`, with a mutant that checks preconditions first. |
| D-PRE-PRIMARY / D-CLAIM-PRE | A missing primary-subject precondition is `invalid_envelope`. **fixture** plus mutant. |
| D-PRE-DUP / D-PRE-KIND | Duplicate precondition subjects are `invalid_envelope` (the reference's rule, now stated). A kind the provider does not own is a nonexistent subject. |
| D-DEDUPE-FILE | Records are filed under the command's own `dedupe_generation`. `g > current` is checked before the record lookup. |
| D-DEDUPE-INIT | The initial window is provider-chosen; fixtures must capture it, never assume it. |
| D-NOTIFY | An ID-less object with `"jsonrpc": "2.0"` and a string `method` is a notification and is ignored. Any other ID-less object is `invalid_request` with `id: null`. **reference:** previously ignored every ID-less object. **fixture** plus mutant. |
| D-RPC-SHAPE | A missing or non-object `params`, extra members, a wrong `jsonrpc` or a bad `method` is `invalid_request` echoing a valid ID. A boolean, object, empty-string or out-of-range ID is `invalid_request` with `id: null`. **fixture** plus mutant. |
| D-STREAM-CLOSE | "Close" on stdio: write the error, flush for at most one second, close output, exit 0 without reading more input. |
| D-UTF8-SURR | Encoded surrogates are `invalid_utf8`; raw noncharacters and a leading BOM are `parse_error`. **fixture:** noncharacter and lone-surrogate-escape frames close with `parse_error`, with mutants. |
| D-EXT-DROP | `requires` is checked for queries too. **fixture** plus mutant. |
| D-TESTPROFILE | `core-test` is exposed only when a provider is launched with a conformance configuration. |
| D-UNSUPPORTED-LIST | The listed unsupported profiles are a minimum. |
| D-REQUIRES-PRENEG | A feature in `requires` before negotiation is `unsupported_required_feature`. |

## D. Unguarded requirements — new fixtures

| Deviation that passed | Fixture now guarding it |
|---|---|
| Limit comparison `>=` instead of `>`; `max_payload_bytes`, `max_array_items` not enforced | `core.envelope.limits-at-and-over-boundary` |
| Frame limit never raised after negotiation | `stream.frame-limit-raised-after-negotiation` |
| `precondition_failed` lists only the first failure | `core.preconditions.all-failures-listed` |
| Preconditions checked before the epoch | `core.authority.epoch-checked-before-preconditions` |
| Noncharacters, lone surrogate escapes or `-0` accepted | `stream.noncharacter-closes`, `stream.lone-surrogate-escape-closes`, `stream.negative-zero-closes` |
| A failed negotiation blocks retry | `core.negotiation.refused-negotiation-can-retry` |
| `requires` uniqueness not enforced; not checked on queries | `core.envelope.requires-unique`, `core.envelope.query-requires-checked` |
| Put without a primary precondition accepted | `core.preconditions.primary-precondition-required` |
| Unknown operation before negotiation gets `negotiation_required` | `stream.control-methods-not-served` version 2 |
| Unknown JSON-RPC members, boolean or empty IDs accepted | `stream.malformed-requests-invalid` |
| Negotiation precedence reversed | `core.negotiation.error-precedence` |
| `correlation` included in the digest | `core.digest.correlation-excluded` |
| Nonzero exit status | runner `stop` step |
| Deduplication across principals without grants | `core.idempotency.scope-is-per-principal` |
| Records discarded below `current` regardless of retention | `core.dedupe.retain-counts-current-generation` |

**Still unguarded:**
- Selecting the highest common major needs a second major version (M6 compatibility).
- Pipelined requests need a runner step that accepts responses in any order (M3, where execution watch needs it).
- `unavailable` and `internal_error` "nothing bound" semantics need fault injection (M3).
- The limit + 1 buffering and inherited-descriptor rules are not observable over stdio; they stay documented requirements.

## E. M2 features — second independent pass

The same helper extended the independent provider to grants, events and capabilities from the documents at `3b32037` (section E of its [divergence log](../../../conformance/independent/python-core/DIVERGENCES.md)). It passed 107 of 108 fixtures. Its sensitivity check showed that 34 of 35 deliberate violations of stated M2 requirements also passed. One of them was a real disclosure bug in the specification (E-GRANT-EVENT-LEAK). Another, in its own first version, silently lost events when a notification exceeded the caller's receive limit (E-NOTIFY-SIZE). The Protocol session's reference provider had the same class of bug, because it never tracked the caller's limit.

### E.1 Contradictions

| Tag | Resolution |
|---|---|
| E-GAP-TO | **spec:** CORE §16.4 lets the provider choose a gap's `to` anywhere from the last discarded position to the stream head. **fixture:** `core.events.retention-gap-returns-snapshot` version 2 accepts either reading with `any_of`. |
| E-READ-LIMIT | **spec:** `limit` is required, as the schema says. |
| E-GRANT-FIELD | **spec:** CORE §5.1 separates fields (`invalid_envelope`) from operations (`unsupported_required_feature`) of an unselected feature. |
| E-FEATURE-OP-STEP | **spec:** an operation's own params are validated at step 2 as usual, and step 3 then refuses it. |
| E-FILTERED | **spec:** `filtered` is `true` exactly when something in the covered range was hidden. **reference** plus **fixture** `core.events.unfiltered-read-under-grant` with mutant `filtered-always-under-grant`. |
| E-GRANT-EVENT-LEAK | **spec:** CORE §16.6 now shows an event or snapshot subject under a grant only if the principal could read it directly: profile subjects need the profile's read right, `core.grant` subjects follow `core.grant.get`. **reference** plus **fixtures** `core.events.authorization-and-filtering` version 2 (mutant `events-ignore-subject-read`), `core.events.retention-snapshot-is-filtered` and `core.events.subscription-applies-kinds-and-grant`. |

### E.2 Unstated expectations

| Tag | Resolution |
|---|---|
| E-SNAPSHOT-STATE | **spec:** CORE §16.4 defines `state` per subject kind and says `subjects` lists every visible subject changed by an event at or before `as_of`; a provider may also list subjects no event changed. |
| E-CAP-EVIDENCE | **spec:** the revision rises when a predicate's name set, status, enforcement or `evidence.source` changes, not for `observed_at` alone. **Still unguarded:** evidence sources are provider-chosen, so no fixture can require a revision change for a source change. |
| E-CURSOR-REASON | **spec:** `invalid_cursor` `details.reason` is informative and unspecified. |
| E-FEATURE-CLAIM-GATING | Accepted as designed. A provider without `core.grants` that accepts a `grant` field fails `core.envelope.unknown-field-refused`'s closed-object rule, not the gated fixture. |
| E-ARRAY-EXACT | **fixture docs:** the conformance README says object patterns match subsets and array patterns match exactly. |

### E.3 Open questions — decisions

| Tag | Decision (spec unless noted) |
|---|---|
| E-AUTH-WITHOUT-FEATURE | Authorization applies without `core.grants`; a non-authority gets `grant_required`. **fixture** `core.grants.authorization-without-grants-feature` (added in section F; the fixture first named here negotiated `core.grants` and did not test the rule). |
| E-UNPROTECTED | CORE §15.5 lists protected operations; `core.capabilities` and `core.events.unsubscribe` are not protected, and a `grant` field there is validated but not evaluated. |
| E-DENIAL-ORDER | `grant_not_found`, `revoked`, `expired`, `authority_epoch_stale`, `right_missing`, `out_of_scope`. **fixture** `core.grants.denial-reason-order` with mutants `grant-state-before-holder` and `denial-order-scope-first`. |
| E-ISSUE-VALIDATION-STEP | Audience, expiry and binding-scope checks run at step 6. **fixture** `core.grants.issue-replays-after-expiry` with mutant `issue-validation-before-dedupe`. |
| E-ISSUE-PARENT-HOLDER | Only the parent's holder may delegate, authorities included; others get `grant_not_found`. **fixture** `core.grants.delegation-needs-usable-delegable-parent`. |
| E-REVOKE-DENIAL | Neither issuer nor authority: `not_authority`, whether or not the grant exists. Already revoked: `revoked`, decided after that check. **reference** (it re-revoked). **fixture** `core.grants.revoke-needs-issuer-or-authority` with mutants `anyone-may-revoke` and `re-revoke-accepted`. |
| E-GRANT-PRE | Exactly one precondition on the grant, revision `0` for `issue` and at least `1` for `revoke`; otherwise `invalid_envelope` at step 2. **reference** (it reported `precondition_failed`). **fixture** `core.grants.grant-command-precondition-revision` with mutant `grant-precondition-revision-unchecked`. |
| E-REVOKE-CASCADE | `revoked` lists the named grant first, then descendants that were still active, in a provider-chosen order. |
| E-BINDING-SCOPE | An unknown scope is `invalid_envelope`; a later epoch of a known scope is accepted and authorizes once current. The reference had never authorized such grants; the independent implementation had treated unknown scopes as epoch 0. **reference** plus **fixture** `core.grants.authority-binding-scope-must-exist` with mutant `unknown-binding-scope-accepted`. |
| E-RIGHTS-UNKNOWN | Unknown rights and resource kinds are accepted and never match. |
| E-PRE-CURRENT | `current` appears only for subjects the principal may read: always for an authority without a grant, otherwise by the §16.6 visibility rules. **reference** (it always disclosed `current`). **fixture** `core.grants.precondition-current-needs-read` with mutant `current-revealed-without-read`. |
| E-EXPIRY-BOUNDARY | Expired from `expires_at` onward; issuing at the current instant is invalid. **fixture** `core.grants.expiry-instant-is-exclusive` with mutants `expiry-boundary-inclusive` and `issue-at-now-accepted`. |
| E-START-ORDER | **fixture docs:** the conformance README fixes the order `dedupe`, new epoch, retention, capabilities. |
| E-CURSOR-AFTER-HIDDEN | `next_cursor` follows trailing hidden events when a read reaches the end. |
| E-CURSOR-OLD-EPOCH | Adopted as the independent implementation chose. A cursor into an earlier epoch past `vouched_through` receives the `epoch_change`; that is the case epochs exist to report. Only a cursor past the current epoch's head, or from another stream, is `invalid_cursor`. **reference** (it refused such cursors). **launch configuration** `events.unvouched_last`. **fixtures** `core.events.cursor-past-vouched-position-gets-epoch-change` (mutant `closed-epoch-cursor-refused`) and `core.events.cursor-from-another-stream-refused` (mutant `accept-cursor-from-other-stream`). **Still unguarded:** a cursor past the current head cannot be produced without constructing a cursor, which callers must not do. |
| E-GAP-EPOCHS | First deferred as unreachable. That premise was wrong; resolved as F-GAP-EPOCHS-REACHABLE. |
| E-SUB-REAUTH | A lapsed grant ends the subscription with a final `core.events.notify` carrying `"ended": {"reason": "authorization_lost"}`; the notification schema gains `ended`. **reference** plus **fixture** `core.events.subscription-ends-when-grant-stops-authorizing` with mutant `subscription-survives-authorization-loss`. |
| E-NOTIFY-SIZE | Reads return fewer items and notifications are split to fit the caller's receive limit. An item that cannot fit alone makes a read `internal_error` and ends a subscription with reason `item_too_large`; it is never skipped. **reference** (it ignored the caller's limit). **runner:** every received frame must fit the session's advertised receive limit; `$repeat` builds large values. **fixture** `core.events.reads-and-notifications-fit-receive-limit` with mutant `ignore-caller-receive-limit`. |
| E-UNSUBSCRIBE | An unknown subscription is `not_found`. |
| E-EVENTS-WITHOUT-FEATURE | Events are recorded whether or not a session negotiated `core.events`. |
| E-CAP-SCOPE | Only `put` depends on `core-test.writes`. **fixture** `core.capabilities.claim-does-not-depend-on-writes` with mutant `claim-depends-on-writes`. |

### E.4 Unguarded requirements — new fixtures

Each deviation in the helper's E.4 table that passed every fixture now has a reference mutant and a fixture that fails it:

| Deviation that passed | Fixture now guarding it |
|---|---|
| `delegation.allowed: false` ignored; stale parent delegates; non-holder delegates | `core.grants.delegation-needs-usable-delegable-parent` |
| Child outlives its parent or drops its binding | `core.grants.delegation-keeps-expiry-and-binding` |
| Anyone may revoke; `core.grant.get` hidden from the issuer | `core.grants.revoke-needs-issuer-or-authority` |
| Expiry boundary inclusive; issue at the current instant | `core.grants.expiry-instant-is-exclusive` |
| Issue validation before deduplication | `core.grants.issue-replays-after-expiry` |
| State before holder; scope before rights | `core.grants.denial-reason-order` |
| An authority naming a grant is unrestricted | `core.grants.named-grant-restricts-authority` |
| `claim` needs no right; `applied_count` unprotected | `core.grants.claim-and-applied-count-need-rights` |
| `current` disclosed without read authority | `core.grants.precondition-current-needs-read` |
| No `core.grant.issued` events; `core.grant.revoked` only for the target | `core.events.multi-event-command-contiguous` (now declares both mutants) |
| `filtered` always true under a grant | `core.events.unfiltered-read-under-grant` |
| Foreign-stream cursor accepted | `core.events.cursor-from-another-stream-refused` |
| Notification before the command's response | `core.events.subscription-delivers-backlog-then-live` (runner enforces the order) |
| Subscription ignores kinds or grant | `core.events.subscription-applies-kinds-and-grant` |
| Subscribe unauthorized; read under any grant | `core.events.read-and-subscribe-need-events-right` |
| Gap snapshot unfiltered | `core.events.retention-snapshot-is-filtered` |
| Capability checked after preconditions or before authorization | `core.capabilities.checked-after-authorization-before-preconditions` |
| Capability event revision or subject wrong | `core.capabilities.change-raises-revision-and-event` version 2 |
| `claim` depends on writes | `core.capabilities.claim-does-not-depend-on-writes` |
| Duplicate precondition subjects accepted (D-PRE-DUP) | `core.preconditions.duplicate-subject-invalid` |

**Independent implementation status.** These resolutions postdated the helper's second pass. A third pass (section F) brought the implementation level: it passes every applicable fixture.

## F. Third independent pass

The helper re-read the documents at `6c64ae4` before opening any fixture and brought the independent provider level with section E. It passed every applicable fixture (138 pass; 6 socket fixtures do not apply to a stdio participant). Its sensitivity harness then applied 37 deviations from the section E resolutions. 27 passed every fixture; 21 of those break stated requirements.

The most important finding: E-GRANT-EVENT-LEAK was fixed in the text but unguarded against its original form. No fixture put a grant held and issued by others in a reader's range. The helper also found that its own earlier harness counted only `fail`, not `timeout`, so section E's unguarded counts may have been low for subscriptions.

### F.1 Contradictions

| Tag | Resolution |
|---|---|
| F-GRANT-VIS-AUTHORITY | **spec:** CORE §16.6 drops "matching `core.grant.get`". Under a grant, an authority sees `core.grant` events only as holder or issuer, like anyone else; `core.grant.get` still shows it every grant. **fixture** `core.events.visibility-follows-direct-read-authority` with mutant `authority-events-unrestricted-under-grant`. |
| F-AUTH-STEP1 | **spec:** §10 step 1 exempts `core.authenticate` from negotiation, matching §3.1 and §18.2. **fixture** `core.authentication.launch-authenticated-session-refuses-authenticate`, which replaces the held-back stdio fixture and runs over both bindings, with mutant `stdio-authenticate-accepted`. |
| F-GAP-EPOCHS-REACHABLE | **spec:** a gap may span epochs without separate epoch changes inside it. A cursor at or past a closed epoch's vouched end gets the `epoch_change` first, then a gap from sequence 1 of the next epoch. **reference:** it began the gap inside the closed epoch. **fixture** `core.events.gap-spanning-epochs` with mutant `gap-hides-epoch-change`. |
| F-AUTH-WITHOUT-FEATURE-GUARD | **fixture** `core.grants.authorization-without-grants-feature` with mutant `authorization-needs-grants-feature`; the E record is corrected. |

### F.2 Unstated expectations

| Tag | Resolution |
|---|---|
| F-UNVOUCHED | Adopted as the helper chose. **fixture docs:** unvouched events leave the stream; they are never delivered and do not count toward a later `retain_last`; subject state is unchanged. **reference:** retention counted them. No fixture: this defines a test control, not provider behavior. |
| F-UNVOUCHED-CONFIG | **fixture docs:** no effect without `new_epoch_on_start`; a value above the last sequence vouches through 0. |
| F-ENDED-CURSOR | **spec:** the final notification's `next_cursor` is where delivery stopped, never past an item the subscription would still have delivered. |
| F-CREDENTIALS-CONFIG | Agreed: a participant refuses launch keys it does not support. |
| F-CREDENTIAL-SCHEMA | **schema:** `$defs/credential` documents the issued format and says why `core.authenticate` params accept any string. |

### F.3 Open questions — decisions

| Tag | Decision (spec unless noted) |
|---|---|
| F-PRE-CURRENT-NO-GRANT | On `core.grant.*` operations a non-authority may read only grants it holds or issued. **reference:** it already did. |
| F-PRE-CURRENT-FOREIGN | Under a grant, a kind no profile defines is never readable. **reference:** it treated a covering resource as enough. |
| F-CURSOR-OLD-EPOCH-PAST-END | Valid, answered with the `epoch_change`. |
| F-SUB-END-TIMING | Authorization is re-checked after every request on the connection; a lapse ends the subscription even with nothing pending. **fixture** `core.events.subscription-ends-when-grant-revoked` (revocation, with a `kinds` filter that has nothing to deliver) with mutant `subscription-reauth-epoch-only`. |
| F-ISSUE-CHECK-ORDER | `audience`, `expires_at`, then binding scope, before the issuing rules. **reference:** it checked the binding before the expiry. No fixture: only `details.path` differs. |
| F-AUTH-ORDER | Agreed: steps 1–3 apply, then `already_authenticated`; the credential is never examined. |
| F-FILTERED-RANGE | Agreed; it is what §16.4 "Cursors" and "Filtering" say together. **fixture** `core.events.filtered-and-cursor-cover-exact-range` with mutants `filtered-counts-beyond-range` and `cursor-stops-at-last-item`. |
| F-READ-FIT | A read carries at least one item whenever the first item fits; how many more is provider-chosen. |

### F.4 Deviations that passed every fixture — now guarded

| Deviation | Fixture and reference mutant |
|---|---|
| A stranger re-revoking learns `revoked` | `core.grants.revoke-needs-issuer-or-authority` version 2, `revoked-before-issuer-check` |
| Re-revocation checked after preconditions | same fixture (stale revision), `re-revoke-after-preconditions` |
| Already revoked descendants re-revoked | `core.grants.cascade-skips-revoked-descendants`, `cascade-rerevokes-descendants` |
| `current_epoch` always disclosed | `core.grants.current-epoch-needs-read`, `current-epoch-always-disclosed` (**reference** fixed: it always disclosed) |
| `core.capabilities` protected; `grant` field evaluated on unprotected operations | `core.grants.unprotected-operations`, `capabilities-protected`, `grant-field-evaluated-on-unprotected` |
| Authorization skipped without `core.grants` | `core.grants.authorization-without-grants-feature` |
| Every grant event visible to `core.events.read` (the original leak); issuer not shown its grants; authority subject without a covering resource; capability events without coverage or needing `core-test.read`; authority unrestricted under a grant | `core.events.visibility-follows-direct-read-authority`, one mutant each |
| `filtered` ignores hidden snapshot subjects; revoked grant's snapshot state | `core.events.retention-snapshot-is-filtered` version 2, `filtered-ignores-snapshot`, `snapshot-grant-state-only` |
| `filtered` beyond the covered range; `next_cursor` at the last item | `core.events.filtered-and-cursor-cover-exact-range` |
| Subscription survives revocation | `core.events.subscription-ends-when-grant-revoked` |
| `core.authenticate` on stdio unknown, accepted or failing | `core.authentication.launch-authenticated-session-refuses-authenticate` |

**Still unguarded, documented:**
- a cursor past the current head, which cannot be produced without constructing a cursor;
- capability revision on an evidence-source change, since sources are provider-chosen;
- the issue check order, which only changes `details.path`;
- unvouched events and retention, which is test-control semantics.
