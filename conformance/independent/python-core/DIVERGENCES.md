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
- **F.** The third pass, against the resolved M2 documents.
- **G.** The M3 pass: `core.effects`, `execution/1` and the decision 007 test controls.

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

---

## F. Third pass: the resolved M2 documents

> Added when this implementation was brought level with the documents at base commit `6c64ae4`, after the Protocol session resolved section E ([M2-DIVERGENCES](../../../docs/work/release-0.1/M2-DIVERGENCES.md) section E). The reading rules were unchanged: CORE, STREAM, the conformance README and schemas, `schemas/**`, VERIFICATION, M2-DIVERGENCES, fixtures and vectors, and this directory. `conformance/reference/**`, `conformance/runner/src/**` and `conformance/crosscheck/**` were not read; the runner was run as a binary. Sections A–E above are left as written, as evidence of what the earlier documents said.

Entries use the same format, with tags `F-…`. `tests/probe_f.py` asserts each chosen behavior under its tag. `tests/probe_m2.py` keeps its E tags. The probes whose decision was resolved the other way (E-FILTERED, E-SUB-REAUTH, E-GAP-FILTER) now assert the resolved behavior and say so. The grants in E-SUB-ORDER and E-GAP-FILTER also gained `core-test.read`, which §16.6 now requires.

**Reading order.** This time the documents were read **before** the fixtures:

1. CORE in full, STREAM, the conformance README (including `events.unvouched_last` and the start order), the launch-configuration and notification schemas, and M2-DIVERGENCES;
2. the diff of `docs/spec`, `schemas`, the conformance README and schemas between `3b32037` and `6c64ae4`, to be sure nothing changed silently;
3. a written change list from those documents alone;
4. only then the eight failing fixtures.

The fixtures did not change the list. One detail the fixtures pin, the `next_cursor` of a final `item_too_large` notification, had already been chosen from the text (F-ENDED-CURSOR).

**Changes after fixture failures: none.** The first complete run after implementing the change list passed every applicable fixture: 138 pass and 6 `socket.*` fixtures are not applicable to a stdio participant.

### F.1 Contradictions between documents

#### F-GRANT-VIS-AUTHORITY — "matching `core.grant.get`" does not match for an authority under a grant

- **Where:** CORE §16.6 table: a `core.grant` subject is visible under a grant when "The session principal is that grant's holder or issuer, matching `core.grant.get` (§15.3)". CORE §15.3: `core.grant.get` is also visible to authority principals. CORE §15.5 says a named grant restricts an authority, but also that a `grant` field on `core.grant.*` operations "is validated but not evaluated".
- **Consequence:** an authority principal reading events under its own grant cannot see the `core.grant.issued` event of a grant delegated between two other principals. `core.grant.get` for the same grant, with the same `grant` field, returns the record. So the two do not match for authorities.
- **Chosen:** the table's literal rule (holder or issuer). Probe F-GRANT-VIS-AUTHORITY shows both answers side by side.
- **Fixtures:** none reads grant events as an authority under a grant.
- **Basis:** spec text. Either drop "matching `core.grant.get`", or state that under a grant an authority is treated as a non-authority for `core.grant` visibility. The second is what "restricted to that grant" implies.

#### F-AUTH-STEP1 — `core.authenticate` before negotiation: §3 and §18 against §10 step 1

- **Where:** CORE §3.1: before negotiation a provider answers `core.describe`, `core.negotiate` "(and `core.authenticate`, §18)". CORE §18.2 "Repeats": a session that has a principal gets `already_authenticated`, "This includes every stdio session". CORE §10 step 1 still reads "session negotiated (unless the operation is `core.describe` or `core.negotiate`)". §18.2 "Order" moves the authentication check before step 1 only for shared transports.
- **Question:** on an unnegotiated stdio session, is `core.authenticate` `negotiation_required` (step 1 read literally) or `already_authenticated` (§3.1, §18.2)?
- **Chosen:** `already_authenticated`, before and after negotiation. Step 1 exempts all three operations.
- **Fixtures:** `socket.already-authenticated` is restricted to the unix binding. No stdio fixture calls `core.authenticate`; see F.5 for the sensitivity result.
- **Basis:** spec text (§3.1 and §18.2 are the more specific rules). Step 1's parenthesis should name `core.authenticate`.

#### F-GAP-EPOCHS-REACHABLE — a discarded range spanning epochs is reachable now

- **Where:** M2-DIVERGENCES E-GAP-EPOCHS defers the order of `epoch_change` and `gap` to M6 because "a discarded range spanning epochs is unreachable with current launch controls". The conformance README counts `events.retain_last` "across all epochs".
- **Reaching it:** record events in epoch 1, restart with `new_epoch_on_start`, commit a command in epoch 2, then restart with `retain_last: 0`. A reader from the start now faces discarded positions in both epochs. The resulting read is probe F-GAP-EPOCHS.
- **Chosen:** unchanged from E-GAP-EPOCHS. One gap covers the whole range, with no `epoch_change` inside it; the result's `stream.epoch` shows the current epoch. A cursor exactly at the old epoch's vouched end receives the `epoch_change` first, then the gap.
- **Basis:** own judgment. The deferral's premise no longer holds, so the order is a 0.1 question.

#### F-AUTH-WITHOUT-FEATURE-GUARD — the resolution record names a fixture that does not test the decision

- **Where:** M2-DIVERGENCES E-AUTH-WITHOUT-FEATURE: "Authorization applies without `core.grants`; a non-authority gets `grant_required`. **fixture** `core.capabilities.checked-after-authorization-before-preconditions` exercises it." That fixture negotiates `core.grants` and `core.capabilities` in both sessions. No `grant_required` fixture runs in a session without `core.grants`.
- **Chosen:** unchanged. A non-authority without the feature gets `grant_required` (probe E-AUTH-WITHOUT-FEATURE).
- **Evidence:** the deviation "authorization skipped when `core.grants` was not negotiated" passes every fixture (F.5).
- **Basis:** fixture text against the resolution record. Either the record or the fixture should change.

### F.2 Expectations the documents do not state

#### F-UNVOUCHED — what happens to events after `vouched_through`

- **Where:** conformance README `events.unvouched_last`: "vouch for the previous epoch only through its last sequence minus this many events … Subject state is unchanged; only the vouched position moves." CORE §16.1: "The previous epoch keeps the events the provider still vouches for, through `vouched_through`." CORE §16.4 `epoch_change`: "Nothing after that position in the old epoch will ever be delivered."
- **Questions:** are the unvouched events still stream events? Do they count toward a later `retain_last`? Does a later retention snapshot reflect their subject changes?
- **Chosen:**
  - They leave the stream at the start that closes the epoch. They are never delivered, and they do not count toward `retain_last` (probe F-UNVOUCHED-RETENTION).
  - A closed epoch ends at its `vouched_through` for reading. A cursor past it receives the `epoch_change` as its first item, with a `vouched_through` below the cursor.
  - Subject state is untouched, so a later gap snapshot whose `as_of` lies in a later epoch includes their changes ("the visible subjects at `as_of`"). A snapshot as of a position inside the closed epoch does not (probe F-GAP-EPOCHS).
