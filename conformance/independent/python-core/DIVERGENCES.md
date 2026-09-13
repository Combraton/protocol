# Divergences, ambiguities and unchecked requirements

> Resolutions of these findings are recorded in [M2-DIVERGENCES](../../../docs/work/release-0.1/M2-DIVERGENCES.md).
>
> Findings from writing an independent Core provider using only the published documents (CORE, STREAM, ENCODING, schemas, fixtures, vectors). Protocol 0.1 draft at base commit `f42d21a`. See [README](README.md) for what was and was not read.

## How to read this

Each entry has:

- a tag (`D-…`), which `tests/probe_provider.py` also uses;
- the spec location;
- the question;
- what this implementation does;
- the affected fixtures;
- its **basis**: *spec text*, *schema*, *fixture-informed* (the prose was open and the choice followed a fixture read before coding) or *own judgment*.

Sections:

- **A.** The documents contradict each other.
- **B.** A fixture or the runner expects something the documents do not state.
- **C.** Open questions where this implementation chose, and no fixture decides.
- **D.** Requirements the documents state that no fixture checks, with evidence.
- **E.** The M2 features (grants, events, capabilities), added in a second pass, with its own subsections E.1 to E.4 in the same four categories.

**Changes after fixture failures: none.** The first complete run passed 57 of 57 fixtures. Nothing was bent to fit a fixture.

Other work around the runs:

1. **Before the first run,** one cosmetic reordering: the check that the method equals the envelope's `operation` moved ahead of the closed-object check, so `details.path` names `/operation`.
2. **After the first run,** a temporary diagnostic, since removed, logged the configuration file the runner writes. That confirmed the key set `format` / `principal` / `limits` / `dedupe`.
3. **After the first run,** modified scratch copies of the provider (never the committed code) were run to find out which behaviors the runner enforces (sections B and D).

---

## A. Contradictions between documents

### D-STREAM-FRAC — a fractional request id cannot be `invalid_request`

- **Where:** STREAM §3 says "A null or fractional `id` is `invalid_request`" and the connection stays open. STREAM §1.2 and ENCODING §1.3 say every number is an integer, and ENCODING §1 says violations are rejected "when it parses a frame (STREAM §2)", which closes the connection with `parse_error`.
- **Question:** Is `"id": 900.0` `invalid_request` (stays open) or `parse_error` (closes)?
- **Chosen:** `parse_error`, `id: null`, close. Frame-level rules apply before the JSON-RPC mapping ever sees the object.
- **Fixtures:** `stream.fraction-closes` expects exactly this.
- **Basis:** spec text (ENCODING and STREAM §1–2). The words "or fractional" in STREAM §3 describe a case that cannot occur and should be deleted.

### D-STREAM-ID — string id limit: bytes or code points

- **Where:** STREAM §3 allows "a string of 1–128 bytes". `schemas/stream/1/jsonrpc.schema.json` `request_id` uses `maxLength: 128`, and JSON Schema counts code points.
- **Question:** Is an id of 65 × `é` (130 UTF-8 bytes, 65 code points) valid?
- **Chosen:** bytes, following the prose. The request gets `invalid_request` with `id: null`.
- **Fixtures:** none. `stream.whitespace-blank-frames-and-string-ids` uses a short non-ASCII id.
- **Basis:** spec text. The schema and the prose should agree.
- **Resolution (2026-09-13):** the specification now says code points, matching the schema ([resolution record](../../../docs/work/release-0.1/M2-DIVERGENCES.md)). The Protocol session changed this implementation's one-line check to follow it; no other code was changed.

### D-TESTCTL — test control: a channel or launch configuration?

- **Where:** CORE §13.1 describes a test-control channel that "is a separate connection with its own protocol, served by a test launcher". It "may restart the provider …, advance or discard deduplication generations, drop connections and set retention configuration". `conformance/README.md` says instead: "Restarts and deduplication retention changes happen through process lifecycle and that configuration … There is no control endpoint."
- **Question:** What do the launch configuration keys mean? None of the allowed documents defines `format`, `principal`, `limits`, `dedupe.advance_on_start` or `dedupe.retain_generations`. `docs/decisions/001` is referenced but is not part of the published contract.
- **Chosen:**
  - `advance_on_start: N` moves `current` forward by N at that process start. It applies to that start only, and the advanced window is durable.
  - `retain_generations: R` keeps R generations counting `current`: `oldest_retained = max(oldest_retained, current − R + 1)`. Records whose `dedupe_generation` is below `oldest_retained` are deleted in the same transaction.
  - Without `dedupe`, nothing moves.
  - `limits` partially overrides the defaults.
  - `principal` may be any JSON value; the deduplication scope is its canonical form.
  - Unknown keys are refused.
  - The initial window is `{0, 0}` (D-DEDUPE-INIT).
- **Fixtures:** `core.dedupe.forgotten-generation-unavailable` (advance 2, retain 1), `core.dedupe.retained-generation-replays-after-advance` (advance 1, retain 5), both limit fixtures, and every fixture through `principal`.
- **Basis:** fixture-informed. Both fixtures also pass under the "R generations behind current" reading (`current − R`), so the exact meaning of `retain_generations` is unchecked. Treating `retain_generations` as "keep only current" is caught (see D).

### D-RESULT-SUBJECT — core-test query results carry `subject`

- **Where:** CORE §13 says `core-test.subject.get` "returns `{ "revision": n, "value": … }`" and `applied_count` "returns how many commands changed the subject". `schemas/core-test/1/*.result.schema.json` require a `subject` member in both results and close the objects.
- **Question:** Which shape is right?
- **Chosen:** the schema shape.
- **Fixtures:** all 32 fixtures that query. Black-box check: a scratch copy returning the prose shape fails 32 fixtures with `"subject" is a required property`.
- **Basis:** schema. A provider written from CORE.md alone fails most of the suite. CORE §13 should show the full result.

### D-MANIFEST-FIELDS — manifest members named only by the schema

- **Where:** CORE §4.1 lists the manifest contents as provider, profiles with majors and features, unsupported profiles, limits and the deduplication window. `core.describe.result.schema.json` also requires `unknown_extensions` (mentioned only as a possibility in §5.1), `depends_on` for each profile (implied by REL-2) and a `reason` enum `not_in_release` / `not_implemented` for unsupported profiles.
- **Chosen:** the schema. `unknown_extensions: "drop"`; `depends_on: ["core"]` for `core-test`; reason `not_in_release`.
- **Fixtures:** `core.describe.manifest-declares-profiles-and-unsupported` and the stream fixtures that call describe. A scratch copy without `unknown_extensions`, or one without `depends_on`, fails 4 fixtures on schema validation.
- **Basis:** schema.

