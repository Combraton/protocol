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