- **Fixtures:** `core.events.cursor-past-vouched-position-gets-epoch-change` fixes delivery and the cursor, not retention.
- **Basis:** spec text for delivery; own judgment for retention and snapshots.

#### F-UNVOUCHED-CONFIG — edge values of `unvouched_last`

- **Where:** the README says "With `new_epoch_on_start`"; the launch-config schema allows any integer ≥ 0.
- **Chosen:**
  - Without `new_epoch_on_start` the key has no effect; the provider does not refuse to start.
  - A value larger than the epoch's last sequence vouches through 0.
- **Basis:** own judgment. Both are probes.

#### F-ENDED-CURSOR — `next_cursor` of the final notification

- **Where:** the notification schema requires `next_cursor` even with `ended`. CORE §16.5 does not say which position it names.
- **Chosen:** where delivery stopped, after the last position the subscription covered. For `item_too_large` that is the position before the item that did not fit, so the item is never skipped. For `authorization_lost` it is the same rule; it can lie past trailing events the grant hid, never past an event the grant would have shown.
- **Fixtures:** `core.events.reads-and-notifications-fit-receive-limit` requires the cursor after the last delivered event for `item_too_large`. Nothing checks it for `authorization_lost`.
- **Basis:** own judgment, chosen before reading the fixture.

#### F-CREDENTIALS-CONFIG — `credentials` in a stdio launch

- **Where:** the launch-config schema gained `credentials`; the README says the runner writes them for `binding: unix`.
- **Chosen:** this stdio-only participant refuses to start (exit 2) when `credentials` is present. The README says a participant "should refuse to start … rather than silently ignore" keys it does not support.
- **Basis:** README.

#### F-CREDENTIAL-SCHEMA — the parameter schema does not use the credential definition

- **Where:** `core.authenticate.params.schema.json` accepts any string of 1–512 code points. `common.schema.json#/$defs/credential` (with the `ccred1.` pattern and a 200-character limit) is referenced by nothing in `schemas/`.
- **Chosen:** the parameter schema. A malformed credential is not `invalid_envelope`; on a shared transport it would be `authentication_failed` (§18.2). On stdio it is `already_authenticated` (F-AUTH-ORDER).
- **Basis:** schema, consistent with §18.2 "Unknown, malformed and revoked credentials all produce `authentication_failed`". The unused definition is harmless but looks like an intended reference.

### F.3 Open questions decided here, not decided by any fixture

#### F-PRE-CURRENT-NO-GRANT — a non-authority acting without a grant on `core.grant.*`

- **Where:** CORE §7 names two cases: "An authority principal acting without a grant may read every subject. Under a grant, the principal may read the subjects that grant would show it in events." A delegating holder (`core.grant.issue` with `parent`) or a non-authority issuer revoking a child acts under no evaluated grant (§15.5: the `grant` field on `core.grant.*` is not evaluated), so neither case applies.
- **Chosen:** such a principal may read what `core.grant.get` shows it: grants it holds or issued. For other kinds `current` is omitted.
- **Basis:** own judgment, as in E-PRE-CURRENT.

#### F-PRE-CURRENT-FOREIGN — `current` for a kind no profile defines, under a grant

- **Where:** CORE §13 rights: `put` needs `core-test.read` "on every other subject named in its preconditions", so a grant covering `other.kind` authorizes such a precondition. CORE §7 defers to the §16.6 table, which has no row for a kind that no profile defines. (Such a subject never exists, CORE §7.)
- **Chosen:** omitted under a grant. An authority without a grant gets `current: 0`.
- **Basis:** own judgment; either answer discloses nothing.

#### F-CURSOR-OLD-EPOCH-PAST-END — a cursor past the last sequence a closed epoch ever had

- **Where:** CORE §16.4 says a cursor into an earlier epoch "is valid even when it points past that epoch's `vouched_through`", and that only "one past the head of the current epoch" is `invalid_cursor`. The general cursor rule also refuses one that "points beyond the stream's current end".
- **Chosen:** valid, answered with the `epoch_change`. A store restored from a backup may never have recorded the position a consumer holds, and that is the case epochs exist for.
- **Basis:** spec text (the more specific rule).

#### F-SUB-END-TIMING — when `authorization_lost` is sent

- **Where:** CORE §16.5: authorization is "checked when subscribing and again before each delivery", and a subscription whose grant stops authorizing gets one final notification.
- **Chosen:** checked after every request on the connection, whether or not an item is pending. The end is therefore reported right after the response of the request that caused it, even when the subscription's `kinds` filter would deliver nothing. With the system clock, an expiry is noticed at the next request; this provider has no timer.
- **Fixtures:** `core.events.subscription-ends-when-grant-stops-authorizing` passes under either reading, because the claim that makes the grant stale records an event the grant can see.
- **Basis:** own judgment.

#### F-ISSUE-CHECK-ORDER — order of the step 6 validity checks on `core.grant.issue`

- **Where:** CORE §15.3: the binding scope is "checked with `audience` and `expires_at` at step 6". No order is given among the three, or against the issuing rules (`not_authority`, `grant_not_found`, `delegation_exceeded`).
- **Chosen:** `audience`, then `expires_at`, then the binding scope, all before the issuing rules. A non-authority issuing a root grant bound to an unknown scope gets `invalid_envelope`, not `not_authority`.
- **Basis:** own judgment (§15.2 field order). Only `details.path` differs among the three.

#### F-AUTH-ORDER — `core.authenticate` on stdio

- **Chosen:**
  - It is a query, not protected (§15.5).
  - Its params are validated at step 2 and its `requires` at step 3 before `already_authenticated`, as D-NEG-AGAIN decided for a second negotiation.
  - The credential is never examined, logged or echoed. A malformed credential string therefore also gets `already_authenticated`.
- **Basis:** spec text for the order (§10: "A query applies steps 1–3"); own judgment for the rest.

#### F-FILTERED-RANGE — the "range covered by this result"

- **Chosen:**
  - When `limit` stops a read, the covered range ends at the last item, and hidden events after it do not set `filtered`.
  - When a read reaches the end, the range extends over trailing hidden events, matching `next_cursor`.
  - An empty read at the head is `filtered: false`.
- **Basis:** spec text (§16.4 "Cursors" and "Filtering" read together).

#### F-READ-FIT — how many items a size-limited read or notification carries

- **Chosen:**
  - The largest item count whose complete frame fits, found by binary search.
  - The frame is measured exactly, including the request's JSON-RPC `id`.
  - A read whose first item cannot fit is `internal_error`, raised explicitly rather than by replacing an oversized response.
- **Basis:** own judgment. CORE §16.4 requires only "fewer than `limit` items".

### F.4 Behavior changes in this pass