---

## B. Expectations in fixtures or the runner that the documents do not state

### D-ERR-RETRY — retry class of binding-level errors

- **Where:** STREAM §3 says "The `data` object always has the members `code`, `retry` and `details`". CORE §12's table gives retry classes only for Core codes. The STREAM tables give a retry only for `overloaded`. No document gives the retry or `details` for `parse_error`, `invalid_utf8`, `frame_too_large` or `invalid_request`.
- **Chosen:** `retry: "no"` and `details: {}` for all four. This was a guess made before the first run.
- **Fixtures:** the runner enforces `no`. A scratch copy using `same_command` fails 9 fixtures: every `stream.*-closes` fixture plus the two `invalid_request` fixtures ("error parse_error must have retry no").
- **Basis:** own judgment, which happened to match. The rule belongs in STREAM §2 and §3.

### D-LIMIT-SCOPE — what a limit is measured over

- **Where:** CORE §9 defines `max_depth` as "deepest nesting of objects and arrays" and `max_string_bytes` as "longest string value". CORE §10 step 2 says "Envelope and payload decode within limits".
- **Questions:**
  1. Do limits cover the whole envelope, including `extensions` and `correlation`, or only `payload`?
  2. Is depth counted from the JSON-RPC frame root or from `params`?
  3. Does the root count as level 1, and does a scalar count as a level?
  4. Do member names count as strings?
  5. Do the JSON-RPC `id` and `method` count?
- **Chosen:**
  1. The whole `params` envelope.
  2. From `params`.
  3. `params` is depth 1; scalars add nothing.
  4. Member names count.
  5. `id` and `method` are not counted; the binding bounds them.
- **Fixtures:** `core.envelope.depth-limit-exceeded` hides 8 nested arrays in an optional unknown extension under `max_depth: 6`, so it requires question 1 to be "whole envelope". That depth is 10 counted from `params` and 11 from the frame root, so the counting convention is untested. `core.envelope.string-limit-exceeded` uses a 129-byte value against 128, so the member-name question is untested.
- **Basis:** spec text ("a message … that exceeds another limit"), confirmed by the fixture.

### D-APPLIED-ABSENT — `applied_count` for a subject that never existed

- **Where:** CORE §13 gives `not_found` only for `get`.
- **Chosen:** `{subject, applied_count: 0}`.
- **Fixtures:** `stream.notification-not-processed`, `core.errors.query-rejection-records-nothing` and others expect 0.
- **Basis:** fixture-informed. It is also the natural reading of "how many commands changed the subject".

### D-CTL-NAME — the refused control-channel method names are not specified

- **Where:** CORE §13.1 says "A provider's product endpoint MUST refuse control-channel method names with `method_not_found`". No document names those methods, and `conformance/README.md` says there is no control endpoint.
- **Chosen:** every unknown operation gets `method_not_found`.
- **Fixtures:** `stream.control-methods-not-served` sends `control.restart`, a name that appears only in the fixture. It shows that one unknown name is refused, which every provider already does.
- **Basis:** spec text. The requirement cannot fail independently of "unknown method".

### D-EXIT — exit status

- **Where:** STREAM §5 says the provider exits at end of input. STREAM §2 says it closes after a frame-level failure. Neither gives an exit status.
- **Chosen:** 0 in both cases (exit status 2 only for a bad configuration).
- **Fixtures:** not checked. Scratch copies exiting 3 at end of input, or 4 after a frame-level failure, still pass all 57.
- **Basis:** own judgment.

---

## C. Open questions decided here, not decided by any fixture

### D-STEP1 — Which name does step 1 check?

- **Where:** CORE §10 step 1 says "operation known". STREAM §3 says the method "MUST equal the envelope's `operation`. A mismatch is `invalid_envelope`", but gives no step.
- **Question:** Is step 1 applied to the JSON-RPC `method` or to `params.operation`? When is the equality checked? Example: method `nope.op` with operation `core.describe` could get `method_not_found` or `invalid_envelope`.
- **Chosen:** step 1 uses `method`. The equality is checked in step 2, right after the limits and before the closed-object checks (D-LIMIT-ORDER). So a limit-exceeding or malformed message to an unknown method gets `method_not_found`.
- **Fixtures:** `stream.method-operation-mismatch-refused` passes under either order.
- **Basis:** own judgment.

### D-NEG-AGAIN — Where does `already_negotiated` sit in the order?

- **Where:** CORE §3.2 and §10. For a query, §10 says "steps 1–3 and then the operation's read rules". `core.negotiate` is labelled a query (§4.2) but changes session state.
- **Chosen:** after steps 1–3. A second negotiate with an invalid payload gets `invalid_envelope`; a valid one gets `already_negotiated`.
- **Fixtures:** `core.negotiation.selects-core-and-requested-profile` sends a valid second payload, so it is neutral.
- **Basis:** spec text (literal query order).

### D-NEG-RETRY — Can a failed negotiation be retried?

- **Where:** CORE §3.2 says negotiation "succeeds at most once per session".
- **Chosen:** yes. A refused negotiation leaves the session unnegotiated.
- **Fixtures:** none retries. A scratch copy that answers `already_negotiated` after a failed attempt passes all 57.
- **Basis:** spec text.

### D-NEG-CORE — Core listed explicitly as optional

- **Where:** CORE §4.2 says "Include `core` implicitly; a caller cannot deselect it."
- **Question:** If the caller lists `core` with `required: false` and no common major, is that unselected or a refusal?
- **Chosen:** a `core` entry is always treated as required, so this is a refusal (`unsupported_version`).
- **Basis:** own judgment.

### D-NEG-OPTREQ — optional profile whose required feature is missing

- **Where:** CORE §4.2 says "Refuse the whole negotiation if a `required` profile cannot be selected or a required feature is missing". It also says "Optional profiles and features that cannot be selected are reported in `unselected`."
- **Question:** Does a missing `required_features` entry on a `required: false` profile refuse the whole negotiation?
- **Chosen:** no. The optional profile is not selected, and each missing feature is reported in `unselected` as `{profile, feature, reason: unknown_feature}`. The profile's absence from `selected` tells the caller it was dropped.
- **Basis:** own judgment. The sentence supports both readings.

