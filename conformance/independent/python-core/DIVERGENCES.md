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