| Change | Document | Previous entry | Fixtures that now pass |
|---|---|---|---|
| Event and snapshot visibility under a grant follows the §16.6 table: profile subjects need `core-test.read` covering them; `core.grant` subjects need the principal to be holder or issuer, whatever the resources; `core.capabilities` needs coverage only | CORE §16.6 | E-GRANT-EVENT-LEAK | `core.events.authorization-and-filtering`, `core.events.retention-snapshot-is-filtered`, `core.events.subscription-applies-kinds-and-grant` |
| `filtered` is true exactly when something in the covered range was hidden | CORE §16.4 | E-FILTERED | `core.events.unfiltered-read-under-grant` |
| `precondition_failed.current` and `stale_authority_epoch.current_epoch` under a grant follow the same table; an authority without a grant reads everything | CORE §7 | E-PRE-CURRENT | none (`core.grants.precondition-current-needs-read` already passed) |
| `events.unvouched_last` accepted; a closed epoch ends at `vouched_through`; unvouched events leave the stream | README, CORE §16.1, §16.4 | E-CURSOR-OLD-EPOCH | `core.events.cursor-past-vouched-position-gets-epoch-change` |
| A subscription ends with a final notification, `ended: authorization_lost` or `item_too_large`, instead of silently | CORE §16.5 | E-SUB-REAUTH, E-NOTIFY-SIZE | `core.events.subscription-ends-when-grant-stops-authorizing`, `core.events.reads-and-notifications-fit-receive-limit` |
| Reads and notifications carry the largest fitting item count, measured on the exact frame | CORE §16.4 "Size", §16.5 | E-NOTIFY-SIZE | (with the row above) |
| An `authority_binding` to an untracked scope is `invalid_envelope` at step 6 | CORE §15.3 | E-BINDING-SCOPE | `core.grants.authority-binding-scope-must-exist` |
| `core.authenticate` is known and answers `already_authenticated` | CORE §3.1, §18.2 | new | none on stdio |
| The capability revision ignores `observed_at` (none is emitted, so this is not observable) | CORE §17.1 | E-CAP-EVIDENCE | none |
| A stdio launch with `credentials` refuses to start | README | new | none |

Re-read against the new text and left unchanged, because this implementation already did what the resolution decided:

- the grant command precondition revisions (E-GRANT-PRE);
- re-revocation and the `not_authority` rule (E-REVOKE-DENIAL);
- the protected-operations list (E-UNPROTECTED);
- the denial order (E-DENIAL-ORDER);
- the expiry boundary (E-EXPIRY-BOUNDARY);
- issue validation after deduplication (E-ISSUE-VALIDATION-STEP);
- the snapshot state shapes (E-SNAPSHOT-STATE);
- a gap ending at the discard boundary, which CORE §16.4 now allows (E-GAP-TO).

### F.5 Fixtures, and requirements no fixture checks

**Failing fixtures: none.** No fixture is believed wrong. One observation: `core.events.retention-snapshot-is-filtered` accepts both gap readings only because the one retained event happens to be hidden from its reader, so under this implementation's boundary reading the gap is still the only item. It does not distinguish the two readings, and E-GAP-TO allows both anyway.

`tests/fixture_sensitivity.py` applies each deviation to a temporary copy, runs the whole suite and counts fixtures that fail beyond the unmodified implementation's (none now fail). The harness was changed in this pass in two ways:

- It runs variants four at a time.
- It counts every non-passing status. The earlier version counted only lines starting with `fail`, so a deviation that made a fixture **time out** was reported as unguarded. The E.4 counts above may therefore be low for subscription rows; they were not re-derived with the old code.

All three groups were rerun: `m1` (section D), `m2` (section E.4, with four edits re-targeted to the new code) and the new `f` group of 49 deviations for the section E resolutions. Rows marked *control* break a requirement a fixture is known to guard; each failed at least one fixture, which shows the harness notices.

**Re-checks of earlier groups.**

- **`m1`:** all 23 deviations now fail at least one fixture. D-PRE-DUP is guarded by `core.preconditions.duplicate-subject-invalid`, and a nonzero exit status fails every fixture.
- **`m2`:** 40 of 43 deviations fail at least one fixture. Three still pass everything:
  - **cursor beyond the head accepted:** known; M2-DIVERGENCES E-CURSOR-OLD-EPOCH says callers cannot construct one.
  - **revision raised only when a status changes:** known; E-CAP-EVIDENCE.
  - **gap and snapshot at the stream head:** no longer a deviation, since CORE §16.4 allows it.

**Group `f`: the section E resolutions.**