### D-NEG-FEATPREFIX — feature named under another profile

- **Where:** CORE §5.1 says a feature name is `<profile>.<feature>`.
- **Question:** What happens to `core.digest-sha512` listed under a `core-test` entry?
- **Chosen:** it is not a feature of that profile, so `unknown_feature`. Such a name is never `invalid_envelope`.
- **Basis:** own judgment.

### D-NEG-DUP — the same profile listed twice

- **Where:** `core.negotiate.params.schema.json` does not forbid two entries with the same `name` and different contents.
- **Chosen:** `invalid_envelope` with path `/payload/profiles/<i>/name`, treating it as an impossible value (CORE §12). This is the only rule here stricter than the schemas.
- **Basis:** own judgment.

### D-NEG-UNSAT — contents and order of `unsatisfied`

- **Where:** CORE §4.2 says "`details.unsatisfied` lists every unsatisfied item".
- **Chosen:** only the items that caused the refusal, from required profiles and Core, in request order with Core first. Unsatisfied optional items are not listed, because an error has no `unselected`.
- **Fixtures:** the negotiation fixtures use `$contains`, which fits either reading.
- **Basis:** own judgment.

### D-NEG-DEP — dependencies

- **Where:** CORE §4.1 (REL-2) and §4.2 reason `dependency_not_selected`.
- **Question:** Does the provider select a dependency the caller did not request?
- **Chosen:** Core is always selected. The only dependency in this provider is `core-test → core`, so `dependency_not_selected` is unreachable here. The dependency check runs anyway.
- **Basis:** spec text for Core. Unspecified in general.

### D-NEG-LIMITS / D-FRAME-SEND — effective limits and the caller's receive limit

- **Where:** CORE §4.2 ("effective limits") and STREAM §1.5.
- **Questions:** Are the `limits` in the result the provider's own limits, or combined with the caller's `receive_limits`? What does a provider do when a response would exceed the caller's receive limit?
- **Chosen:** the result reports the provider's limits. A response larger than the caller's limit is replaced by `internal_error` (`retry: after_reconcile`). For a command that was bound, the outcome is then unknown to the caller. This never happens with core-test.
- **Basis:** own judgment. The documents do not cover the send-side case.

### D-FRAME-NEG — when the post-negotiation frame limit applies

- **Where:** STREAM §1.5 says "Until negotiation completes on a session, the limit is 1,048,576 bytes".
- **Question:** Do frames the caller pipelined before reading the negotiate response use the old or the new limit?
- **Chosen:** the new limit applies to every frame after the negotiate frame. Requests are processed in order. Probes confirm that a 1.5 MiB frame closes the connection before negotiation and is accepted after negotiating a 2 MiB limit.
- **Fixtures:** none sets `max_frame_bytes` (see D).
- **Basis:** own judgment on timing; spec text otherwise.

### D-SHA512 / D-DIGEST-ALG — the optional algorithm

- **Where:** ENCODING §3 says sha512 is "Optional; advertised as Core feature `core.digest-sha512`". CORE §5.1 says "A caller MUST NOT send fields that belong to a feature the session did not select". CORE §6.2 says "same `command_digest`".
- **Questions:** Is a sha512 digest accepted in a session that did not select the feature? Is a 64-hex `sha512:` string `invalid_envelope` (known algorithm, wrong length) or `unsupported_digest_algorithm`? Is the same intent retransmitted under the other algorithm a replay or a conflict?
- **Chosen:**
  - sha512 is supported only when negotiated; otherwise `unsupported_digest_algorithm` with `supported: ["sha256"]`.
  - The length check for the known algorithms sha256 and sha512 always applies, at step 2.
  - A different algorithm gives a different `command_digest`, so `idempotency_conflict`.
- **Basis:** spec text for the conflict (a literal string comparison); own judgment for the gating.

### D-DIGEST-EXPECTED — meaning of `digest_mismatch.details.expected`

- **Chosen:** the provider's recomputed digest, under the algorithm the caller named.
- **Basis:** own judgment. The name could also be read as the caller's value.

### D-LIMIT-ORDER — order of checks inside step 2

- **Where:** CORE §10 step 2 lists `limit_exceeded` and `invalid_envelope` without an order.
- **Chosen:**
  1. limits: depth, array items, string bytes, then canonical payload bytes (depth first, so no later pass sees unbounded nesting);
  2. then method equals operation;
  3. then closed objects and types.
- **Consequence:** schema-fixed bounds such as `preconditions.maxItems: 16` and `requires.maxItems: 64` give `invalid_envelope`. The declared `max_array_items` gives `limit_exceeded`, and it wins when both apply.
- **Basis:** own judgment.

### D-ORDER-7 — authority epoch before preconditions

- **Where:** CORE §10 step 7 says "Authority epoch (§8) and preconditions (§7)".
- **Chosen:** epoch first. A stale epoch together with a failing precondition gives `stale_authority_epoch`.
- **Fixtures:** none combines them. A scratch copy checking preconditions first passes all 57.
- **Basis:** own judgment (listing order).

### D-PRE-PRIMARY / D-CLAIM-PRE — mandatory precondition entries

- **Where:** CORE §13 says "The primary subject's precondition entry is mandatory for `core-test.subject.put`" and the claim takes a "precondition on the authority subject's revision". The schemas require only `minItems: 1` for put, and exactly one entry for claim.
- **Question:** Which error applies when the entries name other subjects?
- **Chosen:** `invalid_envelope` for both.
- **Fixtures:** none. A scratch copy without the put rule passes all 57.
- **Basis:** spec text for the rule; own judgment for the error code.

### D-PRE-DUP / D-PRE-KIND — unusual precondition entries

- **Chosen:**
  - Duplicate entries are evaluated one by one, so two identical entries are fine and two contradictory ones fail.
  - An entry naming a kind this provider does not own is a subject that does not exist (revision 0).
  - `failed` lists every failing entry in request order, with `current: 0` for an absent subject.
- **Basis:** spec text (CORE §7 "checked together") for duplicates; own judgment for foreign kinds.

### D-DEDUPE-FILE — which generation a record belongs to

- **Where:** CORE §6.3 says "records of every bound command issued under a generation ≥ `g_old`".
- **Chosen:**
  - A record is filed under the command's own `dedupe_generation`, not the provider's `current` at bind time.
  - The lookup follows the table order literally. `g > current` gives `invalid_envelope` even when a record exists.
  - A retransmission carrying a different retained generation still replays, because the generation is not part of the digest.