| Deviation applied | Requirement it breaks | Newly failing fixtures |
|---|---|---|
| *control:* binding to an unknown scope accepted | §15.3 | 1 (`core.grants.authority-binding-scope-must-exist`) |
| *control:* current omitted for an authority without a grant | §7 | 5 |
| *control:* profile subjects visible with resource coverage alone | §16.6 | 2 |
| *control:* `core.grant` subjects visible only with resources covering `core.grant` | §16.6 | 4 |
| *control:* snapshot omits `core.grant` subjects | §16.4 | 1 |
| *control:* unvouched events delivered; `unvouched_last` ignored; cursor past `vouched_through` refused | README, §16.4 | 1 each (`core.events.cursor-past-vouched-position-gets-epoch-change`) |
| *control:* reads, or notifications, ignore the caller's receive limit; `item_too_large` ends silently | §16.4, §16.5 | 1 each (`core.events.reads-and-notifications-fit-receive-limit`) |
| *control:* authorization loss ends silently | §16.5 | 1 (`core.events.subscription-ends-when-grant-stops-authorizing`) |
| Binding to a later epoch of a known scope refused at issue | §15.3 | 1 |
| Grant command precondition revision unchecked | §15.3 | 1 |
| Re-revocation accepted | §15.3 | 1 |
| `current` under a grant needs only coverage, not the read right | §7, §16.6 | 1 |
| `core.events.unsubscribe` protected | §15.5 | 1 |
| `filtered` ignores events hidden only by `kinds` | §16.4 | 1 |
| A read whose first item cannot fit returns no items | §16.4 "Size" | 1 |
| `item_too_large` skips the item and continues | §16.5 | 1 |
| Authorization loss reported as `item_too_large` | §16.5 | 1 |
| Final notification's `next_cursor` is the head | F-ENDED-CURSOR | 1 |
| **Grants and authorization** | | |
| Re-revocation reports `revoked` before `not_authority`, so a stranger learns a grant's state | §15.3 "decided after that check" | **0** |
| Re-revocation checked after preconditions (stale revision gives `precondition_failed`) | §10 step 6 before step 7 | **0** |
| Revocation lists and re-revokes descendants already revoked | §15.3 "descendants that were still active" | **0** |
| `stale_authority_epoch.current_epoch` always disclosed | §8, §12 "if permitted" | **0** |
| `core.capabilities` protected (non-authority needs a grant) | §15.5 | **0** |
| `grant` field evaluated on unprotected queries and `core.grant.get` | §15.5 "validated but not evaluated" | **0** |
| Authorization skipped when `core.grants` was not negotiated (labelled *control* before the run, because M2-DIVERGENCES E-AUTH-WITHOUT-FEATURE names `core.capabilities.checked-after-authorization-before-preconditions` as its fixture; that fixture negotiates `core.grants`) | §15.5 "Without `core.grants`" | **0** |
| **Event visibility** | | |
| Every `core.grant` subject visible under any grant with `core.events.read` | §16.6 (the E-GRANT-EVENT-LEAK disclosure) | **0** |
| `core.grant` subjects visible to the holder only, not the issuer | §16.6 | **0** |
| `core-test.authority` visible with `core-test.read` and no resource covering it | §16.6 | **0** |
| `core.capabilities` visible under any events grant | §16.6 | **0** |
| `core.capabilities` also needs `core-test.read` | §16.6 | **0** |
| An authority reading under a grant sees every event | §15.5 rule 1, §16.6 | **0** |
| **Reads, cursors and snapshots** | | |
| `filtered` ignores hidden snapshot subjects | §16.4 "Filtering" | **0** |
| `filtered` counts hidden events beyond the covered range | §16.4 "Filtering" | **0** |
| `next_cursor` stops at the last item although trailing hidden events were covered | §16.4 "Cursors" | **0** |
| Snapshot state of a revoked grant is `{state}`, not `{grant}` | §16.4 "Snapshot state" | **0** |
| **Subscriptions** | | |
| A subscription survives revocation of its grant (only expiry and epoch staleness end it) | §16.5 "Lifetime" | **0** |
| **`core.authenticate` on stdio** | | |
| Unknown operation (`method_not_found`) | §3.1, §18.2 | **0** |
| Succeeds and returns the principal | §18.2 "Repeats" | **0** |
| Answers `authentication_failed` | §18.2 "Repeats" | **0** |
| **This implementation's open choices (section F), not requirements** | | |
| Binding scope checked before audience and expiry | F-ISSUE-CHECK-ORDER | 0 |
| `current` omitted for a delegating non-authority's own grants | F-PRE-CURRENT-NO-GRANT | 0 |
| Cursor in a closed epoch past its last sequence refused | F-CURSOR-OLD-EPOCH-PAST-END | 0 |
| Unvouched events count toward `retain_last` | F-UNVOUCHED | 0 |
| Retention snapshot omits changes made by unvouched events | F-UNVOUCHED | 0 |
| `core.authenticate` needs negotiation first | F-AUTH-STEP1 | 0 |

Of the 37 non-control `f` deviations, **27 pass every fixture**. 21 of those break a stated requirement (bold rows); the other 6 reverse this implementation's own open choices. Together with the three `m2` rows above, the script reports 29 non-control deviations passing, because the `core.grants` row was still labelled *control* during the run.

What these say about the suite:

- **E-GRANT-EVENT-LEAK is fixed in the text but unguarded against its original form.** The fixtures show `core.grant` events to their holder, but none puts a grant held **and** issued by others in a reader's range. A provider that shows every grant record to any `core.events.read` holder passes. The same is true for `core.capabilities` and `core-test.authority` visibility, and for an authority restricted by a named grant when reading events.
- **Subscription authorization loss is guarded only for epoch staleness.** Revocation, the case §16.5 names first, is not exercised in a subscriber's session.
- **The `core.authenticate` stdio rules have no fixture at all,** because every authentication fixture is restricted to the unix binding.
- **Authorization without `core.grants` has no fixture;** the named one negotiates the feature.
- **Re-revocation ordering and cascades over already revoked descendants are unguarded,** as is disclosure of `current_epoch`.
- **`filtered` and `next_cursor` are guarded only in their simplest cases:** nothing hidden, or hidden by `kinds` alone.

### F.6 Verification

Run from the worktree root at base `6c64ae4` plus this pass's changes, on macOS (Darwin 25.3) with Python 3.14.5 and the pinned Rust toolchain. The provider is real; nothing is simulated. Results are in `conformance/results/independent-python-core/` (not committed).

| Command | Exit | Result |
|---|---|---|
| `cargo build --workspace --locked` | 0 | built |
| `./target/debug/combraton-conformance run --participant conformance/participants/independent-python-core.json --out conformance/results/independent-python-core` | 0 | `run: 144 fixtures, 0 not passing`: 138 pass, 6 `socket.*` not applicable (unix binding only) |
| `python3 conformance/independent/python-core/tests/check_vectors.py` | 0 | `failures: 0` |
| `python3 conformance/independent/python-core/tests/probe_provider.py` | 0 | 19/19 probes as documented |
| `python3 conformance/independent/python-core/tests/probe_m2.py` | 0 | 43/43 probes as documented |
| `python3 conformance/independent/python-core/tests/probe_f.py` | 0 | 31/31 probes as documented |
| `python3 conformance/independent/python-core/tests/fixture_sensitivity.py --jobs 4` | 0 | the counts in F.5; the script printed "29 non-control deviations pass every fixture" before the `core.grants` row was relabelled |
| `python3 conformance/independent/python-core/tests/fixture_sensitivity.py --check` | 0 | all 115 deviations apply to the committed sources |

Not established:

- the Unix-socket binding and real credentials;
- behavior under concurrent sessions;
- expiry of a subscription's grant by the system clock between requests;
- anything outside the fixtures and the probes above.

---

## G. M3 pass: effects, execution and test controls

> Added when this implementation was extended to CORE §19 (`core.effects`), `execution/1` (EXECUTION §1–§15) and the decision 007 test controls, from the documents at base commit `d81e49b`. Read: CORE, EXECUTION, STREAM, ENCODING, decision 007, M3, M2-DIVERGENCES, MATRIX, the conformance README, VERIFICATION, `schemas/**`, the fixture and launch-configuration schemas, and this directory. Not read: `conformance/reference/**`, `conformance/runner/src/**`, `conformance/crosscheck/**`, or their history. The runner was used as a binary. Sections A–F are left as written.

**Reading order.**

1. The documents above in full, and the diff of `docs/spec`, `schemas` and the conformance README since the third pass.
2. A change list written from those documents alone, implemented and committed (`188ab07`) before any `execution/`, `socket/` or M3 Core fixture was opened. The decisions it needed are section G.3.
3. The first complete run, recorded below before anything changed.
4. Only then the failing fixtures and their transcripts.

**First complete run** (commit `188ab07`, claims as in the participant descriptor): 211 fixtures, **176 pass, 19 fail, 0 timeout, 0 harness_error, 4 unsupported, 12 skipped.**

**Changes after the first complete run.** Two, each a misreading of text the documents do state:

| Tag | Change | Why | Fixtures that then passed |
|---|---|---|---|
| G-EXIT-RUNTIME | A scripted `exit` step also records runtime `exited` (event `execution.runtime.changed`, then `execution.exit.observed`) | The step is the harness process exiting. EXECUTION §3 makes `runtime` the observed state; reporting an exit code for a process still `active` or `preparing` was an inconsistent observation, not independence of axes. | `execution.action-wait-survives-restart`, `execution.capacity-queue-admits-in-order-and-times-out`, `execution.detach-is-not-cancellation`, `execution.evaluation-is-never-derived-from-exit` |
| G-COALESCE-ADJACENT | Spool discards coalesce with any `spool_limit` range they are adjacent to by offset | EXECUTION §14.1: "Adjacent discards are coalesced into one range". The first version coalesced only with the most recently declared lost range, so a `harness_dropped` record in between split one discarded span into two. | `execution.output-loss-is-declared-with-ranges` |

Nothing else was changed in response to a fixture. **Final run:** 211 fixtures, **181 pass, 14 fail, 0 timeout, 0 harness_error, 4 unsupported, 12 skipped**, identical in three consecutive runs. Every remaining failure is listed in G.2.

**How G.2 was established.** A scratch copy of the provider (never committed) applied each fixture-side expectation below. With all sixteen applied, every applicable fixture passed, so no further divergence hides behind a first failing step. Each expectation was then removed from the copy one at a time; the fixtures column of G.2 is exactly the set that failed without it.

### G.1 Contradictions between documents

#### G-IDLE-EXPIRY-FIXED-CLOCK — CORE §16.5 still says the launch clock is fixed

- **Text:** CORE §16.5 "Expiry while idle": "The conformance launch clock is fixed for a process's lifetime, so no portable fixture observes expiry during a session." CORE §13.1, decision 007 §2 and the conformance README define `clock.file`, and M3 acceptance item 10 names such a fixture.
- **Chosen:** the clock file. `core.events.subscription-ends-at-grant-expiry` and `core.grants.test-clock-never-moves-backward`, previously coverage limits for this participant, now run and pass.
- **Suggested:** drop the last sub-bullet of "Expiry while idle".

#### G-EFFECT-HISTORY-CODE — an error code outside the registry

- **Text:** CORE §19.2: "An effect record the provider no longer retains is `effect_history_unavailable`, never `not_found`." The code is absent from the CORE §12 table and from `schemas/core/1/error-data.schema.json`, so no provider can send it and pass schema validation.
- **Chosen:** effect records are never discarded, so the code is never needed. Unguarded by any fixture.

#### G-EPOCH-ABSENT — CORE §8 and EXECUTION §11.3 disagree on a missing epoch

- **Text:** CORE §8: "Epoch absent where the operation requires one: `invalid_envelope`". EXECUTION §11.3: once a host has a controller epoch, "lower than the current epoch, or absent: `stale_authority_epoch`".
- **Chosen:** EXECUTION, the more specific rule. `execution.stale-controller-is-refused` step 4 pins the same reading (passes).
- **Suggested:** CORE §8 could say profiles may classify an absent epoch as stale.

#### G-REFUSED-AXES — "no delivery" against a required `delivery` axis

- **Text:** EXECUTION §3.1 "Refused admission records the execution with no delivery and no effect". `execution.inspect.result` requires `delivery` from the six determinations, and §3.1 says a delivery never stays `pending` after its wait ends.
- **Chosen:** a refused execution (at submit or by `queue_timeout`) shows `delivery: failed_before_delivery`, `deliveries: []`, `effects: []` and runtime `unknown`. A queued execution shows `pending`. No fixture reads `delivery` or `runtime` of a refused execution.

#### G-EVENT-PROOF-CLASS — determinations without a proof class

- **Text:** EXECUTION §9 lists `execution.delivery.observed` as `{ delivery_id, delivery, proof_class, evidence }`. Determinations made by the delivery timeout, by recovery or by an unknown attempt have evidence but no proof class (§3.1).
- **Chosen:** `proof_class` is omitted from those events and from the delivery record. Fixtures match these events by subset and pass.

#### G-CORRELATION-EVENTS — EXE-20 names events; EXECUTION gives them no correlation

- **Text:** MATRIX EXE-20: "arbitrary correlation round-trips through submit, inspect and events". EXECUTION §9 event payloads and the submit result schema carry no correlation member.
- **Chosen:** `correlation` round-trips through `execution.inspect` only. `execution.submit-records-effect-and-links-retries` checks inspect only.

### G.2 Expectations in fixtures that the documents do not state

All fourteen failing fixtures are here. None was worked around.

| Tag | What the fixtures expect | What the documents say, and this implementation | Failing fixtures (first failing step) |
|---|---|---|---|
| G-OBLIGATION-ID | The prompt delivery's obligation ID is `<delivery_id>.evidence` | CORE §19.4 gives obligations an `id` and no format. Here: `<effect id>.obligation`. | `execution.aborting-an-obligation-leaves-the-effect-unknown` (7), `execution.timeouts-are-distinct-and-prove-nothing` (5) |
| G-LEASE-ID | The workspace lease ID is `<execution>.workspace` | EXECUTION §11.4 names `lease_id` only. Here: `<execution>.lease-1`. | `execution.workspace-checkpoints-declare-coverage` (4) |
| G-EVIDENCE-NAMES | Evidence classes `recorded_before_dispatch` (initial effect observation), `dispatch_intent`, `dispatch_uncertain` (recovery ambiguity on the effect), `delivery_timeout` and `delivery_timeout_before_dispatch`, `transport_error`, `harness_observation` (steering behavior); evidence source `scripted harness` | CORE §19.1 and EXECUTION §3 require evidence to name a class and source; no values are defined except proof classes. Here: `recorded`, `recovery` (on the effect too), `timeout`, `attempt_outcome_unknown`, `observed_behavior`; source `scripted-harness`. | `execution.crash-before-dispatch-recovers-under-same-identity` (9), `execution.damaged-journal-is-not-proof-of-non-dispatch` (7), `execution.delivery-wait-ends-without-evidence` (6), `execution.stale-dispatcher-is-fenced-after-recovery` (7), `execution.steering-facts-stay-separate` (9), `execution.submit-records-effect-and-links-retries` (4); also later steps of `execution.crash-during-dispatch-is-reconciled-without-resending` and `execution.retries-follow-retry-class` |
| G-DISPATCH-INTENT | Writing the dispatch marker appends a second `pending` observation to the prompt effect | CORE §19.1 appends observations of status; EXECUTION §7.1 requires the marker, not an observation of it. Here the marker is executor state, and the effect's observations change with its status. | same three as `recorded_before_dispatch` except the damaged-journal fixture: crash-before-dispatch (9), crash-during-dispatch (8), submit-records (4) |
| G-DELIVER-STEP | A `deliver` step is the dispatch attempt itself: after `crash: after_write` or an attempt ended by `transport_errors`, the next `deliver` step has no effect | The README lists `deliver` as "a proof class" with no sequencing rule. EXECUTION §3.1: "When later evidence resolves a delivery, the current determination changes to the resolved value", e.g. `ambiguous → delivered`. Here a correlated `provider_ack_id` arriving after an ambiguous attempt resolves the delivery to `acknowledged`, with the ambiguity kept in history, and nothing is resent. | `execution.crash-during-dispatch-is-reconciled-without-resending` (7), `execution.inactivity-and-reconciliation-timeouts-prove-nothing` (5), `execution.retries-follow-retry-class` (8) |
| G-STALL | A script that only stalls never dispatches, so its delivery timeout gives `failed_before_delivery` | EXECUTION §15 lists "never exit" among harness behaviors; `stall` is undefined. Here a stalled harness is one that received the prompt and never reports, so the timeout gives `ambiguous`. | `execution.delivery-wait-ends-without-evidence` (7, hidden behind step 6) |
| G-OVERDUE-ORDER | At the delivery timeout, `core.effect.obligation.overdue` comes before the `execution.delivery.observed` that makes the delivery `ambiguous` | EXECUTION §8 table: "The evidence wait ends (below); open obligations of the delivery become overdue", in that order of phrases, and no event order. Here: timeout, then determination, then overdue. | `execution.timeouts-are-distinct-and-prove-nothing` (6, hidden behind step 5) |
| G-ACTIONS-ABSENT | `actions` is absent from inspect until an action has been requested | EXECUTION §11.2: "A harness action request sets runtime `requires_action`. `execution.inspect` then carries `runtime_detail` … and lists `actions`." Read here as: sessions with `execution.actions` see the list, empty until a request. | `execution.action-ids-belong-to-their-execution` (5) |
| G-RESPONSE-STATUS | An action response effect is `succeeded` once its attempt completes | CORE §19.3: "a completed call can still leave the effect `pending` or `unknown`"; EXECUTION §11.2 defines no evidence for a response. Here it stays `pending`, with no obligation, since none is stated. | `execution.action-ids-belong-to-their-execution` (12, hidden behind step 5) |
| G-RECOVERY-HOST-EVENT | Recovery that resumes dispatch appends no `execution.host.changed` (the event list is exact) | **Contradicts** EXECUTION §7.1 "Recovery advances the host generation" with §9 "`execution.host.changed` … when the host generation changes". Here the event is appended. | `execution.stale-dispatcher-is-fenced-after-recovery` (8, hidden behind step 7) |
| G-NOTIFY-BATCH | The submit's `execution.admission.changed` and the executor's first `execution.delivery.observed` arrive in **one** notification | CORE §16.5 lets the provider split items across notifications and requires only ordering after the response. Here the command's events are notified right after its response, and the executor's later observation in a second notification. | `execution.watch-execution-facts-with-subscriptions` (6) |