- **Basis:** spec text.

### D-DEDUPE-INIT — initial window

- **Chosen:** `{oldest_retained: 0, current: 0}`.
- **Basis:** own judgment; the documents give no starting value.

### D-NOTIFY — an object without `id` that is not a request

- **Where:** STREAM §3 says notifications ("requests without `id`") are not processed and get no reply.
- **Chosen:** an id-less object with `"jsonrpc": "2.0"` and a string `method` is a notification and is ignored silently. Any other id-less object gets `invalid_request` with `id: null`.
- **Basis:** own judgment, following JSON-RPC 2.0's definition of a Request object.

### D-RPC-SHAPE — malformed request objects

- **Where:** STREAM §3, and the `request` definition in the jsonrpc schema (`params` required, closed object).
- **Chosen:** a missing or non-object `params`, extra top-level members, a wrong `jsonrpc`, or a missing or over-long `method` gets `invalid_request`, echoing the id when the id is valid. An id that is boolean, an object, an empty string or out of range gets `invalid_request` with `id: null`. JSON-RPC 2.0 itself allows `params` to be omitted.
- **Fixtures:** only `id: null` and top-level arrays are tested.
- **Basis:** schema.

### D-STREAM-CLOSE — "closes the connection" on stdio

- **Where:** STREAM §2.
- **Chosen:** write the id-null error on a writer thread and wait at most 1 s. Then close stdout and exit without reading further input. The runner's follow-up write after `frame_too_large` may therefore meet a closed pipe.
- **Basis:** own judgment; the stdio form does not say what closing means for the process.

### D-UTF8-SURR — raw surrogates and BOM

- **Where:** STREAM §1.2 says "no unpaired surrogates or noncharacters, escaped or raw". STREAM §2 separates `invalid_utf8` from `parse_error`.
- **Chosen:**
  - UTF-8-encoded surrogates (such as `ED A0 80`) are not valid UTF-8 (RFC 3629), so they get `invalid_utf8`.
  - Raw noncharacters, which are valid UTF-8, get `parse_error`.
  - A frame starting with a BOM gets `parse_error`; U+FEFF is not JSON whitespace.
- **Basis:** own judgment on the classification.

### D-EXT-DROP — optional extensions

- **Where:** CORE §5.1 says the receiver "MUST either store the extension unchanged with that content or declare in its manifest that it drops unknown extensions".
- **Chosen:** declare `drop`. An extension key in `requires` always gets `unsupported_required_feature`, because no extension is understood. `requires` is checked for queries as well as commands.
- **Basis:** spec text.

### D-TESTPROFILE — "outside test configurations"

- **Where:** CORE §13 says providers "MUST NOT expose [core-test] outside test configurations".
- **Chosen:** the conformance launch configuration is mandatory, so core-test is always exposed. The documents define no signal that marks a test configuration.
- **Basis:** own judgment.

### D-UNSUPPORTED-LIST — which names to declare unsupported

- **Where:** CORE §4.1 says the list "includes `coordination` and `remote-trust`".
- **Chosen:** exactly those two, reason `not_in_release`. Other product profile names are unknown to this provider, so they get `unknown_profile`, not `declared_unsupported`.
- **Basis:** spec text. "Includes" leaves the full list open.

### D-REQUIRES-PRENEG — `requires` before negotiation

- **Chosen:** `core.describe` or `core.negotiate` with a feature name in `requires`, sent before negotiation, gets `unsupported_required_feature`, because nothing is negotiated yet.
- **Basis:** spec text (CORE §10 steps 1–3 apply to every query).

---

## D. Requirements the documents state that no fixture checks

`tests/fixture_sensitivity.py` applies each deviation below, one at a time, to a temporary copy of this provider and runs the full suite with the black-box runner.

| Deviation applied | Requirement it breaks | Fixtures failing |
|---|---|---|
| Limit comparison `>=` instead of `>` | CORE §9: a value at the limit is within it. Only the frame limit has an at-limit fixture. | 0 |
| `max_payload_bytes` not enforced | CORE §9, CORE-5 | 0 |
| `max_array_items` not enforced | CORE §9, CORE-5 | 0 |
| Frame limit never raised after negotiation | STREAM §1.5 (no fixture configures `max_frame_bytes`) | 0 |
| `precondition_failed` lists only the first failure | CORE §7: "The error lists every failed entry" | 0 |
| Preconditions checked before the authority epoch | D-ORDER-7 (open question) | 0 |
| Noncharacters accepted in strings | ENCODING §1.2, STREAM §1.2 (vectors only, not over the wire) | 0 |
| Lone surrogate escapes accepted | ENCODING §1.2 | 0 |
| `-0` accepted | ENCODING §1.3 | 0 |
| A failed negotiation blocks a later one | CORE §3.2 "succeeds at most once" | 0 |
| `requires` uniqueness not enforced | CORE §5.1 → `invalid_envelope` | 0 |
| `requires` not checked on queries | CORE §10 "A query applies steps 1–3" | 0 |
| Put without a primary-subject precondition accepted | CORE §13 | 0 |
| Unknown operation before negotiation gets `negotiation_required` | CORE §3.1 "an unknown one gets `method_not_found`" | 0 |
| Unknown JSON-RPC request members accepted | jsonrpc schema (closed `request`) | 0 |
| Boolean or empty-string ids accepted | STREAM §3 | 0 |
| Negotiation error precedence reversed (`unsupported_version` before `unsupported_profile`) | CORE §4.2 code order | 0 |
| `correlation` included in the command intent | CORE §6.1: excluded from the digest | 0 |
| Provider exits with status 3 | none (D-EXIT) | 0 |
| Unsupported optional feature reported as selected | CORE §4.2 | 1 (`core.negotiation.unknown-optional-feature-unselected`) |
| Records discarded below `current` regardless of retention | CORE §6.3 | 1 (`core.dedupe.retained-generation-replays-after-advance`) |
| Binding-level errors use `retry: same_command` | not in the documents (D-ERR-RETRY) | 9 |

More requirements no fixture can reach with one launch configuration per start:

- **Deduplication across principals (CORE §6.2, CORE-12).** A fixture could restart with a different `principal` and reuse a `command_id`.
- **Selecting the highest common major (CORE §4.2).** Only major 1 exists.
- **STREAM §5 inherited descriptors,** and STREAM §1.5 "MUST NOT buffer more than limit + 1 bytes".
- **Pipelined requests.** No fixture sends a second frame before reading the first response.
- **`unavailable` and `internal_error`** "nothing was bound" semantics.