Unstated values the fixtures hard-code that this implementation happened to choose the same way, so the fixtures pass without guarding them:

- **Effect and record IDs:** `<execution>.delivery-1` (fixtures query it after restarts without capturing it), `<execution>.steer-<n>` and `<execution>.steer-<n>.delivery`, `<execution>.checkpoint-<n>`, `<execution>.probe-<n>`. Only `cancel-<n>` and `response-<n>` are in EXECUTION.
- **Names:** effect kind `execution.status_probe`; refusal reason `capability_unavailable` for an adapter predicate that is not `supported` (EXECUTION §5 names no reason); evidence classes `reconciliation` and `recovery` on the delivery record.
- **Values:** runtime `preparing` before any runtime observation (`execution.timeouts-are-distinct-and-prove-nothing` step 11).
- **G-CRASH-TIMING:** fixture notes say "the executor tick at the start of this request reaches the scripted crash". Here a crash step runs only between requests, right after the preceding response. Both satisfy `expect_close`; the README says only that steps advance "no later than the provider's next request".

### G.3 Open questions decided here

Decided from the documents before any M3 fixture was read. Fixtures that exercise a decision are named; "unguarded" means no fixture distinguishes it.

**Clock and ticks**

- **G-CLOCK-READING.** The clock file is read once per request and once per idle re-check, and that reading is used throughout, so one command never sees two times. Surrounding whitespace, including a trailing line feed, is accepted. Unguarded.
- **G-CLOCK-RESTART.** The last good instant is not persisted: a new process accepts any well-formed instant. Decision 007 speaks of one run. Unguarded.
- **G-IDLE-RECHECK.** A 100 ms real-time thread advances scripts and deadlines under the processing lock. It delivers notifications only when it committed something, so a grant expiry alone is reported at the next request, as CORE §16.5 allows on stdio. `core.events.subscription-ends-at-grant-expiry` passes.
- **G-TIMEOUT-SCOPE.** A timeout passes at `now ≥ start + seconds`, like grant expiry. Scope: `queue` only while queued; `delivery` only while the delivery is `pending`; `reconciliation` only while `ambiguous`; `inactivity` while runtime is not `exited`; `execution_deadline` once after admission regardless. A queue timeout's `execution.timeout.passed` precedes its `execution.admission.changed`. Partly guarded by the timeout fixtures.

**Scripted executor**

- **G-DISPATCH-POINT.** Dispatch is lazy. The first script step that stands for harness interaction dispatches first; so does the end of the script. `wait_until`, `wait_for`, `transport_errors`, `on_cancel`, `crash`, `stale_dispatch`, `probe_status` and `reconcile_finds` do not dispatch. `execution.recovery-revalidates-before-dispatch` depends on `wait_until` not dispatching (passes). `stall` is G-STALL.
- **G-CRASH-EXIT.** A crash step exits with status 1, only between requests. `before_dispatch` commits the script position first; `after_write` commits the marker and the attempt first.
- **G-MAX-ATTEMPTS.** Retryable effects (`idempotent_key` cancel forwarding, `read` status probes) get at most three attempts; `non_repeatable` effects get one. `execution.retries-follow-retry-class` gives the probe two transport errors, which any limit of three or more satisfies; the limits are otherwise unguarded.
- **G-CANCEL-OUTCOME.** `on_cancel` declares how the harness answers cancellation, whether it comes before or after the cancel. Without it the forwarding completes and no outcome is observed. When no forwarding attempt completes, the outcome is `unknown`. Exercised by `execution.cancel-returns-a-request-receipt`, `execution.cancel-refusal-survives-lost-acknowledgments` and the `e-key` script of `execution.retries-follow-retry-class`, which declares `on_cancel` before the cancel arrives.
- **G-HISTORY.** A delivery record's history gains an entry only when the determination changes. Evidence recorded while it stays `pending` (e.g. `transport_only`) replaces the current evidence. Pinned by `execution.delivery-wait-ends-without-evidence` step 6, whose exact history is one `pending` entry with `bytes_written` evidence; that step fails first on G-EVIDENCE-NAMES, and with those names aligned the history matches.
- **G-EXEC-REVISION.** An execution's revision rises by exactly one per execution event. Marker, script position and counters are stored without a revision change.
- **G-EXIT-STEP.** Only `{code}` or `{signal}` exit objects are accepted; any other shape refuses the launch, because inspect could not report it.
- **G-REQUIRES-ACTION-RUNTIME.** A `runtime: requires_action` step without a pending action is ignored, so `requires_action` always carries an identity (EXE-4). Unguarded.
- **G-STALE-DISPATCH-CURRENT.** `stale_dispatch` at the current generation is not stale and records nothing. Unguarded.
- **G-CONTEXT-OUTCOMES.** Harness report `accepted` maps to `delivered` and `lost` to `unknown`. `unavailable` for an unsupported boundary takes precedence over `late`. Passes `execution.context-delivery-reports-late-and-unavailable`.

**Admission and operations**

- **G-PRIMARY-PRECONDITION.** The one precondition the Execution and effect command schemas allow must name the command's own subject; otherwise `invalid_envelope`.
- **G-SUBMIT-PRECONDITION.** A submit precondition revision other than 0 is `invalid_envelope` ("Creates the execution (precondition revision 0)"), as for `core.grant.issue`. Unguarded.
- **G-NOT-FOUND-ORDER.** A missing target (the execution, a workspace lease, the controller host, the effect) is `not_found` after authorization and before step 7. A payload lookup (an action ID, an obligation) is `not_found` after preconditions. Unguarded for the order.
- **G-CAPACITY.** An admitted execution holds capacity until runtime `exited`, an observed exit, a `failed_before_delivery` or `not_delivered` delivery, or a `cancelled` outcome. Passes the capacity fixture, which uses an exit.
- **G-BUDGET.** The requested `amount` counts against the pool while `reserved` or `settled`. `settled` needs at least one usage observation. Passes `execution.usage-liability-survives-timeouts`.
- **G-ADMISSION-EVENT.** `execution.admission.changed` carries `delivery_id` for direct admission too, not only for a queued execution admitted later. Fixtures match by subset.
- **G-OUTPUT-OFFSET.** An `offset` beyond the end is clamped to `end_offset`. `max_bytes` defaults to 65,536 and shrinks until the response fits the caller's receive limit. Unguarded.
- **G-NEG-EXEC-ITEMS.** An `execution` request refused for its own unknown required feature also lists its missing Core features as `dependency_not_selected`. Unguarded; `execution.requires-core-features` passes.

**Authorization and visibility**

- **G-EFFECT-AUTH.** `core.effects.get` needs `execution.read` covering the effect's target, and `core.effects.abort_obligation` needs `core.effects.abort_obligation` on it. For an effect ID that does not exist, only a resource covering the whole `execution.execution` kind covers it, so a narrowed grant gets the same `out_of_scope` for a missing effect and an uncovered one. `execution.effects-resolve-only-with-target-authority` passes.
- **G-RECONCILE-AUTH.** `execution.reconcile` needs `execution.read`; a command ID is looked up in the caller's own deduplication scope, and executions the grant does not cover are omitted as if not held. Unguarded under grants.
- **G-CONTROLLER-VISIBILITY.** `execution.controller` subjects are visible under a grant with `execution.read` covering them, which also decides `current_epoch` disclosure. Unguarded.
- **G-INSPECT-GATING.** Feature members of `execution.inspect` (`steering`, `actions`, `workspace`, `usage`, `context`, `continuation`) appear only in sessions that negotiated the feature. `runtime_detail` is base.

**Core capabilities and snapshots**

- **G-ADAPTER-PREDICATES.** Configured adapter predicates are listed in `core.capabilities` beside `core-test.writes` (EXECUTION §10 reuses CORE §17). A changed adapter configuration across restarts therefore appends `core.capabilities.changed`. Unguarded.
- **G-EXEC-SNAPSHOT.** EXECUTION defines no retention snapshot state. An execution's state folds the axes its events carry, a controller's is `{epoch}`, and an effect's lists its aborted obligations. Unguarded.

**Not implemented, and so not claimed**

- **`core.events.backpressure`.** Launch keys `events.max_pending_notification_bytes` and `events.backpressure_notice_ms` refuse the start. The four slow-consumer fixtures are `unsupported`; one is recorded as a coverage limit for the undeclared signal `backpressure.limit.reached`.
- **Barriers and signals.** None declared; a launch that enables a barrier refuses to start.
- **The Unix-socket binding.** The 12 `socket.*` fixtures are `skipped`.
- **Effect history loss** (G-EFFECT-HISTORY-CODE), an inline brief (the submit schema allows only a digest reference), and real harness adapters.

### G.4 Verification

Run from the worktree root on macOS (Darwin 25.3) with Python 3.14.5 and the pinned Rust toolchain. The provider is real; the harness is the scripted executor. Results are under `conformance/results/` (not committed).

| Command | Exit | Result |
|---|---|---|
| `cargo build --workspace --locked` | 0 | built |
| `./target/debug/combraton-conformance run --participant conformance/participants/independent-python-core.json --out conformance/results/independent-m3-first` (at `188ab07`) | 1 | 176 pass, 19 fail, 4 unsupported, 12 skipped |
| the same run at the final commit, `--out conformance/results/independent-python-core`, and three repeats | 1 | 181 pass, 14 fail (G.2), 4 unsupported, 12 skipped; identical failing set each time |
| `python3 conformance/independent/python-core/tests/check_vectors.py` | 0 | `failures: 0` |
| `python3 conformance/independent/python-core/tests/probe_provider.py` | 0 | 19/19 probes as documented |
| `python3 conformance/independent/python-core/tests/probe_m2.py` | 0 | 43/43 probes as documented |
| `python3 conformance/independent/python-core/tests/probe_f.py` | 0 | 31/31 probes as documented |

Not established:

- behavior no fixture exercises, beyond ad hoc smoke runs that were not kept as probes;
- the sensitivity of the M3 fixtures to deliberate deviations (`tests/fixture_sensitivity.py` was not extended in this pass);
- crash consistency beyond scripted crashes and SIGKILL restarts;
- anything on the Unix-socket binding.

### G.5 Realignment after resolution

> Added in a fifth spec-only pass, on the same branch rebased onto the resolution commit `ad91182` ([M3-DIVERGENCES](../../../docs/work/release-0.1/M3-DIVERGENCES.md)). The read and write rules were unchanged. The resolution record and the diff of `docs/spec`, `schemas`, MATRIX and M3 between `d81e49b` and `ad91182` were read first and the change list below was written from them. The five fixtures at new versions were read only after the change list was implemented and the first run recorded.

The rebase replayed this branch's three commits without conflicts, and the runner was rebuilt.

**Change list, each with the resolution it follows.**