Taken together: once the reference provider passes, the M1 suite mostly pins the happy path and one refusal per error code. The boundaries, the "every"/"all" quantifiers and the check orderings above are unguarded, both for this provider and for the reference provider's mutants.

---

## E. M2 features (grants, events, capabilities)

> Added when this implementation was extended to `core.grants` (CORE §15), `core.events` (§16) and `core.capabilities` (§17), from the documents at base commit `3b32037`. The same reading rules applied: CORE, STREAM, ENCODING, `schemas/**`, the conformance README and schemas, VERIFICATION, [M2-DIVERGENCES](../../../docs/work/release-0.1/M2-DIVERGENCES.md), fixtures and vectors. `conformance/reference/**`, `conformance/crosscheck/**` and `conformance/runner/src/**` were not read. As in M1, the fixtures were read **before** the code was written, so a basis of *fixture-informed* means the prose was open and the fixture picked the answer.

Entries use the same format as sections A–D, with tags `E-…`; `tests/probe_m2.py` asserts each chosen behavior under its tag.

**Changes after fixture failures: none.** The first complete run with the features claimed passed 107 of 108 fixtures. The one failure, `core.events.retention-gap-returns-snapshot`, was predicted from reading the fixture before coding and is recorded as E-GAP-TO instead of being worked around. The suite now exits 1 for this participant because of it.

Other changes in this pass, not driven by a fixture:

1. **D-PRE-DUP applied.** CORE §7 now says "Two entries naming the same subject are `invalid_envelope`" (resolution in M2-DIVERGENCES). This implementation still accepted duplicates, and all 108 fixtures passed with that behavior. It now refuses them. E.4 records that the old behavior is still unguarded (deviation "duplicate precondition subjects accepted" in `fixture_sensitivity.py m1`: 0 failing).
2. **The D-STREAM-ID probe** expected the pre-resolution byte count. It now expects the resolved code-point count; the provider's check had already been changed by the Protocol session.
3. **Launch configuration `principal`** must now be a string of 1–128 characters, as `conformance/schemas/launch-config.schema.json` says. D-TESTCTL had accepted any JSON value. Grant holders are compared with it as strings.

### E.1 Contradictions between documents

#### E-GAP-TO — does a retention gap end at the last discarded event or at the head?

- **Where:** CORE §16.4 says a gap item means "Events between positions `from` and `to` were discarded under retention", with `snapshot.as_of` equal to `to`, and "Reading continues after `to`". The conformance README defines `events.retain_last` as "discard all but the newest N events".
- **The fixture:** `core.events.retention-gap-returns-snapshot` records four events, restarts with `retain_last: 1`, and reads from the start. By the README event (1,4) survives. The fixture expects exactly one item, a gap with `to` = `as_of` = (1,4), and the next read to begin at (1,5). So the retained event is reported inside a range described as discarded and is never delivered to this reader.
- **Chosen:** the literal reading. The gap runs from (1,1) to (1,3), its snapshot is as of (1,3), and event (1,4) follows as an ordinary item. Recorded transcript: `items` holds the gap to `{"epoch":1,"sequence":3}` and then event 4. The runner stops at "step 9: result/items: expected 1 items, found 2".
- **Evidence:** the deviation "gap snapshot and `to` at the stream head" in E.4 (fold every stored event into the snapshot and end the gap at the head) makes this fixture pass and breaks no other.
- **Why the two readings diverge in practice:** a snapshot at the head needs only current subject state. A snapshot at the discard boundary needs historical state, so this implementation folds discarded events into a stored base snapshot when it discards them. A provider that keeps only current state can only produce the head reading, which may be how the fixture's expectation arose.
- **Basis:** spec text. Either CORE §16.4 should allow a gap to extend over retained events ("positions `from` through `to` are not delivered individually"), or the fixture should use `retain_last: 0`, or expect a gap to (1,3) followed by event 4.

#### E-READ-LIMIT — is `limit` optional?

- **Where:** CORE §16.4 says "plus optional `kinds` … and `limit` (1–1000 items)". `core.events.read.params.schema.json` lists `limit` in `required`.
- **Chosen:** the schema. A read without `limit` is `invalid_envelope`.
- **Fixtures:** none; every fixture read sends `limit`.
- **Basis:** schema (the stricter reading). The two documents should agree.

#### E-GRANT-FIELD — refusing a field of an unselected feature

- **Where:** CORE §5.1's closed-object rule says an undefined field is `invalid_envelope`, and the §5 table says `grant` is "Allowed only when feature `core.grants` is negotiated". The last bullet of §5.1 says "A caller MUST NOT send fields that belong to a feature the session did not select, or call operations that belong to one; such operations are refused with `unsupported_required_feature`". That can be read as covering fields too.
- **Chosen:** `invalid_envelope` for a `grant` field in a session without `core.grants`.
- **Fixtures:** `core.grants.grant-field-requires-feature` expects this.
- **Basis:** spec text, confirmed by the fixture. The last bullet of §5.1 should say that fields are `invalid_envelope` and operations are `unsupported_required_feature`.

#### E-FEATURE-OP-STEP — step 2 would pre-empt step 3 for feature operations

- **Where:** read literally, CORE §5.1 ("A field that the negotiated profile version and features do not define makes the message invalid") makes every payload member of `core.events.read` undefined when `core.events` was not selected. That gives `invalid_envelope` at step 2, so step 3's rule "if the operation belongs to a feature … that feature is selected" could never apply to an operation with a non-empty payload.
- **Chosen:** an operation's own payload is validated against its schema whether or not its feature was selected. Step 3 then refuses it with `unsupported_required_feature` and `details.features` naming the feature.
- **Fixtures:** `core.events.feature-must-be-negotiated` expects this.
- **Basis:** step 3 text plus the fixture. §5.1 should exempt the payload of a known operation from the feature condition.

#### E-FILTERED — is `filtered` always true under a grant?

- **Where:** CORE §16.4: "When authorization or `kinds` hides some events, `filtered` is `true`" (conditional). CORE §16.6: "Under a grant, only events and snapshot subjects covered by its resources are included, and `filtered` is `true`" (unconditional).
- **Chosen:** always `true` under a grant; for `kinds`, `true` only when an event or snapshot subject in the scanned range was actually hidden.
- **Fixtures:** `core.events.authorization-and-filtering` has hidden events, so both readings pass (E.4).
- **Basis:** spec text (§16.6 is the more specific rule).