| # | Change | Resolution followed | Replaces |
|---|---|---|---|
| 1 | Obligation IDs `<effect>.evidence` (prompt, steering, action response), `<effect>.outcome` (cancel forwarding), `<effect>.result` (status probe); lease ID `<execution>.workspace`. Action responses and status probes now carry an obligation. | EXECUTION §15.1 "Identifiers" (G-OBLIGATION-ID, G-LEASE-ID) | `<effect>.obligation`, `<execution>.lease-1`, no obligation on responses and probes |
| 2 | Evidence classes from the §15.1 table: `recorded_before_dispatch`, `dispatch_intent`, the proof class, `transport_error`, `recovery` on the delivery record with `dispatch_uncertain` or `never_dispatched` on the effect, `delivery_timeout` or `delivery_timeout_before_dispatch`, `reconciliation`, `harness_response`, `harness_status`, `harness_observation`. The harness source is `scripted harness`. | §15.1 "Evidence classes" (G-EVIDENCE-NAMES) | own class names |
| 3 | Writing the dispatch marker appends a `pending` `dispatch_intent` observation to the prompt effect | §15.1 (G-DISPATCH-INTENT) | marker as executor state only |
| 4 | Dispatch happens only at `deliver`, `crash: after_write` and an unfenced `stale_dispatch`. Other steps and the end of a script no longer dispatch. Once delivery is not `pending`, `deliver` and `crash` steps have no effect. `stall` never dispatches. | §15.1 "Script steps" (G-DELIVER-STEP, G-STALL) | lazy dispatch at the first harness step (G-DISPATCH-POINT) |
| 5 | `wait_for: action` waits until every requested action is answered, then sets runtime `active` and sends each response once; the harness acknowledges it (`provider_ack_id`, effect `succeeded`, obligation satisfied) | §15.1 (G-RESPONSE-STATUS) | response attempt right after the command; effect left `pending` |
| 6 | `steering` and `actions` appear in inspect only once they have an entry | §15.1 "Inspect members" (G-ACTIONS-ABSENT) | present, possibly empty, in sessions with the feature |
| 7 | At a delivery timeout: `execution.timeout.passed`, then `core.effect.obligation.overdue` for each open obligation, then `execution.delivery.observed` | §15.1 "Event order" (G-OVERDUE-ORDER) | overdue after the determination |
| 8 | Every recovery decision advances the host generation; events are `execution.recovery.decided` (naming the new host), the delivery observation if any, then `execution.host.changed` | EXECUTION §7.1 item 3, §9, §15.1 (G-RECOVERY-HOST-EVENT) | only a resumed dispatch advanced it, with `execution.host.changed` first |
| 9 | A refused execution keeps runtime `preparing` | §3.1 (G-REFUSED-AXES) | runtime `unknown` |
| 10 | `execution_deadline` passes only while runtime is not `exited` | §8 (G-TIMEOUT-SCOPE) | passed regardless of runtime |
| 11 | `effect_history_unavailable` is a registered code with retry `after_reconcile`. Records are still never discarded, so it is never sent. | CORE §12 (G-EFFECT-HISTORY-CODE) | not registered |

Already as resolved, so unchanged:
- an absent controller epoch is `stale_authority_epoch` (CORE §8, G-EPOCH-ABSENT);
- `proof_class` only for proof-class evidence (§9, G-EVENT-PROOF-CLASS);
- correlation through inspect only (EXE-20, G-CORRELATION-EVENTS);
- the clock file (CORE §16.5, G-IDLE-EXPIRY-FIXED-CLOCK);
- `exit` sets runtime `exited` (§15.1, G-EXIT-RUNTIME);
- three attempts for retryable classes, one for `non_repeatable` (§15.1, G-MAX-ATTEMPTS);
- the remaining §8 timeout scopes;
- the notification split (G-NOTIFY-BATCH), which fixture version 2 no longer observes.

**Runs.** 211 fixtures each.

| Run | pass | fail | timeout | harness_error | unsupported | skipped |
|---|---|---|---|---|---|---|
| First run after changes 1–11 | 195 | 0 | 0 | 0 | 4 | 12 |
| After restoring G5-CAPACITY (below), and two repeats | 195 | 0 | 0 | 0 | 4 | 12 |

The four `unsupported` fixtures are the backpressure fixtures; one is listed under `coverage_limits` for the undeclared signal `backpressure.limit.reached`. The twelve `skipped` fixtures are `socket.*`. The probes and the vector check pass as in G.4.

After the first run, the five fixtures at new versions were read. They agree with the change list, including the reading in G5-RECOVERY-GENERATION: `execution.crash-during-dispatch-is-reconciled-without-resending` version 2 expects `execution.host.changed` with generation 2 after an ambiguous recovery.

**Remaining disagreements and underspecified points.** None of these fails a fixture.

- **G5-CAPACITY (behavior kept).** §15.1 says "Capacity is released when runtime is `exited`". It does not say what an execution holds when it can never run: a delivery `failed_before_delivery` (a delivery timeout before dispatch, or recovery) or `not_delivered`, or an outcome `cancelled`. Read as the only release condition, such an execution would hold capacity forever and queued work would wait for its queue timeout. This implementation released capacity in those cases before the resolution and still does. The step was briefly narrowed to runtime `exited` only for the first run, then restored; the capacity fixture passes either way. Suggested: state whether these cases release capacity.
- **G5-RECOVERY-GENERATION (underspecified).** §7.1 item 3, "Recovery advances the host generation", sits among the conditions for resuming dispatch, so it could be read as applying only to `dispatch_resumed`. The §15.1 event order ("the delivery observation if any, then `execution.host.changed`") and the crash-during-dispatch fixture imply every decision advances it, which is what this implementation now does. Suggested: say so in §7.1.
- **G5-SECOND-DELIVER (underspecified).** §15.1 says `deliver` is one dispatch attempt, and a delivery is never dispatched twice, but only says `deliver` has no effect once delivery is *not* `pending`. A second `deliver` while delivery is still `pending` (after `transport_only`, say) is either a second attempt, which the rule forbids, or further evidence. Here it records the harness report as further evidence for the same dispatch, with no new marker or attempt. Unguarded.
- **G5-TIMEOUT-OBLIGATIONS (underspecified).** At a delivery timeout the obligations become `overdue` before the determination. Whether a timeout-driven `failed_before_delivery` then satisfies them is not stated. Here they stay `overdue`, because the wait ending is not the expected observation, while an evidence-driven terminal determination satisfies them. Unguarded.
- **G5-OBSERVATIONS (decision).** Every harness report, reconciliation finding and timeout determination appends an effect observation, even when the status is unchanged (for example `pending` with `transport_only`), as CORE §19.1 "appended as evidence" and the `dispatch_intent` row suggest. Fixtures that list observations exactly contain no repeated status apart from `dispatch_intent`.
- **G5-CRASH-ATTEMPT (decision).** `crash: after_write` records the dispatch attempt as `completed`, since the write finished before the process died, and then exits. Unguarded.
- **G5-STALE-HIGHER (underspecified).** A `stale_dispatch` whose generation is *above* the current one is not fenced, so under §15.1 it dispatches if the delivery is still `pending`. A generation the executor never issued arguably should not dispatch. Unguarded.
- **G5-REFUSED-RUNTIME (followed, with a reservation).** §3.1 now keeps runtime `preparing` for a refused execution. That reads as work about to start. This implementation follows the text; an explicit "not started" value, or leaving runtime out for refusals, would be clearer.