#### E-GRANT-EVENT-LEAK — grant records visible through events but not through `core.grant.get`

- **Where:** CORE §15.3: `core.grant.get` is "Visible to its holder, its issuer and authority principals. Anyone else gets `not_found`". CORE §16.6: under a grant with right `core.events.read`, events "covered by its resources" are included. §16.2 gives `core.grant.issued` the payload `{ "grant": record }`.
- **Consequence:** a holder of `core.events.read` on `{ "kind": "core.grant" }` reads every grant record — other principals' holders, rights and resources — from `core.grant.issued` events and gap snapshots, although `core.grant.get` answers `not_found` for the same grants. Likewise `core.events.read` alone exposes `core-test.subject` values without `core-test.read`.
- **Chosen:** the literal §16.6 rule; event visibility depends only on resource coverage.
- **Fixtures:** none reads grant events under a grant.
- **Basis:** spec text. This is probably unintended: §16.6 should also require the subject's read rule, or define `core.grant` event visibility the way §15.3 does.

### E.2 Expectations in fixtures or the runner that the documents do not state

#### E-SNAPSHOT-STATE — the shape of a snapshot subject's `state`

- **Where:** CORE §16.4 defines `snapshot.subjects[]` as `{ subject, revision, state }` and the schema allows any object. No document defines `state` for any subject kind, or which subjects a snapshot lists.
- **Chosen:** `state` is what a reducer over that subject's events would hold:
  - `core-test.subject` → `{ "value" }`;
  - `core-test.authority` → `{ "epoch" }`;
  - `core.grant` → `{ "grant": record }`, where a `core.grant.revoked` event updates `grant.state`;
  - `core.capabilities` → `{ "predicates" }`.

  The snapshot lists the visible subjects that had at least one discarded event. A subject that never had an event, such as a new store's revision-1 capability snapshot, is not listed.
- **Fixtures:** `core.events.retention-gap-returns-snapshot` expects `state: {"value": "b"}` for a `core-test.subject`.
- **Basis:** fixture-informed for `core-test.subject`; own judgment for the rest.

#### E-CAP-EVIDENCE — evidence content and stability across restarts

- **Where:** CORE §17.1 says `evidence` is `{ source, observed_at? }`, and `revision` "increases whenever any predicate's status, enforcement or evidence changes. It is stable across restarts when nothing changed."
- **Chosen:**
  - Without a configured status, `core-test.writes` is `supported` with source "store write transaction committed at provider start". That is real evidence: every start commits a transaction.
  - With a configured status, the source is "conformance launch configuration".
  - `observed_at` is omitted. A start-time instant would change the evidence, and so the revision, on every restart.
  - `enforcement` is omitted: none of the PIO levels describes the provider's own store.
  - A status configured as `supported` differs in evidence from the default, so it raises the revision (probe E-CAP-EVIDENCE).
- **Fixtures:** `core.capabilities.snapshot-reports-evidenced-predicates` requires the same revision after a plain restart. That rules out a fresh `observed_at`, which the text does not say.
- **Basis:** own judgment, constrained by the fixture.

#### E-CURSOR-REASON — `invalid_cursor` reasons

- **Where:** CORE §12 gives `invalid_cursor` a `reason` member but no values.
- **Chosen:** `malformed`, `other_stream` or `beyond_end`.
- **Fixtures:** `core.events.invalid-cursor-refused` sends `"not-a-cursor"`, which is `malformed`. Its title says "not from this stream", but no fixture sends a well-formed cursor from another stream or one past the head (E.4).
- **Basis:** own judgment.

#### E-FEATURE-CLAIM-GATING — refusal fixtures run only for providers that claim the feature

- **Where:** `core.grants.grant-field-requires-feature` and `core.events.feature-must-be-negotiated` test sessions that did **not** negotiate the feature, but the runner marks them `not_applicable` unless the participant claims it.
- **Consequence:** a provider without `core.grants` must still refuse a `grant` field with `invalid_envelope` (CORE §5.1), but it is never checked. For `core.events.read`, a provider without the feature does not know the operation and answers `method_not_found`, so gating that fixture is correct.
- **Basis:** runner behavior observed in the manifest before the features were claimed ("participant does not claim feature core.grants").

#### E-ARRAY-EXACT — array patterns are exact-length

- **Where:** the conformance README says "Patterns match subsets".
- **Observed:** arrays in patterns must have exactly the expected length ("result/items: expected 1 items, found 2"). Objects match as subsets. That is what makes E-GAP-TO fail rather than pass by prefix.
- **Basis:** runner output.

### E.3 Open questions decided here, not decided by any fixture

#### E-AUTH-WITHOUT-FEATURE — does authorization apply when `core.grants` was not negotiated?

- **Chosen:** yes. Authority principals are provider configuration, not a negotiated feature. A non-authority in a session without `core.grants` cannot name a grant, so every protected operation gets `permission_denied` with `grant_required`.
- **Basis:** own judgment (the secure reading of §15.1, "Every other principal acts only under a grant").

#### E-UNPROTECTED — which operations step 6 protects

- **Where:** CORE §15.5 applies to operations "that a profile protects". Rights are defined for core-test (§13) and `core.events.read` (§16.6) only.
- **Chosen:**
  - Protected by rights: `core-test.*` (as in §13) and `core.events.read` / `core.events.subscribe`.
  - Own rules: `core.grant.issue` and `core.grant.revoke` (§15.3); `core.grant.get` (visibility, `not_found`).
  - Not protected: `core.describe`, `core.negotiate`, `core.capabilities` (any principal reads the snapshot) and `core.events.unsubscribe` (it only removes the session's own subscription).
  - A `grant` field on an unprotected or grant-management operation is validated but not evaluated.
- **Basis:** own judgment.

#### E-DENIAL-ORDER — which reason wins

- **Chosen:**
  1. `grant_not_found`, when the grant does not exist or is held by someone else;
  2. then `revoked`, `expired`, `authority_epoch_stale`;
  3. then `right_missing` for any needed right;
  4. then `out_of_scope` for any needed subject.

  Checking the holder first is required for "a grant held by someone else is indistinguishable from a nonexistent one". Checking state first would answer `revoked` for another principal's revoked grant (E.4).
- **Basis:** spec text for the holder check first (§15.5); the CORE list order for the rest.

#### E-ISSUE-VALIDATION-STEP — where the audience and expiry checks sit

- **Where:** CORE §15.3 says a wrong `audience`, or an `expires_at` not after the provider's current time, is `invalid_envelope`. CORE §10 lists `invalid_envelope` among the errors of step 6.
- **Chosen:** at step 6, before the issuing rules, so after the deduplication lookup. If these checks sat at step 2, retransmitting a bound issue after the provider clock passed its `expires_at`, or after `provider_id` changed, would be refused instead of replayed (CORE §6.2).
- **Probes:** E-ISSUE-VALIDATION-STEP shows both replays.
- **Basis:** spec text (§6.2 replay guarantee; step 6's error list).

#### E-ISSUE-PARENT-HOLDER — may an authority delegate from a grant it does not hold?

- **Chosen:** no. "A grant with `parent` may be issued only by the parent's holder" is read literally; the answer is `grant_not_found`. An authority issues root grants instead.
- **Basis:** spec text.

#### E-REVOKE-DENIAL — refusing a revoker who is neither issuer nor authority

- **Where:** no §15.5 reason fits: `not_authority` is defined for issuing root grants, and `grant_not_found` for grants not held by the principal.
- **Chosen:**
  - `not_authority` for anyone who is neither issuer nor authority, whether or not the grant exists or is visible to them, so nothing leaks.
  - Revoking an already revoked grant is `permission_denied` with `revoked`: the outcome schema requires a non-empty `revoked` list, so re-revocation cannot succeed.
- **Basis:** own judgment.

#### E-GRANT-PRE — the precondition of `issue` and `revoke`

- **Chosen:**
  - `issue` needs exactly one precondition, naming the grant with revision 0.
  - `revoke` needs exactly one, naming the grant with a revision ≥ 1, because an existing grant's current revision is never 0.
  - Anything else is `invalid_envelope` at step 2.
- **Basis:** spec text for the rule (§15.3); own judgment for the error, as D-PRE-PRIMARY.

#### E-REVOKE-CASCADE — revocation order and revisions

- **Chosen:**
  - The grant first, then its descendants breadth-first in issue order.
  - Each grant that is still active gets `state: revoked`, revision + 1 and one `core.grant.revoked` event; already revoked descendants are skipped and not listed.
  - The acknowledgment's revision is the named grant's new revision.
- **Fixtures:** `core.grants.revocation-cascades` checks that the child's revision is 2 and that two grants are listed.
- **Basis:** spec text (§15.3, §16.3 "primary subject first"); own judgment for the descendant order.

#### E-BINDING-SCOPE — an `authority_binding` to an unknown scope or future epoch

- **Chosen:** accepted at issue. A scope this provider does not track stays at epoch 0. A grant bound to a later epoch authorizes once that epoch becomes current ("authorizes only while that authority scope's current epoch equals `epoch`").
- **Basis:** spec text; whether issuing should refuse such bindings is open.

#### E-RIGHTS-UNKNOWN — unknown right names and resource kinds

- **Chosen:** accepted; they never match anything.
- **Basis:** own judgment.

#### E-PRE-CURRENT — "where the principal may read the subject"

- **Chosen:**
  - As an authority without a grant: always.
  - Under a grant: when the grant has `core-test.read` covering the subject.
  - For `core.grant` subjects: when the principal is the holder, the issuer or an authority.
  - `stale_authority_epoch.current_epoch` follows the same rule for the authority subject.
  - A put under a write-only grant that fails its primary precondition therefore gets no `current`.
- **Basis:** spec text (§7, §12), with own judgment on what "read" means per kind.

#### E-EXPIRY-BOUNDARY — at exactly `expires_at`

- **Chosen:** expired, consistent with refusing to issue a grant whose `expires_at` is not after the current time.
- **Basis:** own judgment.

#### E-START-ORDER — order of launch-time changes

- **Chosen:** deduplication window, then `new_epoch_on_start`, then `retain_last` (counted across all epochs), then the capability snapshot comparison, whose event is therefore never discarded at the same start.
- **Basis:** own judgment.

#### E-CURSOR-AFTER-HIDDEN — where `next_cursor` points when trailing events are hidden

- **Where:** CORE §16.4: "`next_cursor` is the position after the last item".
- **Chosen:** when a read reaches the head, `next_cursor` is the head, even if the events after the last item were hidden. A filtered reader then does not rescan them. When `limit` is reached, it is the last item's position.
- **Basis:** own judgment. Neither choice repeats or skips an item.

#### E-CURSOR-OLD-EPOCH — a cursor in a closed epoch past `vouched_through`

- **Chosen:** accepted; the reader receives the `epoch_change`. Only a position after the current head is `beyond_end`. Such a cursor is what a consumer holds after the provider lost events it had delivered, which is the case epochs exist for.
- **Basis:** own judgment.

#### E-GAP-EPOCHS — a discarded range that spans epochs

- **Chosen:** one gap covers the whole range. Epoch changes inside it are not reported separately; the gap's `from` and `to` carry different epochs. An `epoch_change` at the reader's own epoch boundary still comes before the gap.
- **Probe:** E-START-ORDER.
- **Basis:** own judgment; unreachable by current fixtures.

#### E-SUB-REAUTH — a subscription whose authorization lapses

- **Chosen:** authorization is re-evaluated before each delivery. If the grant no longer authorizes (revoked, expired, epoch superseded), the subscription ends without a notification, because `core.events.notify` has no way to say it ended.
- **Basis:** own judgment. This is in tension with §16.5 "Semantic events are never dropped silently"; the notification schema may need an end item.

#### E-NOTIFY-SIZE — notifications and reads larger than the caller's receive limit

- **Chosen:**
  - A notification carries at most 100 items, and fewer when the frame would exceed the caller's `receive_limits.max_frame_bytes`.
  - A read returns fewer than `limit` items when the full response would not fit.
  - An item that cannot fit even alone ends the subscription, or makes the read `internal_error`, rather than being skipped.
- **Basis:** own judgment (STREAM §1.5; §16.5 "never dropped silently").

#### E-UNSUBSCRIBE — an unknown subscription

- **Chosen:** `not_found`.
- **Basis:** own judgment.

#### E-EVENTS-WITHOUT-FEATURE — recording in sessions without `core.events`

- **Chosen:** every accepted command records its events whatever the session negotiated, as does a capability change at start. The stream belongs to the store (§16: "Every provider records what it committed").
- **Basis:** spec text.

#### E-CAP-SCOPE — what depends on `core-test.writes`, and unknown capability names

- **Chosen:**
  - Only `core-test.subject.put` depends on it (§17.4). `claim`, `grant.issue` and `grant.revoke` still commit when it is `unsupported`, although the capability simulates "a store that became read-only".
  - A launch configuration naming any other capability refuses to start with exit status 2.
  - The capability event's subject id is the `provider_id` of the start that observed the change.
- **Basis:** spec text for the dependency; own judgment for the rest.

### E.4 Requirements the documents state that no fixture checks

`tests/fixture_sensitivity.py m2` applies each deviation to a temporary copy, runs the whole suite, and counts fixtures that fail **in addition to** those the unmodified implementation fails (the baseline is `core.events.retention-gap-returns-snapshot`). Rows marked *control* are deviations a fixture is known to guard; they show the harness notices.

| Deviation applied | Requirement it breaks | Newly failing fixtures |
|---|---|---|
| *control:* revocation does not cascade | §15.3 | 1 (`core.grants.revocation-cascades`) |
| *control:* non-authority may issue root grants | §15.3 | 1 (`core.grants.non-authority-cannot-issue-root-grant`) |
| *control:* grant expiry ignored | §15.5 | 1 (`core.grants.expired-grant-refused`) |
| *control:* other precondition subjects need no read right | §13 rights | 1 (`core.grants.precondition-subjects-need-read`) |
| *control:* `caused_by` not copied into events | §16.2 | 1 (`core.events.commands-append-contiguous-events`) |
| *control:* `epoch_change` item omitted | §16.4 | 1 (`core.events.epoch-change-is-explicit`) |
| *control:* unsubscribe does not stop delivery | §16.5 | 1 (`core.events.subscription-delivers-backlog-then-live`) |
| *control:* a new store's initial capability snapshot appends an event | §17.3 | 9 |
| `grant` field accepted in a session without `core.grants` | §5.1 (E-GRANT-FIELD) | 1 (`core.grants.grant-field-requires-feature`) |
| **Grants** | | |
| Parent's `delegation.allowed: false` ignored | §15.3 | 0 |
| Child may expire after its parent, or lack an expiry | §15.3 | 0 |
| Child need not keep the parent's `authority_binding` | §15.3 | 0 |
| A revoked, expired or epoch-stale parent may still delegate | §15.3 "while the parent is active, unexpired and still bound" | 0 |
| Any principal may delegate from a parent, not only its holder | §15.3 | 0 |
| Any principal may revoke any grant | §15.3 "by its issuer or by an authority principal" | 0 |
| `core.grant.get` hidden from a non-authority issuer | §15.3 | 0 |
| Grant still usable at exactly `expires_at` | E-EXPIRY-BOUNDARY | 0 |
| Issue accepted with `expires_at` equal to the provider clock | §15.3 "not after the provider's current time" | 0 |
| Expiry checked before the deduplication lookup, so a bound issue no longer replays | §6.2, E-ISSUE-VALIDATION-STEP | 0 |
| Grant state checked before its holder, so another principal's revoked grant answers `revoked` | §15.5 "indistinguishable from a nonexistent one" | 0 |
| An authority principal naming a grant is not restricted by it | §15.5 rule 1 | 0 |
| `core-test.authority.claim` needs no `core-test.claim` right | §13 rights | 0 |
| `core-test.subject.applied_count` not protected | §13 rights | 0 |
| `precondition_failed.current` revealed to principals that may not read the subject | §7, §15.5 | 0 |
| `out_of_scope` checked before `right_missing` | E-DENIAL-ORDER (open) | 0 |
| No `core.grant.issued` events | §16.2, §16.3 | 0 |
| `core.grant.revoked` only for the named grant, not per revoked grant | §16.2 "one event per revoked grant" | 0 |
| **Events** | | |
| Gap `to` and snapshot at the stream head, retained events folded in | E-GAP-TO | 0; `core.events.retention-gap-returns-snapshot` **now passes** |
| `filtered: false` under a grant when nothing in range was hidden | §16.6 (E-FILTERED) | 0 |
| Well-formed cursor from another stream accepted | §16.4 | 0 |
| Cursor past the head accepted | §16.4 | 0 |
| Notifications sent **before** the response of the command that caused them | §16.5 "Ordering" | 0 |
| Subscription ignores `kinds` | §16.5 | 0 |
| Subscription ignores the grant's resources | §16.6 | 0 |
| `core.events.subscribe` needs no authorization | §16.6 | 0 |
| `core.events.read` under any grant, without right `core.events.read` | §16.6 | 0 |
| Gap snapshot subjects not filtered by the grant's resources | §16.6 "events and snapshot subjects covered by its resources" | 0 |
| **Capabilities** | | |
| Capability checked after the authority epoch and preconditions | §10 step 7, §17.2 "before its preconditions" | 0 |
| Capability checked before authorization | §10 (step 6 precedes step 7) | 0 |
| Revision raised only when a status changes, not when evidence changes | §17.1 | 0 |
| Capability event `revision` differs from the snapshot revision | §17.3 | 0 |
| Capability event subject id is not `provider_id` | §17.3 | 0 |
| `core-test.authority.claim` also depends on `core-test.writes` | §17.4 "`put` depends on it" | 0 |

34 of the 35 non-control deviations pass every fixture. Only the `grant` field row is guarded. The whole list above ran twice: once before and once after the E-NOTIFY-SIZE change, with the same counts.

Two re-checks outside M2:

- **M1 rows.** Rerunning the section D deviations (`fixture_sensitivity.py m1`) now fails the fixtures the resolutions added, except **duplicate precondition subjects accepted: 0**. The D-PRE-DUP resolution is stated but unguarded.
- **E-NOTIFY-SIZE was found by review, not by the suite.** The first version of this implementation sent up to 100 items per notification. Ten 200 KB events then made a frame over the caller's 1 MiB limit; it was replaced by an id-less `internal_error` while the subscription cursor had already advanced, so those events were silently lost. Every fixture passed with that bug.

The most valuable fixtures to add, judged by what these deviations would let through:

- **Subscriptions:** response-before-notification ordering, kinds and grant filtering, and authorization.
- **Existence and authorization leaks:** another principal's revoked grant must answer `grant_not_found`; a stranger must not be able to revoke.
- **Delegation bounds:** expiry, binding, `allowed: false`, and a revoked parent.
- **Step 7 order:** capability before epoch and preconditions.
- **Cursors:** a well-formed cursor from another stream, and one past the head.
- **Gaps:** a gap snapshot under a grant.
- **Retention:** a decision on E-GAP-TO.
