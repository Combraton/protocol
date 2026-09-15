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
- **H.** The M4 pass: `evidence/1`, `context/1` and `execution.context_revalidation` (tags `H-…` for readings before the fixtures, `H3-…` for changes after them).

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

### G.6 Final alignment

> Base `c3e79d6`, which resolves G.5 (section G of [M3-DIVERGENCES](../../../docs/work/release-0.1/M3-DIVERGENCES.md)). The rebase fast-forwarded. Read: the EXECUTION §7.1 and §15.1 changes, the resolution table, then the new fixture.

**Change:** one. A `stale_dispatch` naming any generation other than the current one is now fenced (`execution.dispatch.fenced`, nothing sent), including a generation above the current one that was never issued. This follows §15.1, which resolves G5-STALE-HIGHER. Before the change the suite at this base gave 195 pass, 1 fail (`execution.dispatcher-from-an-unissued-generation-is-fenced`, step 5: two events where three were expected), 4 unsupported, 12 skipped.

**The other G.5 points against the resolved text:**
- G5-RECOVERY-GENERATION (§7.1), G5-SECOND-DELIVER and G5-TIMEOUT-OBLIGATIONS (§15.1) were already implemented as now stated.
- G5-CAPACITY: §15.1 now releases capacity on `failed_before_delivery` and `not_delivered`, and allows release on `cancelled`, which this implementation does. Nothing is left in dispute.

**Run** (212 fixtures, twice): **196 pass, 0 fail, 0 timeout, 0 harness_error, 4 unsupported, 12 skipped.**

**Remaining disagreement:** only G5-REFUSED-RUNTIME, which the resolution leaves open for the owner. This implementation follows EXECUTION §3.1 (`preparing`).

### G.7 Acceptance corrections C1–C3

> Base `2d8402a`, the owner's conditional M3 acceptance with corrections C1–C3. I read the changes to EXECUTION §3, §3.1, §7.1, §9 and §15.1, CORE §16.5 and §19.4, the conformance README and the fixture schema first. The three re-versioned C1 fixtures came after the change list was implemented.

**Changes.**

| Correction | Change |
|---|---|
| C1 (EXECUTION §3.1, §9) | Every execution starts at runtime `not_started`; queued and refused executions stay there. Admission, direct or later from the queue, moves runtime to `preparing`. Every `execution.admission.changed` carries `runtime` after the change: `preparing` on admission, otherwise `not_started`, including a refusal by `queue_timeout`. The retention snapshot of an execution folds that `runtime`. This resolves G5-REFUSED-RUNTIME. |
| C3 (CORE §19.4) | A recovery decision `failed_before_delivery` with reason `deadline_passed` now leaves the delivery's obligations `overdue`. `core.effect.obligation.overdue` comes after `execution.recovery.decided` and before the delivery observation. Before this change they were satisfied (G7-RECOVERY-OBLIGATIONS, below). |

**C3 checked as general rules.** No change was needed for:
- **One dispatch per delivery (§3.1):** only the first dispatch point dispatches, and later reports while `pending` are evidence.
- **Recovery fencing (§7.1):** any non-current generation is fenced (G.6).
- **Causal order (§9):** `execution.timeout.passed` precedes the overdue marking and determination it causes, for the queue, delivery and reconciliation timeouts. `execution.recovery.decided` precedes the observation and host change it causes.
- **Ended waits:** a delivery or reconciliation timeout, or an obligation deadline, leaves obligations `overdue`; only harness evidence, reconciliation or provable non-dispatch satisfies them.
- **Evidence classes:** each states its basis, as §15.1 names them.

**C2 against this implementation's limits.** It still does not claim `core.events.backpressure`. The correction also restates that sessions without the feature are bounded and closed within `backpressure_notice_ms` of a stall's start. This implementation does not meet that. Its stdio writer blocks when the caller stops reading: it has no bound on pending output, no room deadline and no closure. README "Limits" now says so. No fixture that applies to this participant observes it; the backpressure fixtures are `unsupported`.

**Runs** (212 fixtures).

| Run | pass | fail | timeout | harness_error | unsupported | skipped |
|---|---|---|---|---|---|---|
| At `2d8402a` before the changes | 193 | 3 | 0 | 0 | 4 | 12 |
| After the changes, and one repeat | 196 | 0 | 0 | 0 | 4 | 12 |

Before the changes, the three failures were the C1 fixtures. `execution.admission-refuses-unenforceable-or-unknown-requirements` (step 5), `execution.capacity-queue-admits-in-order-and-times-out` (step 5) and `execution.context-bindings-gate-admission` (step 7) each found runtime `preparing` where `not_started` was expected.

**Underspecified points.**
- **G7-RECOVERY-OBLIGATIONS.** CORE §19.4 says an obligation is `overdue` when "a profile ends a wait because it timed out". §7.1 recovery also ends the delivery wait for reasons that are not timeouts: `cancelled`, `authorization_lost` and `recovery_policy`, each with an intact journal and no marker. Here those decisions satisfy the obligations, because provable non-dispatch is evidence of the effect's outcome (`never_dispatched`). A `deadline_passed` decision leaves them `overdue`. Neither text nor fixture states which applies, and no fixture observes it.
- **G7-EXIT-ORDER.** §9's causal-order rule covers decisions and passed timeouts only. For an observed process exit, this implementation appends `execution.runtime.changed` (`exited`) before `execution.exit.observed`. Whether an observation that causes another axis change must precede it is not stated.

### G.8 Exit causal order and recovery obligations

> Base `25f9c0b`, which resolves G.7 (section H of [M3-DIVERGENCES](../../../docs/work/release-0.1/M3-DIVERGENCES.md)). Read first: EXECUTION §7.1 "Obligations", the §9 `execution.exit.observed` row and "Causal order", and the CORE §16.5 "Declared bounds" change. Then the two fixtures at version 2.

**Change.** One. A scripted exit now appends `execution.exit.observed` first, then the separate `execution.runtime.changed` (`exited`) it causes (§9: causal order covers observations). The README limit on backpressure now says this provider makes no backpressure guarantee, as CORE §16.5 now scopes the bound to providers that implement the feature. Recovery obligations already matched §7.1 (G.7).

**Runs** (212 fixtures).

| Run | pass | fail | timeout | harness_error | unsupported | skipped |
|---|---|---|---|---|---|---|
| At `25f9c0b` before the change | 194 | 2 | 0 | 0 | 4 | 12 |
| After the change, and one repeat | 195 | 1 | 0 | 0 | 4 | 12 |

Before the change, the failures were `execution.evaluation-is-never-derived-from-exit` (step 5, event order) and the one below. The remaining failure is left failing, so `run` exits 1.

**Remaining disagreement.**

- **G8-INSPECT-OBLIGATIONS.** `execution.recovery-revalidates-before-dispatch` version 2, step 18, expects `execution.inspect`'s `obligations` to contain `e-cancel.delivery-1.evidence` with state `satisfied`.
  - **What the text says.** EXECUTION §4 defines `execution.inspect` as returning "Current axes, receipts, **open** obligations and an events cursor", and `execution.reconcile` as returning "any **open** obligations". CORE §19.4 treats `open` and `overdue` together as the obligations still waiting (`abort_obligation` refuses any other).
  - **This implementation.** Inspect lists `open` and `overdue` obligations only. A satisfied obligation is not open, so it is omitted; its state is observable through `core.effects.get` on `e-cancel.delivery-1`. The satisfaction the fixture wants to show does happen here, as §7.1 requires.
  - **Suggested.** Either §4 should say inspect lists every obligation of the execution's effects, or the fixture should read the obligation through `core.effects.get`.

The step 20 expectation of the same fixture (`deadline_passed` leaves the obligation `overdue` in inspect) passes, since `overdue` is listed.

---

## H. M4 pass (base 39e9dc8)

> Sixth spec-only pass: `evidence/1` (features `evidence.manifests`, `evidence.retention_control`, test control `evidence.store`), `context/1` (the six Context features, test control `context.script`, packets sealed in this provider's own store) and `execution.context_revalidation` for the single-provider case, with the bound-work rule of EVIDENCE §10. Branch `release-0.1/m4-independent` from `39e9dc8`. Read: EVIDENCE and CONTEXT in full; EXECUTION in full (§13.1–§13.2 are new); CORE §8, §10, §12, §15.5, §16.2–§16.3 and §16.6 again; STREAM §3 again; M4 in full; the M4 rows of MATRIX (EVD, CTX, EXE-21, SCN, CMP-8); the diff of VERIFICATION; `schemas/evidence/1`, `schemas/context/1` and the changed Execution and Core schemas; the conformance README; the changed parts of the fixture and launch-configuration schemas; and this directory. Decision 007 and M2-DIVERGENCES were not re-read (unchanged since the fifth pass); M3-DIVERGENCES gained one row, G8-INSPECT-OBLIGATIONS, resolved the way this implementation already behaved, so `execution.recovery-revalidates-before-dispatch` passes at the baseline. Not read: `conformance/reference/**`, `conformance/runner/src/**`, `conformance/crosscheck/**`, or their history. The runner was used as a binary. Sections A–G are left as written.

**Reading order.**

1. The documents above, and the diff of `docs/spec`, `schemas`, the conformance README and the fixture and launch-configuration schemas between `25f9c0b` and `39e9dc8`.
2. H.1 below, written before any M4 fixture was opened. It records how each open question was read.
3. An implementation from H.1, committed ("M4 independent: first implementation from documents") before any fixture in `conformance/fixtures/evidence/`, `conformance/fixtures/context/`, `conformance/fixtures/composition/` or the M4 `execution/` fixtures was opened.
4. The first complete run, recorded before anything changed. Only then the fixtures and transcripts.

**Baseline** at `39e9dc8` before any change: 246 fixtures, **196 pass, 0 fail, 30 unsupported, 20 skipped** (14 `evidence.*`, 9 `context.*`, 3 revalidation and 4 backpressure fixtures unsupported; 12 `socket.*` and 8 `composition.*` skipped).

### H.1 Readings before the fixtures

Each entry: location; question; what the documents say; what this implementation does; basis. Fixtures are named in H.3 once read.

#### Contradictions and defects noticed in the documents

- **H-CORE12-TABLE.** CORE §12. The sentence "Profile error codes are raised only after authorization (§15.5) …" was inserted in the middle of the error table, between `hold_active` and `capability_unavailable`. In Markdown this ends the table: the rows from `capability_unavailable` to `internal_error` render as a paragraph of pipe characters, not as registry rows. Implemented as if the table were whole. Basis: spec text (defect).
- **H-ARTIFACT-PROVIDER-OPTIONAL.** EVIDENCE §2 makes `provider` in an evidence reference "required whenever the reference may be read at another provider"; `schemas/evidence/1/common.schema.json#/$defs/reference` makes it optional everywhere, while the packet reference schema (context common) requires it. A reader cannot tell from a reference alone whether it "may be read at another provider". Here a reference without `provider` means this provider. Basis: own judgment.
- **H-TRANSITION-EXACTLY.** CONTEXT §3 says `transition` "is present exactly when the obligation is `required_before_transition`"; the item schema allows `transition` on any obligation and does not require it. The prose is enforced (`invalid_envelope` at `/payload/items/<i>/transition`). Basis: spec text over schema.
- **H-SUBMIT-STATES.** CONTEXT §3 lists six request states for the submit outcome; `context.request.submit.result` allows only `preparing` and `refused`. Consistent with "Steps advance no later than the provider's next request" (§12): a submit never returns a finished preparation. Implemented per the schema. Basis: schema.
- **H-HOLD-EXPIRED-STATE.** EVIDENCE §9 says "A released or expired hold stays inspectable"; the inspect schema's hold `state` is only `active` or `released`. Here an expired hold is recorded as released (see H-HOLD-EXPIRY). Basis: schema.

#### Evidence

- **H-EVD-NEG.** EVIDENCE §1: `evidence/1` requires `core.events`, "enforced at negotiation, as for Execution". Implemented like EXECUTION §1: one `dependency_not_selected` item with `feature: "core.events"`, refusal for a required request and `unselected` for an optional one; `core.describe` keeps `depends_on: ["core"]`. Basis: spec text.
- **H-EVD-FEATURE-OPS.** EVIDENCE §1, §9. `evidence.hold`, `evidence.release` and `evidence.purge` belong to `evidence.retention_control`: without it they are `unsupported_required_feature` at step 3. Base providers still report availability and holds in inspect (an empty list). Manifest completeness appears in `evidence.inspect` only in sessions that negotiated `evidence.manifests`; manifest content is validated at seal whenever the media type is the manifest type, because the provider implements the format regardless of the sealing session. Basis: own judgment.
- **H-CHUNK-LIMIT.** EVIDENCE §4 "Chunk sizing". `chunk_overhead_bytes` is declared as 2048 (the JSON-RPC and envelope bytes around the base64 string, with room for 128-character identifiers and a sha512 digest). The base64 length allowed is `min(max_payload_bytes − 2048, max_frame_bytes − 2048, max_string_bytes)`, and `chunk_limit` is the largest multiple of 3 whose base64 form fits, so it depends only on this provider's limits. The document does not say whether one overhead figure applies to both the payload and the frame limit; here it does. Basis: own judgment.
- **H-CHUNK-STEP.** An append over `chunk_limit` is `limit_exceeded` with `{ limit: "chunk_limit", maximum }`. The document does not place it in CORE §10. It is a declared limit, independent of any subject, so it is decided at step 2 after the schema checks, before authorization; it reveals nothing. Undecodable base64 (bad length or padding) is `invalid_envelope` at `/payload/data_base64`. Basis: own judgment.
- **H-EVD-STEP7-ORDER.** EVIDENCE §6: profile errors come "after authorization (step 6) and after the revision preconditions". `not_found` for a missing upload is not in that table. Here every Evidence command checks preconditions first (a nonexistent artifact has revision 0), then existence and state (`not_found`), then the §6 codes. For append: `upload_offset_mismatch` before `upload_size_exceeded`. For seal: `upload_incomplete`, then `content_digest_mismatch`, then manifest validation. Basis: spec text for the §6 codes, own judgment for `not_found`.
- **H-PREPARE-VALIDATION.** EVIDENCE §3. A `producer.principal` other than the session principal is `invalid_envelope` at `/payload/producer/principal`, decided at step 2 (the session principal is known then). A locator is refused as credential-bearing (`/payload/locator`) when it has user information (`scheme://…@host`) or a query parameter whose name contains `token`, `sig`, `signature`, `credential`, `password`, `secret`, `key` or `auth` (case-insensitive), or starts with `X-Amz-`. The document gives no test; this list is a heuristic. `capture.uncertainty` with `not_before` after `not_after` is `invalid_envelope`. Basis: spec text (the rule), own judgment (the test).
- **H-TERMINAL-COVERAGE.** EVIDENCE §3: "A source of kind `terminal_output` MUST NOT declare `complete` coverage of a tool trace." It is not said how a descriptor declares coverage "of a tool trace", nor which error applies. Here any descriptor whose `source.kind` is `terminal_output` with `coverage.completeness: "complete"` is `invalid_envelope` at `/payload/coverage/completeness`: the only coverage a descriptor declares is of its source, and terminal output never covers the trace (§13). Basis: own judgment.
- **H-CONTENT-DIGEST-ALG.** Descriptor digests use ENCODING §3. Both `sha256` and `sha512` are computed at seal; `core.digest-sha512` governs only command digests (CORE §10 step 4). Any other algorithm is `unsupported_digest_algorithm` with `{ algorithm, supported }` at prepare, step 7. Basis: own judgment.
- **H-AVAILABILITY-STATES.** EVIDENCE §9 names availability values but not what staged or abandoned artifacts report. Here: staged → `partial`; abandoned → `unavailable` with reason equal to the abandonment reason; sealed → `available`, or as the store and purge make it. `evidence_store.corrupt` makes a sealed artifact `unavailable` / `integrity_failed` at every read; `evidence_store.unavailable` makes it `unavailable` / `store_unavailable`. Neither affects staging or seal: the controls describe stored bytes, which exist once sealed. Neither is recorded as an event (§5). Basis: own judgment.
- **H-ABANDON-REASON.** EVIDENCE §4, §12: `evidence.artifact.abandoned { reason }` and inspect `abandoned_reason`. The reason for an explicit `evidence.upload.abandon` is not named; here `producer_abandoned`. A staging timeout uses `staging_expired`. Basis: own judgment (name), spec text (`staging_expired`).
- **H-STAGING-TIMEOUT.** EVIDENCE §4, README `staging_timeout_seconds`. Counted from `prepare` (not the last append), passing at `now ≥ staged_at + seconds`, applied before each request and on the idle re-check as a provider-origin `evidence.artifact.abandoned` that raises the artifact's revision. Basis: own judgment.
- **H-SEAL-REPEAT.** EVIDENCE §4. A new seal command on a sealed artifact (including one that is purge-pending or purged, whose state stays `sealed`) binds the command and returns `already_sealed: true` with the acknowledgment revision equal to the unchanged current revision, and no event. Basis: spec text.
- **H-FETCH.** EVIDENCE §5. Default `max_bytes` 65536. Order: existence (`not_found`), not sealed (`not_found`), reference digest (`artifact_digest_mismatch`, no details), availability (empty `data_base64`, `next_offset = offset`). An offset at or past `size` returns no bytes with `next_offset = offset`. The response shrinks until it fits the caller's receive limit, keeping at least one byte when one fits. Basis: spec text, own judgment for defaults.
- **H-QUERY.** EVIDENCE §5. Default `limit` 100; results in creation order; `next_cursor` present only when more matching readable artifacts remain; a cursor this provider did not issue is `invalid_cursor` with reason `malformed`. `filtered` is true whenever the session acts under a grant whose `evidence.read` resources do not cover the whole kind `evidence.artifact`; an authority acting without a grant gets `false`. A grant without `evidence.read` is `permission_denied` `right_missing`. Staged, abandoned and purged artifacts are listed. `digest` in an item is the declared digest. Basis: spec text (`filtered`), own judgment (the rest).
- **H-HOLD-VISIBILITY.** EVIDENCE §5 "holds the reader may see", §6 `hold_active` "the active holds the caller may inspect", §9 loss reports. No rule says who may see a hold. Here: an authority acting without a grant sees every hold; under a grant, a hold is visible when the session principal is its owner or the grant has `evidence.read` covering `{ kind: "evidence.hold", id }`. The same rule filters hold events. Basis: own judgment.
- **H-HOLD-PLACE.** EVIDENCE §8–§9. `evidence.hold` needs `evidence.hold` covering the artifact (not the hold subject). The artifact must exist, else `not_found` after preconditions; a hold on a purge-pending or purged artifact is `not_found` (nothing is retained any more). `expires_at` at or before the provider clock is `invalid_envelope` at `/payload/expires_at`, decided at step 6 like grant expiry. Events: `evidence.hold.placed` on the hold subject only; the artifact's revision is unchanged. Basis: own judgment.
- **H-HOLD-EXPIRY.** An active hold whose `expires_at` has passed is released by the provider (provider-origin `evidence.hold.released`, revision + 1) before the next request or on the idle re-check, and no longer blocks a purge. Basis: own judgment.
- **H-RELEASE-AUTH.** EVIDENCE §9: release by "the owner, by an authority principal, or under a grant with `evidence.release` covering that hold, delegated by the owner or an authority". Here: without a grant, the principal must be the owner or an authority (otherwise `grant_required`); with a grant, the session principal's ownership suffices, otherwise the grant needs `evidence.release` (`right_missing`) covering the hold (`out_of_scope`). The "delegated by the owner or an authority" clause is not checked: every grant chain starts at an authority (CORE §15.3), so the clause excludes nothing a provider can observe. Releasing a hold already released is `not_found`. Basis: spec text, own judgment for the clause.
- **H-PURGE.** EVIDENCE §9. Step 2: preconditions must be exactly the artifact plus one per named hold, else `invalid_envelope` at `/preconditions`. Step 6: `evidence.purge` on the artifact, then release authority for each named hold as in H-RELEASE-AUTH. Step 7: preconditions; the artifact must exist and be sealed (a staged or abandoned artifact is `not_found`: there are no sealed bytes to lose); a repeated purge returns the current availability, empty `released_holds`, unchanged revision, no event; each named hold must be an active hold on this artifact, else `not_found`; any other active hold is `hold_active` with `holds` = the blocking holds the caller may see (hold subjects) and `filtered` = whether blocking holds were withheld. Commit: CORE §16.3 puts the primary subject first, so `evidence.artifact.purge_requested` precedes the `evidence.hold.released` events, although EVIDENCE §9 lists the steps the other way. Basis: spec text, own judgment where noted.
- **H-DELETION.** README `deletion_delay_seconds` (default 0 when unset). Deletion is confirmed at `now ≥ requested_at + delay`, before the next request or on the idle re-check: stored bytes are deleted, availability becomes `purged`, `loss.confirmed_at` is set, provider-origin `evidence.artifact.purged { confirmed_at }`. With delay 0 the confirmation follows the purge's response. Basis: spec text, own judgment for the default.
- **H-LOSS-RECORD.** EVIDENCE §9 "Proof-loss record". Tracked dependency kinds are `hold`, `manifest` and `packet` (reported in `coverage.tracked`); `affected` entries are `{ kind, subject }`. The record is taken at the purge request: every hold on the artifact, every sealed manifest listing it by artifact, every packet citing it. At `evidence.inspect` it is filtered for the reader: holds by H-HOLD-VISIBILITY, manifests by `evidence.read` on the manifest artifact, packets by `context.packet.read` on the packet. `released_holds` is filtered the same way. `coverage.filtered` is true when anything was withheld. The kind names are not given by the document. Basis: own judgment.
- **H-MANIFEST.** EVIDENCE §7. Valid content: canonical-form-independent JSON object with exactly `format` (`combraton-evidence-manifest/1`) and `children`, each child exactly `role` (string 1–128), `evidence` (a reference) and `required` (boolean). Child state for a reader, in order: `unverified` when the reference names another provider; `withheld` when the reader may not read the child artifact (authorization only, whether or not it exists); `missing` when it does not exist, is not sealed, is purge-pending or purged, or its sealed digest differs; `unverified` when it is sealed with another availability (`unavailable`, integrity failure); otherwise `present`. Completeness: `complete` when every required child is `present`, else `incomplete` when a required child is `missing`, else `undetermined`. Basis: spec text, own judgment for order.
- **H-BOUND-WORK.** EVIDENCE §10. A grant's "work" resources are those whose kind is neither `evidence.artifact` nor `evidence.hold` (a hold is not work). When a prepare's grant has any, the descriptor's `work` must be covered by one of them, else `permission_denied` `binding_violation` after `right_missing` and `out_of_scope`. Only prepare checks it. Basis: spec text, own judgment for `evidence.hold`.
- **H-EVD-READ-RIGHTS.** Event visibility (CORE §16.6): `evidence.artifact` events need `evidence.read` covering the artifact; `evidence.hold` events follow H-HOLD-VISIBILITY. Basis: spec text (§8 "and their events").

#### Context

- **H-CTX-NEG.** CONTEXT §1: `context/1` "depends on `core/1` with `core.events`". The document does not repeat Evidence's "enforced at negotiation". Enforced the same way (`dependency_not_selected` with `feature: "core.events"`). `evidence/1` is not required in the same session, so `depends_on` stays `["core"]`. Basis: own judgment.
- **H-CTX-OBLIGATION-STEP.** CONTEXT §3: an item obligation whose feature was not negotiated is `unsupported_required_feature` with `features`, at step 3 — including `advisory`, whose feature is `context.advisory`. `features` lists each missing feature once, in item order. Basis: spec text.
- **H-CTX-SUBMIT-VALIDATION.** Step 2: duplicate `item_id` values are `invalid_envelope`; an `authority_content` entry naming no item is `invalid_envelope` at its `item_id`; a basis repository whose workspace is not `clean` with `dirty: null` needs `completeness: "partial"`. A `clean` workspace with a dirty snapshot is accepted. `fallback` defaults to `proceed_with_gap` when advisory items exist. Basis: spec text, own judgment for the extra checks and default.
- **H-CTX-IDS.** Job `<request>.job`; packet subject `{ kind: "context.packet", id: <request> }` (§5); packet artifact `packet.<request>.<revision>` (by analogy with EXECUTION §13.2's `output.<execution>.<completion_id>`). The artifact descriptor: producer the session principal, `source { kind: "context.packet", id: <request> }`, `scope: "context"`, media type `application/vnd.combraton.context-packet+json`, `retention_class: "default"`, `coverage.completeness` `complete` when the request is `ready`, otherwise `partial`, with the packet's gaps. Basis: spec text (packet ID, media type), own judgment (the rest).
- **H-CTX-BUDGET.** CONTEXT §3, §12. Mandatory size is the UTF-8 byte length of the `content` of the script's `section` steps whose `item_id` names a required item, before the first `publish`. It is decided at submit from the script of the job the request would start or join, so the submit outcome is `refused` with `budget_insufficient` and `needed`, with no job. Refused items are reported `unmet` (required) or `degraded` (advisory) with reason `budget_insufficient`. At publish, required sections are always included; other sections are included in packet order while they fit the remaining capacity, otherwise omitted with `output_capacity` and their advisory items `degraded` with `output_capacity`. Basis: spec text, own judgment for the counting window and item results.
- **H-CTX-SATISFACTION.** CONTEXT §3 `check`. An item is `satisfied` only by an included, non-historical section with its `item_id` for which the check holds: `source_included` — the section's `source` names the same repository and path; `evidence_included` — a section citation names the same artifact and digest; `authority_content_included` — the section's `authority_revision` equals the item's current authority revision (the request's `authority_content`, raised by any `correction`). Basis: own judgment.
- **H-CTX-UNSATISFIED-REASONS.** Reasons for items not satisfied at publication: an explicit `unmet` step's reason; `corrected_during_preparation` when a correction left no section at the corrected revision (§8, advisory items `degraded` with the same reason); `output_capacity` or an `omit` step's reason when the item's content was omitted; `investigation_budget_exhausted` or `deadline_passed` when those ended preparation; a job `end` step's reason; otherwise `unavailable`. Advisory items take `degraded`, required items `unmet`; a required item is never downgraded. Basis: spec text for the named reasons, own judgment for the precedence and `unavailable`.
- **H-CTX-DEADLINE.** CONTEXT §5, CTX-7. When the provider clock reaches a preparing request's deadline, its packet is published at once with every unsatisfied item `unmet`/`degraded` `deadline_passed`, whatever the script is doing. A `publish` step waits only while some advisory item is unsatisfied and the fallback is `wait_until_deadline`; a publish with an unsatisfied required item does not wait. Basis: spec text.
- **H-CTX-INVESTIGATE.** An `investigate` step that would take the job's spent units above the starting request's `limits.investigation.amount` spends nothing: the job's preparing requests are published with unsatisfied items `investigation_budget_exhausted`, then `context.job.ended { reason: "investigation_budget_exhausted" }`, and the script stops. Spending exactly the amount is allowed. Basis: own judgment.
- **H-CTX-JOB-END.** Jobs end at an `end` step (reason as given; preparing requests are first published as in H-CTX-UNSATISFIED-REASONS), at budget exhaustion, or when a cancel leaves no preparing subscriber (`no_subscribers`, recorded in the cancel's own transaction). Running out of script steps does not end a job. Basis: own judgment.
- **H-CTX-SHARED.** CONTEXT §4. A request joins an existing job only when its session negotiated `context.shared_jobs`, the job has not published and has not ended, and the basis and items are canonically equal and the submitting principal is the same. The job's script is the one of the request that started it. Basis: own judgment for "compatible".
- **H-CTX-CANCEL.** CONTEXT §4, §11. Cancel checks preconditions, then `not_found` for a nonexistent request or any state other than `preparing` (including `cancelled`). It appends only `context.request.cancelled { job_continues }` (the state change it names), then `context.job.ended { reason: "no_subscribers" }` on the job when `job_continues` is false. Basis: spec text, own judgment for the event set.
- **H-CTX-PUBLISH-EVENTS.** CONTEXT §11. At each publication, in one provider transaction: the packet artifact's `evidence.artifact.staged`, `appended` and `sealed` (provider-origin, §11 "together with the sealed artifact's Evidence events"), then `context.packet.published { reference, items }`, then `context.request.changed { state, job }` when the state changed. A shared job publishes one packet per preparing subscriber in submission order. Basis: spec text, own judgment for order.
- **H-CTX-UPDATES.** CONTEXT §8. Every later `publish` step creates revision n + 1 (with `supersedes { revision: n }`, the previous revision reported `current: false`) for each already-published subscriber whose submitting session negotiated `context.updates`. The update carries all sections so far, with correction flags, and the same basis. Its request state is recomputed. Basis: spec text.
- **H-CTX-CORRECTION.** CONTEXT §8 CTX-13. A `correction { item_id, authority_revision }` raises the item's current authority revision; every section for that item with a lower `authority_revision` is then `historical: true` with label `stale`, both in facts and in packet bytes. Sections without `authority_revision` are untouched. Basis: spec text.
- **H-CTX-SCRIPT-SHAPES.** The launch-configuration schema leaves step members loose. Accepted: `section { section_id, label, content, item_id?, citations?, authority_revision?, source? }` with `citations` a list of `{ citation_id, evidence }` and `source { repository, path, tree }`; `coverage { producer, frontier, gaps }`; `unmet { item_id, reason }`; `omit { reason, item_id?, section_id? }`; `correction { item_id, authority_revision }`; `conditions` a list of condition objects (context common schema); `publish {}`; `end` a reason string; `stall: true`; `wait_until` an instant; `investigate` an integer. `execute` and `context.executor` / `context.evidence_provider` need sockets and refuse the start (exit 2). Basis: schema, own judgment for the shapes the schema leaves open.
- **H-CTX-EXPAND.** CONTEXT §6. Step 6 needs `context.packet.read` covering the packet and the right `evidence.read` (existence-independent, so `right_missing` reveals nothing). Then the packet and revision must exist (`not_found`). A citation that is not in that revision, names another provider, names an artifact that does not exist here, or is not covered by the grant's `evidence.read` resources is `permission_denied` with `out_of_scope`, identically, for authorities too. A citation whose artifact's sealed digest differs is `artifact_digest_mismatch`; an unsealed or unavailable one returns an empty excerpt. Basis: spec text, own judgment for the reason.
- **H-CTX-PACKET-INSPECT-RIGHTS.** CONTEXT §6, §10. `context.packet.inspect` needs only `context.packet.read`, although its excerpt carries packet bytes and §10 says "packet bytes are additionally subject to the Evidence rights of their artifacts". §7 has the executor read facts at the context provider under the context grant alone, so the excerpt is treated as part of the facts. Basis: own judgment (tension noted).
- **H-CTX-EVENT-VISIBILITY.** CONTEXT §10: `context.read` covers request events and "events of their jobs". A job event is visible under a grant with `context.read` covering any request attached to that job. Basis: spec text.

#### Execution context revalidation (single provider)

- **H-REVAL-SCOPE.** EXECUTION §13.1. Revalidation applies to executions submitted in a session that negotiated `execution.context_revalidation`; others keep the M3 rules exactly. `origin` (§13.2) is accepted on every submit and shown in inspect, because the submit schema does not gate it. `executor.peers` and `executor.evidence_outputs` need sockets and refuse the start. Basis: spec text, own judgment for `origin`.
- **H-REVAL-HELD.** "Holds the packet": a binding's packet matches an `executor.context_packets` entry by exact equality in the same form (`{ ref, digest }` or `reference`), or across forms when `ref` equals the packet ID and `digest` equals the artifact digest. A `fetch` member cannot be used here (no peers): when present and the packet is not held, the check's `fetch` is `provider_unreachable`, and a `fetch.context` member adds result `packet.current` = `unavailable`. Basis: own judgment.
- **H-REVAL-OBSERVE.** Launch `observed_basis` is the executor's initial observation; script step `observe_basis` merges into that execution's observed basis per repository and per member. `repository_tree` and `dirty_snapshot` compare with the observed repository's `tree` / `dirty_snapshot`, `environment_digest` with `environment_digest`; an absent observation is `unavailable`; `authority_revision` is always `unavailable`. `observed` is given for match and mismatch. Evidence strings: `observed_basis` and `not_observed`. Basis: spec text, own judgment for strings.
- **H-REVAL-BOUNDARIES.** Checks at admission (at submit and at each re-evaluation of a queued execution) and at dispatch cover `advisory` and `required_before_start` bindings; a `required_before_transition` binding is checked only at its named `transition` step. The admission and first dispatch check are always recorded; a re-evaluation records a check only when `held`, `state` or the results (condition, result, observed) differ from the last check of that binding at that boundary. `execution.context.checked` carries the check record and precedes the `execution.admission.changed` it informs. Basis: spec text, own judgment for advisory coverage and the comparison.
- **H-REVAL-QUEUE-REASON.** With several required-before-start bindings not current, the queue reason is `context_binding_stale` if any is stale, else `context_binding_unsatisfied` if any is not held, else `context_binding_unknown`. `context.blocked` names the first such binding in submission order with its own reason. Basis: own judgment.
- **H-REVAL-BLOCKS.** Dispatch: the first dispatch point of the script (EXECUTION §15.1) is held while a required-before-start binding is not current; `context.blocked` is set and cleared when it becomes current (no event besides checks). Transition: the `transition` step is held; `execution.transition.blocked { boundary, binding_id, transition, reason }` is appended when a block begins (not on every re-evaluation), and the transition is observed once current. A blocked dispatch keeps its capacity slot (M4 owner point 4 is still open). Basis: spec text, own judgment for event repetition.
- **H-REVAL-STATE-VIEW.** A binding's `revalidation` in inspect is evaluated at the read against the current observed basis and holdings, not copied from the last check. Its M3 `state` is `satisfied` only while `current`. Sessions without the feature see `{ ref, digest }`, no revalidation members, `checks` or `blocked`, and a stale or unknown `queue_reason` shown as `context_binding_unsatisfied`. Events are not rewritten per session: `execution.admission.changed` carries the real queue reason. Basis: spec text, own judgment for events.

### H.2 First complete run

Commit `cd411d5` ("M4 independent: first implementation from documents"), before any M4 fixture was opened: 246 fixtures, **205 pass, 17 fail, 0 timeout, 0 harness_error, 4 unsupported, 20 skipped**. Every fixture that passed at the baseline still passed. The 17 failures, as the runner reported them:

| Fixture | Step | Runner reason |
|---|---|---|
| `context.correction-during-preparation-never-becomes-current` | 7 | `result/authority: expected 1 items, found 0` |
| `context.deadline-leaves-required-unmet-and-advisory-follows-fallback` | 5 | `result/items/0/result: expected "satisfied", found "unmet"` |
| `context.expand-returns-only-authorized-citations` | 10 | `result/state: expected "ready", found "unmet"` |
| `context.limits-are-separate-and-mandatory-content-is-never-dropped` | 3 | `result/items/0/result: expected "satisfied", found "unmet"` |
| `context.packet-is-an-exact-sealed-evidence-artifact` | 5 | `result/outcome/job/id: expected "r-1", found "r-1.job"` |
| `context.shared-job-survives-one-subscriber-cancelling` | 2 | `result/outcome/job/id: expected "r-1", found "r-1.job"` |
| `context.updates-are-new-revisions-and-old-bytes-stay-historical` | 3 | `result/state: expected "ready", found "unmet"` |
| `evidence.chunk-retransmission-and-conflicts` | 8 | `error upload_offset_mismatch must have retry no, received "after_reconcile"` |
| `evidence.descriptor-provenance-coverage-and-locator-rules` | 26 | `result/items/0/artifact/id: expected "eq-1", found "l-3"` |
| `evidence.hold-release-needs-ownership-or-delegated-authority` | 25 | `result/holds: expected 2 items, found 0` |
| `evidence.integrity-failure-is-never-served` | 15 | `result/items/1/artifact/id: expected "ok-1", found "u-1"` |
| `evidence.manifest-completeness-respects-authorization` | 17 | `result/outcome/availability: expected "purged", found "purge_pending"` |
| `evidence.partial-upload-then-append-then-seal` | 4 | `error upload_incomplete must have retry no, received "after_reconcile"` |
| `evidence.purge-requires-release-authority-for-holds` | 19 | `error hold_active must have retry no, received "after_reconcile"` |
| `evidence.seal-refuses-digest-mismatch-and-is-idempotent` | 7 | `result/abandoned_reason: expected "abandoned_by_producer", found "producer_abandoned"` |
| `evidence.upload-seal-and-fetch-exact-bytes` | 4 | `result/availability/state: expected "available", found "partial"` |
| `execution.revalidation-repeats-at-dispatch-and-after-queueing` | 8 | `result/admission: expected "admitted", found "queued"` |

Passing on the first run, with readings from H.1 that the fixtures then confirmed: `evidence.access-reveals-no-existence` (H-QUERY `filtered`, `out_of_scope` for unreadable artifacts), `evidence.chunk-limit-accounts-for-encoding-overhead` (H-CHUNK-LIMIT: the fixture expects exactly `chunk_overhead_bytes: 2048` and `chunk_limit: 3072`, the values chosen here), `evidence.publish-grant-bound-to-work` (H-BOUND-WORK), `evidence.requires-core-events-and-gates-retention-control` (H-EVD-NEG, H-EVD-FEATURE-OPS), `evidence.staging-timeout-reports-abandoned` (H-STAGING-TIMEOUT), `context.request-items-are-checkable-and-obligations-negotiated` (H-TRANSITION-EXACTLY, H-CTX-OBLIGATION-STEP, and reason `unavailable` from H-CTX-UNSATISFIED-REASONS), `context.requires-core-events-and-gates-optional-features` (H-CTX-NEG), `execution.context-revalidation-is-a-negotiated-extension` and `execution.revalidation-reports-match-mismatch-and-unavailable-by-obligation` (H-REVAL-SCOPE, H-REVAL-HELD, H-REVAL-OBSERVE, H-REVAL-BOUNDARIES, H-REVAL-QUEUE-REASON, H-REVAL-STATE-VIEW). Store reason `store_unavailable` (H-AVAILABILITY-STATES) and packet artifact ID `packet.<request>.<revision>` (H-CTX-IDS) also matched.

### H.3 Changes after reading the fixtures

Committed as `fc42747` ("M4 independent: align with the M4 fixtures where the documents are silent"), plus H3-DEADLINE-PUBLISH and H3-OBSERVED-BASIS-RESTART in the same commit. Each entry: location; question; what the documents say; the fixture expectation; what changed; basis.

- **H3-STAGED-AVAILABILITY.** EVIDENCE §9 lists `partial` among availability values without saying what it means; §5 says a provider never serves "partial, pending or unverified bytes". H-AVAILABILITY-STATES reported a staged artifact as `partial`. `evidence.upload-seal-and-fetch-exact-bytes` step 4 expects `availability.state: "available"` for a staged artifact with 16 of 33 bytes. Changed to `available`; abandoned artifacts still report `unavailable` with their reason (unchecked). What `partial` availability is for is left unstated. Basis: fixture-informed.
- **H3-ABANDON-REASON.** EVIDENCE §4, §12 name no reason for an explicit abandon. `evidence.seal-refuses-digest-mismatch-and-is-idempotent` steps 7 and 16 expect `abandoned_by_producer` (inspect and event). Changed from `producer_abandoned`. Basis: fixture-informed.
- **H3-QUERY-ORDER.** EVIDENCE §5 says "bounded page; opaque cursor", not an order. `evidence.descriptor-provenance-coverage-and-locator-rules` step 26 and `evidence.integrity-failure-is-never-served` step 15 expect items in artifact ID order (`eq-1, eq-2, l-3, l-5, loc-a`; `c-1, ok-1, u-1`), not creation order, with array patterns that match exactly. Changed to ID order; the cursor now names the last ID returned. Pagination itself is unchecked. Basis: fixture-informed.
- **H3-HOLD-VISIBILITY.** EVIDENCE §5 "holds the reader may see" has no rule (H-HOLD-VISIBILITY). `evidence.hold-release-needs-ownership-or-delegated-authority` step 25 expects a reader under a grant with only `evidence.purge` and `evidence.read` on the artifact to see both holds, owned by others; `evidence.purge-requires-release-authority-for-holds` step 19 expects a grant without `evidence.read` to see none (`holds: [], filtered: true`). Now a reader of the held artifact also sees its holds; owner and hold-subject coverage still count. This rule also filters hold events, `hold_active` details and loss records. Basis: fixture-informed.
- **H3-HOLD-ORDER.** Not stated. `evidence.purge-requires-release-authority-for-holds` step 25 expects holds `h-1` then `h-own` although `h-own` was placed first. Holds in inspect, `hold_active` and loss records are now in hold ID order. Basis: fixture-informed.
- **H3-HOLD-ACTIVE-DETAILS.** EVIDENCE §6: `holds`, "the active holds the caller may inspect", shape unstated. The same fixture, step 23, expects `{"$exact": {"holds": ["h-own"], "filtered": false}}`: hold IDs, not hold subjects. Changed. Basis: fixture-informed.
- **H3-LOSS-KINDS.** EVIDENCE §9: `affected` lists "holds, manifests listing the artifact, packets citing it, outcome references", and `coverage` "declares the dependency kinds the provider tracks". Kind names are not given. The same fixture, steps 34 and 43, expects `affected` kinds `evidence.hold` and `evidence.manifest_child` and exactly `tracked: ["evidence.hold", "evidence.manifest_child"]`. Changed from `hold`/`manifest`/`packet`, and packets citing a purged artifact are no longer tracked. A provider that also tracks packets, as the text invites, fails that exact array. Basis: fixture-informed.
- **H3-IMMEDIATE-DELETION.** EVIDENCE §9: "Physical deletion is confirmed separately … Until then the provider reports `purge_pending`"; the README gives `deletion_delay_seconds` no default. `evidence.manifest-completeness-respects-authorization` step 17, with no `evidence_store` control, expects the purge outcome itself to be `purged`. Without a configured delay (or with 0), deletion is now confirmed in the purge's own transaction: `evidence.artifact.purge_requested`, then `evidence.artifact.purged`, both command-origin, outcome `purged`, and the acknowledgment carries the artifact's final revision (unchecked). With a delay, H-DELETION is unchanged. Basis: fixture-informed.
- **H3-JOB-ID.** CONTEXT §2 gives a job its own identity ("Must not be used as: a request"). `context.packet-is-an-exact-sealed-evidence-artifact` step 5 and `context.shared-job-survives-one-subscriber-cancelling` step 2 expect `{ kind: "context.job", id: "r-1" }` for a job started by request `r-1`. Changed from `r-1.job`. The two identities still differ by kind. Basis: fixture-informed.
- **H3-PACKET-PRODUCER.** EVIDENCE §3: the producer "is always the session principal of the `prepare` command"; a packet the provider seals itself has no prepare. `context.packet-is-an-exact-sealed-evidence-artifact` step 10 expects `producer.principal: "conformance-provider"`, the provider ID. Changed from the session principal. CONTEXT §12 gives the provider its own principal only when sealing at a separate evidence provider. Basis: fixture-informed.
- **H3-CTX-SATISFACTION.** CONTEXT §3: "`check` states how satisfaction is decided". H-CTX-SATISFACTION applied each check kind. `context.deadline-leaves-required-unmet-and-advisory-follows-fallback` step 5, `context.expand-returns-only-authorized-citations` step 10, `context.limits-are-separate-and-mandatory-content-is-never-dropped` step 3 and `context.updates-are-new-revisions-and-old-bytes-stay-historical` step 3 expect `source_included` items satisfied by scripted sections that carry no `source`. An included, non-historical section with the item's ID now satisfies the item, whatever its check says; checks are still validated at submit. This makes `check` decorative for the scripted provider, which the document's wording does not suggest. Basis: fixture-informed (conflicts with the purpose §3 gives `check`).
- **H3-LIVE-ITEMS.** CONTEXT §3: "Item results: `pending` while preparing; then `satisfied`, `unmet` … or `degraded`". `context.deadline-leaves-required-unmet-and-advisory-follows-fallback` steps 6 and 7 expect, for requests still `preparing`, items already `satisfied` beside `pending` ones. `context.request.inspect` now reports, while preparing, `satisfied` for items the job's content satisfies and `pending` for the rest. Basis: fixture-informed (the text reads as all items `pending` while preparing).
- **H3-AUTHORITY-ENTRIES.** CONTEXT §5: `authority` is "the revision included and what it rests on". `context.correction-during-preparation-never-becomes-current` step 7 expects `authority: [{ item_id: "i-rule", authority_revision: 2 }]` in revision 1, whose only section for the item is at revision 1 and historical. Each authority-content item now has an entry at its current (corrected) authority revision, whether or not content at that revision is included. Basis: fixture-informed (conflicts with "the revision included").
- **H3-PUBLISH-REVISION.** Not stated. `context.shared-job-survives-one-subscriber-cancelling` step 8 cancels request `r-2`, published (`ready`) after one submit, with precondition revision 2 and expects `not_found`. Under H-CTX-PUBLISH-EVENTS the request was at revision 3 (published and changed), so preconditions failed first. Two readings fit the fixture: a publication raises the request's revision once, or cancel checks the request's state before preconditions. The first was chosen, because EVIDENCE's fixtures put preconditions before state errors: `context.packet.published` and `context.request.changed` now share the new revision. Basis: fixture-informed; the alternative is equally consistent with the fixture.
- **H3-DEADLINE-PUBLISH.** CONTEXT §12: "A `publish` whose advisory items are unsatisfied under `wait_until_deadline` waits for the deadline." With `context.updates` negotiated (optional in that fixture), the deadline publication was followed by the waiting step publishing revision 2 with reason `unavailable`. `context.deadline-leaves-required-unmet-and-advisory-follows-fallback` step 10 expects one packet with `deadline_passed`. The publication at the deadline is now the one the waiting step stood for. Basis: spec text (a misreading fixed), found by the fixture.
- **H3-OBSERVED-BASIS-RESTART.** README `executor.observed_basis` "what revalidation can observe", a launch key applied at each start. The first implementation copied it into the execution at submit. `execution.revalidation-repeats-at-dispatch-and-after-queueing` step 8 restarts with another `observed_basis` and expects the queued execution to be re-checked against it and admitted. The launch basis is now read at every start, with the execution's own `observe_basis` steps merged over it. Basis: spec text (launch configuration is per start), found by the fixture.

**Not changed: H-RETRY-RUNNER.** CORE §12 and EVIDENCE §6 give `upload_offset_mismatch`, `upload_incomplete` and `hold_active` retry `after_reconcile`, and M4-Q5 says "`upload_incomplete` is `after_reconcile`, not `same_command`". The fixtures state no retry class; the runner checks it itself and requires `no`:
- `evidence.chunk-retransmission-and-conflicts` step 8: `error upload_offset_mismatch must have retry no, received "after_reconcile"`;
- `evidence.partial-upload-then-append-then-seal` step 4: `error upload_incomplete must have retry no, received "after_reconcile"`;
- `evidence.purge-requires-release-authority-for-holds` step 19: `error hold_active must have retry no, received "after_reconcile"`.

The runner's expectation contradicts both documents, so this implementation keeps the documented classes and the three fixtures fail. A scratch copy (never committed) answering `no` for these codes passes all 14 `evidence.*` fixtures, so nothing else hides behind these steps. The runner's registry seems to lack the new Evidence codes, or to default them to `no`; the runner source was not read to confirm this. M4 reports these fixtures green for the reference provider, which would mean it also sends `no`, against §12. Suggested: give the runner, and the reference if it differs, the CORE §12 classes for the six Evidence codes.

### H.4 Remaining failures

Three, all H-RETRY-RUNNER above. No other M4 fixture fails.

### H.5 Requirements no fixture checks

None of these is exercised by a fixture this participant runs (the composition fixtures need the socket binding and are skipped).

- **Evidence.** Hold `expires_at` and expiry release (H-HOLD-EXPIRY); query pagination and cursors (H-QUERY); `unsupported_digest_algorithm` for a descriptor digest (H-CONTENT-DIGEST-ALG); fetch and packet excerpts shrinking to the caller's receive limit; `evidence.availability.changed` (never recorded here, since integrity failures are only observed at reads); abandoned-artifact availability; releasing an already released hold; a hold on a purge-pending artifact; purge of a staged or abandoned artifact; manifest validation when the sealing session did not negotiate `evidence.manifests`; a manifest child that is sealed but unavailable (`unverified`); `released_holds` filtering in a loss record; a staging timeout counted from prepare rather than from the last append; the two credential-locator forms beyond user information and `X-Amz-Signature` (H-PREPARE-VALIDATION).
- **Context.** Script steps `end` and `unmet`; `omit` naming an item or a section; conditions of kind `dirty_snapshot` and `environment_digest`; a cancel that leaves no subscriber (`context.job.ended` `no_subscribers`); item results of a refused request; `context.expand` of a citation whose artifact digest differs, or is unavailable; `context.read` visibility of job events; shared jobs with different fallbacks or deadlines; corrections on advisory items; `origin` on a context request.
- **Revalidation.** A blocked dispatch or transition becoming current again (block cleared, check recorded, work continues); advisory checks at dispatch; a `required_before_transition` binding held but unknown; `fetch` and the implicit `packet.current` condition, `request` beyond echo, and `origin` on an execution submit outside compositions; recording a new check only when the result differs (checked only indirectly by the exact `checks` arrays of `execution.revalidation-repeats-at-dispatch-and-after-queueing`).

### H.6 Verification

| Run (base `39e9dc8`) | pass | fail | timeout | harness_error | unsupported | skipped |
|---|---|---|---|---|---|---|
| Baseline, before this pass | 196 | 0 | 0 | 0 | 30 | 20 |
| First complete run (`cd411d5`) | 205 | 17 | 0 | 0 | 4 | 20 |
| Final run | 219 | 3 | 0 | 0 | 4 | 20 |

Filtered final runs: `--filter evidence.` 14 fixtures, 11 pass, 3 fail; `--filter context.` 9 pass; `--filter revalidation` 3 pass. The full run was repeated once, and the `context.` and `execution.` subsets twice more, with identical results. The four `unsupported` fixtures are the backpressure fixtures; the 20 `skipped` are 12 `socket.*` and 8 `composition.*`. `tests/check_vectors.py`, `probe_provider.py`, `probe_m2.py` and `probe_f.py` pass as before.

### H.7 Realignment (base 8500948)

> Seventh spec-only pass. The branch was fast-forwarded to `8500948`, which resolves section H ([M4-DIVERGENCES](../../../docs/work/release-0.1/M4-DIVERGENCES.md)). Read first: that record, then `git diff 4a1d58a 8500948 -- docs/spec schemas conformance/README.md conformance/schemas` (CONTEXT, CORE, EVIDENCE, EXECUTION, the context common schema and the evidence inspect result schema; the README and conformance schemas did not change). The change list below was written from those alone, before the re-versioned and new fixtures were opened. The runner was rebuilt at this base. Read rules unchanged: no `conformance/reference/**`, `conformance/runner/src/**`, `conformance/crosscheck/**`, or their history.

**What changed for this implementation, from the documents.**

| # | Resolved text | Before | Change |
|---|---|---|---|
| 1 | CONTEXT §12 "Scripted content and checks", §3: the item's `check` decides satisfaction | H3-CTX-SATISFACTION: any section for the item | Back to H-CTX-SATISFACTION: `source_included` needs the section's `source` repository and path, `evidence_included` a citation of the exact artifact and digest, `authority_content_included` a section at the current authority revision. Live item results (H3-LIVE-ITEMS, now CONTEXT §3) follow the same test. |
| 2 | CONTEXT §12 conventions: compiler `combraton-reference-context` | compiler `combraton-independent-python-core` | Adopted; job ID and packet artifact ID already matched. |
| 3 | EVIDENCE §9: hold states `active`, `released`, `expired`; `evidence.hold.expired` at `expires_at`; an expired hold does not block purge; releasing it is `not_found` | H-HOLD-EXPIRY: expiry recorded as a provider-origin `evidence.hold.released` | Expiry sets state `expired` with provider-origin `evidence.hold.expired`. |
| 4 | EVIDENCE §9: a hold on an artifact that is not sealed, or whose purge was requested, is `not_found` | staged artifacts could be held | Only sealed artifacts without a purge request can be held. |
| 5 | EVIDENCE §9 "Visibility": owner, authority principals, readers of the held artifact; hold events alike | H3-HOLD-VISIBILITY also admitted `evidence.read` on the hold subject | That extra case removed. |
| 6 | EVIDENCE §3: `not_before` after `not_after` is `invalid_envelope` at `/payload/capture/uncertainty` | path `/payload/capture/uncertainty/not_after` | Path changed. |
| 7 | EVIDENCE §9 proof-loss record: `affected` in kind then ID order; candidate kind `context.packet_citation` | holds, then manifests; packets no longer tracked (H3-LOSS-KINDS) | Entries sorted by kind then ID. Packets citing the artifact are tracked again, as `context.packet_citation`, and `tracked` lists it. |
| 8 | EXECUTION §13.1: `revalidation` is the state of the binding's latest check | H-REVAL-STATE-VIEW: evaluated at the read | Taken from the latest check; see H7-REVALIDATION-NO-CHECK for a binding without one. |
| 9 | CORE §12 retry classes, runner fixed | H-RETRY-RUNNER | No change here; the classes were already `after_reconcile`. |

**Already as resolved, so unchanged:** `chunk_limit` at step 2 as the largest multiple of 3 (H-CHUNK-STEP, H-CHUNK-LIMIT); purge appends the artifact's events before released holds, including an immediate `purged` (H-PURGE, H3-IMMEDIATE-DELETION); queue-reason priority stale, unsatisfied, unknown (H-REVAL-QUEUE-REASON); `artifact_digest_mismatch` for a readable citation with another digest (H-CTX-EXPAND); abandoned artifacts `unavailable` with their reason and staged ones `available` (H3-STAGED-AVAILABILITY); a foreign cursor `invalid_cursor` and query in ID order (H3-QUERY-ORDER); shared jobs only for the same principal (H-CTX-SHARED); `expires_at` later than the clock (H-HOLD-PLACE); content digests `sha256` and `sha512`, others `unsupported_digest_algorithm`, wrong lengths `invalid_envelope` (H-CONTENT-DIGEST-ALG); packet producer, one revision per publication and authority revisions after correction (H3-PACKET-PRODUCER, H3-PUBLISH-REVISION, H3-AUTHORITY-ENTRIES); `transition` exactly for `required_before_transition` (H-TRANSITION-EXACTLY, now also the schema).

**Points the resolved documents still leave open, noted before the fixtures.**

- **H7-SHA512-ADVERTISED.** EVIDENCE §3 accepts `sha512` content digests "when it advertises `core.digest-sha512`". This provider advertises that feature in `core.describe` (D-SHA512), but the participant descriptor has never claimed it, because the runner's feature claims gate fixtures. So the provider accepts sha512 content digests while the descriptor does not claim them. Kept; if a fixture depends on the claim, the descriptor gains `core.digest-sha512`. Basis: spec text.
- **H7-DIGEST-ALG-STEP.** EVIDENCE §3 names `unsupported_digest_algorithm` for a descriptor digest, but not its CORE §10 step. CORE step 4 decides the *command* digest algorithm; this one depends on no subject either. Kept at step 7 after preconditions, as before, pending the fixture. Basis: own judgment.
- **H7-REVALIDATION-NO-CHECK.** EXECUTION §13.1 says `revalidation` is "the state of the binding's latest check", but a `required_before_transition` binding has no check before its transition, and an execution admitted with no start bindings has none. Here such a binding omits `revalidation`, and its M3 `state` is `satisfied` only if an evaluation at the read would be `current`. Basis: own judgment.
- **H7-HOLD-EXPIRY-INSPECT.** EVIDENCE §9 says expiry happens "when the provider clock reaches `expires_at`" and records an event, so it is a provider transaction. Here it runs before each request and on the idle re-check, like the staging timeout. Basis: spec text.

**Runs** (249 fixtures at `8500948`, runner rebuilt).

| Run | pass | fail | timeout | harness_error | unsupported | skipped |
|---|---|---|---|---|---|---|
| Sixth-pass code (`4a1d58a`) against `8500948`, from a scratch copy, before any change | 222 | 3 | 0 | 0 | 4 | 20 |
| After changes 1–8 (`6e9bebf`), first run, and one repeat | 225 | 0 | 0 | 0 | 4 | 20 |

Before the changes, the three failures were:
- `context.items-are-satisfied-only-by-their-check`, step 7: `result/items/0/result: expected "unmet", found "satisfied"` (change 1);
- `evidence.descriptor-provenance-coverage-and-locator-rules`, step 12: `details/path: expected "/payload/capture/uncertainty", found "/payload/capture/uncertainty/not_after"` (change 6);
- `evidence.expired-hold-stops-protecting`, step 9: `result/holds/0/state: expected "expired", found "released"` (change 3).

The H-RETRY-RUNNER failures are gone with the rebuilt runner. The sixth-pass code passes every other fixture at this base, so no fixture distinguishes changes 2, 4, 5, 7 and 8. Filtered runs after the changes: `--filter evidence.` 16 pass; `--filter context.` 10 pass; `--filter revalidation` 3 pass. The probes and the vector check pass as before.

**Fixtures read after the run.** The re-versioned and new fixtures (`git diff --stat 4a1d58a 8500948 -- conformance/fixtures`, 24 files) agree with the change list. Nothing was changed after reading them. What they settle and what they leave:
- `evidence.descriptor-provenance-coverage-and-locator-rules` v2 checks `unsupported_digest_algorithm` (`md5`, `details.algorithm`) and a short sha256 digest (`invalid_envelope` at `/payload/digest`) on fresh subjects, so the step of the algorithm refusal (H7-DIGEST-ALG-STEP) stays unobserved.
- `evidence.chunk-limit-accounts-for-encoding-overhead` v2 sends an oversize chunk with a stale precondition and expects `limit_exceeded`: step 2 is now checked.
- `evidence.integrity-failure-is-never-served` v2 pages with `limit: 2`, checks `next_cursor` present and then absent, and refuses a foreign cursor.
- `evidence.expired-hold-stops-protecting` and `evidence.purge-appends-the-artifact-events-first` match changes 3 and H-PURGE.
- `context.shared-job-survives-one-subscriber-cancelling` v2 checks that a request from another principal gets its own job.
- `context.request-items-are-checkable-and-obligations-negotiated` v2 accepts `/payload/items/0/transition` or `/payload/items/0`.
- `execution.revalidation-reports-match-mismatch-and-unavailable-by-obligation` v2 checks stale over unknown with two bindings.

**Still open or contradictory after the resolution.** None fails a fixture.

- **H7-SHA512-ADVERTISED** (above). Still open: EVIDENCE §3 ties sha512 content digests to advertising `core.digest-sha512`, and the descriptor's claims and the provider's `core.describe` can differ. No fixture uses a sha512 content digest. Suggested: say whether "advertises" means `core.describe` or the negotiated session. Basis: spec text.
- **H7-DIGEST-ALG-STEP** (above). Unobserved, as noted. Basis: own judgment.
- **H7-REVALIDATION-NO-CHECK** (above). Unobserved: the transition fixture inspects the binding only after its transition check. Suggested: EXECUTION §13.1 should say what `revalidation` shows before a binding's first check (omitted, or `unknown`). Basis: own judgment.
- **H7-COMPILER-NAME.** CONTEXT §12 fixes the compiler as `combraton-reference-context` for "the conformance provider". An independent provider running under the conformance configuration then reports itself as the reference compiler in `provenance.compiler`, which reads as a false provenance claim, although no fixture checks the name. Adopted as written. Suggested: make the convention "a compiler name the provider chooses", or name it by role. Basis: spec text (tension with CONTEXT §5 "compiler … identity").
- **H7-STAGED-INTEGRITY.** EVIDENCE §9 now says staged and sealed artifacts are `available` "while their stored bytes are intact". The `evidence_store.corrupt` control is described for stored bytes generally. Here it still applies only to sealed artifacts, so a staged artifact named in `corrupt` stays `available` and can be sealed if its bytes match. Whether a staged corrupt artifact should report `unavailable` and fail its seal is not stated. Unchecked. Basis: own judgment.
- **H7-PARTIAL-UNREACHABLE.** EVIDENCE §9 defines `partial` as "some sealed bytes are known lost", but no store control or operation produces that state. It is never reported here. Basis: spec text (coverage limit).
- **H7-EXPIRED-HOLD-IN-LOSS.** An expired hold on a purged artifact is listed in `affected` as `evidence.hold`, like a released one. §9 does not say whether expired holds count as dependencies. Unchecked. Basis: own judgment.
- **H7-PACKET-CITATION-TRACKED.** Change 7 tracks `context.packet_citation`, a *candidate* kind, because this provider holds the citing packets; entries are visible under `context.packet.read` on the packet. `evidence.purge-requires-release-authority-for-holds` v2 now uses `$contains`, so this passes, but no fixture purges an artifact a packet cites. Basis: spec text (candidate kind).
- **Owner point 4** (capacity while blocked at dispatch) remains open, and this implementation keeps the slot (H-REVAL-BLOCKS).

### H.8 (base 63f1bb0)

> Eighth spec-only pass, the owner's M4 close-out. The branch was fast-forwarded to `63f1bb0`. Read first: `git diff 6460e1d 63f1bb0 -- docs/spec schemas conformance/README.md conformance/schemas docs/work/release-0.1/M4.md`, and the two-line documentation change of the step-8 commit (`git diff 35f09a5 6460e1d -- docs/spec`). The readings below were written before any changed or new fixture was opened. Read rules unchanged.

**Readings of each change.**

- **H8-STEP8-COMPILER** (CONTEXT §12 at `6460e1d`). The compiler name is no longer a convention: "`compiler` names the implementation that compiled the packet". This resolves H7-COMPILER-NAME; the compiler goes back to `combraton-independent-python-core`. Basis: spec text.
- **H8-STEP8-DIGEST** (EVIDENCE §3 at `6460e1d`). `sha512` content digests are accepted when `core.describe` lists `core.digest-sha512`, which this provider's manifest does. `unsupported_digest_algorithm` and the wrong-length `invalid_envelope` are both decided at step 2. This resolves H7-SHA512-ADVERTISED and H7-DIGEST-ALG-STEP; the algorithm refusal moves from step 7 to step 2. Basis: spec text.
- **H8-CAPACITY-RELEASE** (EXECUTION §13.1 "Capacity while blocked before dispatch", §9). Reading:
  - When the dispatch gate (H-REVAL-BLOCKS) blocks an admitted execution for a `required_before_start` binding, the execution releases its capacity slot. It records `scheduling: { capacity: "released", reason: <block reason> }` and appends `execution.scheduling.changed` with the same payload, after the dispatch check that caused it. Delivery, effect, budget, lease, generation and epoch are untouched. A released execution does not count toward capacity.
  - `scheduling` appears in inspect, only in sessions with `execution.context_revalidation`, once it has first changed. Before any release there is nothing to report, and the text defines no initial value.
  - At each later evaluation of that execution's dispatch boundary, while released, the order is:
    1. The §7.1 checks first: cancellation requested, submitter no longer authorized, delivery or execution deadline passed. If one applies, the delivery becomes `failed_before_delivery`, and the execution ends. Its events: `execution.scheduling.changed { capacity: "released", reason: <cancelled|authorization_lost|deadline_passed> }`, then, for a deadline, `core.effect.obligation.overdue` for each open obligation, then `execution.delivery.observed`. The delivery evidence class is `never_dispatched`, the effect's §15.1 class for provable non-dispatch, with the reason as source. Other reasons satisfy the obligations. No `recovery` entry is added: this is not a restart.
    2. Otherwise the context bindings are revalidated at dispatch (checks recorded as before). While still blocked, the execution stays released; a changed block reason updates `scheduling.reason` with an event.
    3. Once current, capacity is reacquired: if running executions already fill it, `scheduling.reason` becomes `capacity` (event once); otherwise `scheduling: { capacity: "held" }` with an event, and the dispatch step runs exactly once under the existing delivery identity, through the marker and generation fencing as before.
  - A delivery timeout passing while released is handled by the ordinary §8 timeout, which ends the wait the same way (`failed_before_delivery`, obligations overdue). The §13.1 path is reached only if the script's dispatch step is evaluated first. Basis: spec text, own judgment for event order, evidence class and inspect presence.
- **H8-PINNING** (EXECUTION §13.1, CONTEXT §8). Binding member `require_current` (boolean), accepted only under the feature and echoed in inspect. The implicit results `packet.facts` (and `packet.current` under `require_current: true`) exist only "when the binding names a context fetch grant". This executor reaches no peers, so with `fetch.context` both are `unavailable`, with evidence `context_provider_unreachable`, and without `fetch.context` neither appears, whatever `require_current` says. `packet.current` is no longer reported for a binding without `require_current`. Basis: spec text (the scope of the implicit conditions).
- **H8-INVALIDATED-ITEMS** (CONTEXT §8 "Corrections after publication"). `context.packet.inspect` always carries `invalidated_items` (required by the schema). It lists one entry per authority entry of the revision whose item's current authority revision, after later `correction` steps of the job, is greater than the one the revision was prepared against. `authority_revision` in the entry is read as the corrected revision that invalidated it; the text's "for authority revisions corrected after the revision was prepared" could also mean the old revision. `superseded_by` is `{ revision: n + 1 }` when a later revision exists, mirroring `supersedes`; the text could also mean the latest revision. Neither changes the stored facts. Basis: own judgment for both member meanings.
- **H8-CONSTRAINTS** (CORE §15.3, EVIDENCE §10, core common schema). `core.grant.issue` accepts `constraints` (at most 8, unique), each `{ kind, work? }`:
  - At step 2, a kind this provider does not implement (anything but `evidence.work_binding`) is `invalid_envelope` at `/payload/constraints/<i>/kind`, and a `evidence.work_binding` without `work` is `invalid_envelope` at that item.
  - At step 3, `evidence.work_binding` when the issuing session did not negotiate the feature is `unsupported_required_feature` with `features: ["evidence.work_binding"]`.
  - Delegation: the child must contain each parent constraint, unchanged; otherwise `delegation_exceeded`. Adding constraints is allowed, since a constraint only narrows.
  - The record and `core.grant.issued` carry the constraints.
  Basis: spec text; own judgment for "child may add" and for `work` on other kinds (refused along with the unknown kind).
- **H8-WORK-BINDING** (EVIDENCE §10, feature `evidence.work_binding`). Replaces H-BOUND-WORK. Resource kinds imply nothing. For every `evidence.work_binding` constraint of the grant a prepare acts under, the descriptor's `work` must equal the constraint's `work`; otherwise `binding_violation` after `right_missing` and `out_of_scope`. Enforced whether or not the preparing session negotiated the feature: the grant carries it. Basis: spec text.
- **H8-ABANDON** (EVIDENCE §4 table). Abandon is `not_found` unless the artifact is staged, and a new abandon on an abandoned artifact is `not_found`. Already so. Basis: spec text.
- **H8-FILTERED** (EVIDENCE §5). Unchanged; the implementation already computes `filtered` from authorization alone. Basis: spec text.
- **H8-BASIS-CHANGES** (README `executor.basis_changes`, step `observe_host_basis`). Reading: the host's observed basis starts from launch `observed_basis`. Each `basis_changes` entry whose `at` is at or before the provider clock applies, in `at` order. Each `observe_host_basis` step applies at the instant it ran, persisted in the store, so it survives restarts. Every execution's `observe_basis` steps merge over the result. A `basis_changes` entry *replaces* the host basis ("what the executor observes"); `observe_host_basis` *merges* per repository and member, like `observe_basis`. Entries of both kinds are applied in time order, a `basis_changes` entry first at equal instants. The README says neither replace nor merge. Basis: own judgment.
- **H8-ALTERED-BYTES** (EVIDENCE §14, README `serve_altered_bytes`). For listed artifacts, `evidence.fetch` of available content returns every byte XORed with `0x01`, with the sealed digest, `size` and `available` unchanged. Inspect, query and the store's own integrity check see the true bytes. It applies to fetch only, not to packet excerpts or `context.expand`. Basis: spec text, own judgment for the alteration.

**Implementation.** Committed as `76db7a0` ("eighth pass implementation from the documents") before any changed or new fixture was opened, with the participant descriptor now claiming feature `evidence.work_binding`. One change followed the fixtures (H8-RESUMED-REASON, below).

**Runs** (255 fixtures at `63f1bb0`).

| Run | pass | fail | timeout | harness_error | unsupported | skipped |
|---|---|---|---|---|---|---|
| Seventh-pass code at this base, seventh-pass claims | 217 | 10 | 0 | 0 | 5 | 23 |
| First run after the implementation (`76db7a0`) | 227 | 1 | 0 | 0 | 4 | 23 |
| After H8-RESUMED-REASON, and one repeat | 228 | 0 | 0 | 0 | 4 | 23 |

- Before the implementation, the ten failures were seven `context.*` fixtures, each on `context.packet.inspect` result schema validation (`"invalidated_items" is a required property`), and the three new execution fixtures (`participant closed the connection` at step 1: launch keys `basis_changes` refused). `evidence.publish-grant-bound-to-work` v2 was unsupported for want of the `evidence.work_binding` claim.
- The one failure after the implementation was `execution.blocked-dispatch-releases-capacity-and-resumes`, step 11: `result/items: no element matches {"event":{"payload":{"capacity":"held","reason":"resumed"},…,"type":"execution.scheduling.changed"}}`.
- Filtered final runs: `--filter evidence.` 16 pass; `--filter context.` 10 pass; `--filter revalidation` 3 pass; `--filter blocked-dispatch`, `--filter released-work` and `--filter transition-block` 1 pass each.
- The 23 skipped are 12 `socket.*` and 11 `composition.*`. The four unsupported are backpressure.

**Change after reading the fixtures.**

- **H8-RESUMED-REASON.** EXECUTION §9 gives `execution.scheduling.changed` the payload `{ capacity: "held" | "released", reason }`, reason unconditional. §13.1 names no reason for reacquiring capacity, and the `scheduling.reason` enum of `schemas/execution/1/common.schema.json` has no value for it (`context_binding_*`, `capacity`, `cancelled`, `authorization_lost`, `deadline_passed`). The first implementation emitted `{ capacity: "held" }`. `execution.blocked-dispatch-releases-capacity-and-resumes` step 11 expects `{ capacity: "held", reason: "resumed" }` in the event, while step 8 expects inspect `scheduling: { capacity: "held" }`. The event now carries `reason: "resumed"`, and the inspect member omits the reason when held, which keeps it schema-valid. The event reason is outside the inspect member's enum. Suggested: add `resumed` to §13.1 and to the schema enum, or say the held event has no reason. Basis: fixture-informed (contradicts the schema enum; the §9 payload implies a reason).

**Fixtures read after the run: what they settle and what they leave.**

- `execution.transition-block-clears-when-current` expects `scheduling` absent for work never released, which confirms H8-CAPACITY-RELEASE on inspect presence. It also covers the transition block clearing, previously in H.5.
- `execution.released-work-ends-instead-of-resuming` inspects only after every binding is current again (T40). So it does not distinguish ending at the first evaluation after the cause (here T30 for the deadline, T20 or later for cancellation and revocation) from ending only when resumption would otherwise happen. It checks the effect `failed` with no attempts, but not the delivery evidence class (`never_dispatched` here) or the overdue/satisfied obligations.
- `evidence.publish-grant-bound-to-work` v2 matches H8-CONSTRAINTS and H8-WORK-BINDING: feature not negotiated, unknown kind path, the combined grant with unrelated resources, missing parent constraint on delegation. A child adding constraints is not exercised.
- `evidence.access-reveals-no-existence` v2 (paired query), `evidence.hold-release-needs-ownership-or-delegated-authority` v2 (abandon of a held, sealed artifact is `not_found`) and `evidence.seal-refuses-digest-mismatch-and-is-idempotent` v3 (abandoned availability, abandon of a sealed artifact) pass as implemented.
- The skipped composition fixtures were read for the points they settle for other participants: `invalidated_items` carries the corrected revision (`authority_revision: 2` after a correction to 2), as H8-INVALIDATED-ITEMS chose. `superseded_by` is `{ revision: 2 }` with only two revisions, so "next" and "latest" stay indistinguishable. `observe_host_basis`, `serve_altered_bytes` and `require_current` appear only in composition fixtures.

**Still open or contradictory.** None fails a fixture.

- **H8-PACKET-FACTS-OBSERVED.** The composition fixtures expect `packet.facts` mismatches to carry `observed: "corrected: <item_id>"`. EXECUTION §13.1 gives the result's `observed` no format for implicit conditions. This executor never produces `match` or `mismatch` for `packet.facts` (no peers), so it is untested here. Basis: fixture expectation not in the documents (composition only).
- **H8-REQUIRE-CURRENT-WITHOUT-FETCH.** `require_current: true` on a binding without `fetch.context`: the implicit conditions exist only "when the binding names a context fetch grant", so the requirement has no effect, and a pinned-versus-current distinction is invisible for packets held through launch configuration. Kept as written. Suggested: say whether `require_current` without a context fetch grant is refused, ignored, or makes `packet.current` `unavailable`. Unchecked. Basis: spec text (gap).
- **H8-END-TIMING.** EXECUTION §13.1 "At its dispatch boundary the executor first applies the §7.1 revalidation" does not say how often a released execution's boundary is evaluated. Here it is evaluated on every tick, so released work ends as soon as a cause appears, even while its binding is still stale. Unguarded, as noted above. Basis: own judgment.
- **H8-RELEASED-TIMEOUT-OVERLAP.** A `delivery` timeout passing while released ends the wait through §8 (`failed_before_delivery`, evidence `delivery_timeout_before_dispatch`, `execution.timeout.passed`). An `execution_deadline` ends it through §13.1 (`deadline_passed`, evidence `never_dispatched`). Both give `failed_before_delivery`, but the scheduling reason and evidence differ by which timeout passed. The fixture uses `execution_deadline` only. Basis: own judgment.
- **H8-BASIS-CHANGES-MERGE.** Replace versus merge for `basis_changes` (H8-BASIS-CHANGES) is not distinguished by the single-repository fixtures. Basis: own judgment.
- **H8-CONSTRAINT-ADDITION.** CORE §15.3 says a delegated grant "carries every constraint of its parent, unchanged, otherwise `delegation_exceeded`", but not whether it may add constraints. Allowed here, since constraints only narrow. Unchecked. Basis: own judgment.
- **H8-CAPACITY-FAIRNESS.** M4 names fairness among released executions waiting for capacity as unspecified. Here queued admissions are evaluated before scripts in each tick, so work queued at admission takes a freed slot before released work resumes. Unchecked. Basis: own judgment (limit acknowledged in M4).

### H.9 (base 9591551)

> Ninth spec-only pass, a short realignment with the resolution of H.8 (M4-DIVERGENCES §C.2). Read first: `git diff dc2f8c8 9591551 -- docs/spec conformance/README.md docs/work/release-0.1/M4-DIVERGENCES.md`. The one changed fixture, `execution.revalidation-reports-match-mismatch-and-unavailable-by-obligation` v3, was read after implementing and running. Read rules unchanged.

**Changes, from the documents.**

| Resolved text | Change |
|---|---|
| EXECUTION §13.1: `require_current: true` needs `fetch.context`, otherwise `invalid_envelope` at `/payload/context_bindings/<i>/require_current` | Refused at step 2 (resolves H8-REQUIRE-CURRENT-WITHOUT-FETCH). `require_current: false` without a fetch grant is accepted. |
| Conformance README: `basis_changes` merges per member and per repository, like `observe_host_basis`, in time order | `basis_changes` now merges (resolves H8-BASIS-CHANGES-MERGE) |
| CONTEXT §8: `superseded_by` names the request's current revision | The latest revision instead of the next one |
| EXECUTION §13.1: a passed delivery timeout ends released work with `scheduling.reason: "deadline_passed"` | The §8 delivery-timeout path now also records `scheduling: { capacity: "released", reason: "deadline_passed" }` and its event for released work, and clears `context.blocked` (resolves H8-RELEASED-TIMEOUT-OVERLAP) |
| EXECUTION §13.1: `observed` strings for `packet.facts` and `packet.current` mismatches | No change: this executor reaches no context provider, so those results are always `unavailable`, with no `observed` |
| EXECUTION §9 `resumed`; CORE §15.3 added constraints; §13.1 end timing and fairness | Already as implemented (H8-RESUMED-REASON, H8-CONSTRAINT-ADDITION, H8-END-TIMING, H8-CAPACITY-FAIRNESS) |

**Run** (255 fixtures): **228 pass, 0 fail, 0 timeout, 0 harness_error, 4 unsupported, 23 skipped**, on the first run after the changes. The v3 fixture adds one step, a `require_current: true` binding without fetch grants expecting `invalid_envelope` at `/payload/context_bindings/0/require_current`, which matches. Nothing was changed after reading it.

**Still open.** None fails a fixture.

- **H9-TIMEOUT-SCHEDULING-ORDER.** When a delivery timeout ends released work, this implementation appends `execution.timeout.passed`, the overdue markings, `execution.delivery.observed`, then `execution.scheduling.changed { released, deadline_passed }`. On the §13.1 path (execution deadline, cancellation, revoked authorization) the scheduling change comes first, before the determination. §9's causal order puts the cause first, but the text does not say whether the scheduling change is caused by the timeout or causes the determination. The two paths therefore order the same events differently. Unchecked. Suggested: fix one order for both. Basis: own judgment.
- **H9-OBSERVED-UNTESTED.** The `observed` strings for `packet.facts` and `packet.current` are exercised only by composition fixtures, so this stdio participant never produces or checks them (coverage limit). Basis: spec text.

### H.10 (base 0e80475)

> Tenth spec-only pass. Read first: `git diff 36ca076 0e80475 -- docs/spec conformance/README.md docs/work/release-0.1/M4-DIVERGENCES.md` (EXECUTION §13.1 and §15.1; M4-DIVERGENCES §C.3). The new fixture `execution.released-work-ending-emits-cause-first` was read only after implementing and running. Read rules unchanged.

**Changes, from the documents** (`941b60b`), resolving H9-TIMEOUT-SCHEDULING-ORDER:
- **§15.1 "Event order".** Released work ending before dispatch now emits `execution.scheduling.changed` last on every path. On the §13.1 path (cancellation, lost authorization, execution deadline), the order is the overdue markings for a deadline, then `execution.delivery.observed`, then the scheduling change; before, the scheduling change came first. The delivery-timeout path already ended with it.
- **§15.1 evidence-class row.** On the §13.1 path the delivery record's evidence class is `scheduling` and the effect's `never_dispatched` (before: `never_dispatched` on both). The delivery-timeout path keeps `delivery_timeout_before_dispatch` on both.
- **"`execution.timeout.passed` for each timeout that passed".** When the delivery timeout ends released work and the execution deadline is due at the same evaluation, both `execution.timeout.passed` events now precede the overdue markings.

**Run** (256 fixtures), first run after the changes: **229 pass, 0 fail, 0 timeout, 0 harness_error, 4 unsupported, 23 skipped** (`run: 256 fixtures (229 pass, 23 skipped, 4 unsupported), 0 not passing`). The new fixture passes. It checks the delivery-timeout order and evidence, and the cancellation order starting with `execution.cancel.requested` then the delivery observation with class `scheduling`. Nothing was changed after reading it.

**Still open.** None fails a fixture.
- **H10-DEADLINE-SEPARATE-UNITS.** An execution deadline passes in the executor's timeout unit, and the ending (overdue, delivery observation, scheduling change) follows in the script unit of the same tick, a separate provider transaction. The stream order is as §15.1 states, but another execution's events could fall between them. §15.1 orders events "with the same cause", not their contiguity. Unchecked. Basis: own judgment.
- **H10-INACTIVITY-AT-ENDING.** "`execution.timeout.passed` for each timeout that passed": an `inactivity` timeout due at the same evaluation as an ending delivery timeout is emitted after the ending here, since inactivity does not end the wait. Whether it counts as part of the cause is not stated. Unchecked (M4-DIVERGENCES §D lists both deadline kinds at one evaluation as a coverage limit). Basis: own judgment.

### H.M5 (base 950f3e0)

> Eleventh spec-only pass: `knowledge/1` (all 11 operations, test control `knowledge.store`), `verification/1` (features `verification.jobs` and `verification.record`, test control `verifier.script`) and `context.claims` for the single-provider case. Branch `release-0.1/m5-independent` from `950f3e0`. Read: KNOWLEDGE and VERIFICATION in full; CONTEXT §14; EXECUTION §13.3; CORE §7, §8, §10, §12, §15, §16, §17; M5 and the M5 rows of MATRIX; `schemas/knowledge/1`, `schemas/verification/1` and the changed Context and Core schemas; the diff of `docs/spec`, `schemas`, the conformance README and the fixture and launch-configuration schemas between `0e80475` and `950f3e0`; this directory. Not read: `conformance/reference/**`, `conformance/runner/src/**`, `conformance/crosscheck/**`, or their history or diffs. The runner was built and used as a binary.

**Reading order.** The documents; an implementation from them, committed as `97b5471` before any M5 fixture was opened; the first complete run; then only the fixtures that failed (`knowledge.claim-revisions-are-immutable-with-base-checks`, `verification.assessment-never-passes-missing-unavailable-or-outdated`, `verification.evaluate-contract-returns-a-job-and-pins-its-evaluator`, `verification.receipts-list-every-role-and-property`, `verification.reconnect-with-changed-evaluator-keeps-pinned-meaning`), each in full. No passing fixture was read, including `context.claims-in-packets-are-negotiated-snapshots`, `composition.claims-in-packets-block-only-the-required-boundary` and the other six `knowledge.*` fixtures.

**Baseline** at `950f3e0` with the tenth-pass code and claims: `run: 271 fixtures (229 pass, 24 skipped, 18 unsupported), 0 not passing`. The 14 M5 fixtures (8 `knowledge.*`, 5 `verification.*`, `context.claims-in-packets-are-negotiated-snapshots`) were unsupported, besides the 4 backpressure fixtures.

#### Readings before the fixtures

Choices made from the documents where they are silent or ambiguous, all in `97b5471`. Basis: own judgment unless stated. The fixture-sensitivity column comes from the probe described below: "unguarded" means a deliberate deviation passed every M5 fixture.

| Tag | Question and choice |
|---|---|
| HM5-CORE-PRECONDITION-ORDER | KNOWLEDGE §6 and §7 and VERIFICATION §4 give step 7 orders that do not place the Core preconditions of CORE §10 step 7. Here they follow the authority-epoch check (`decision.record`, `conflict.resolve`), follow the evaluator capability (`evaluate_contract`), and come first elsewhere, before the profile's own `not_found`, digest and contract checks. |
| HM5-NAMED-REVISION-RIGHTS | KNOWLEDGE §10: "Commands that name revisions also need `knowledge.read` on those claims." Applied to the claim of every revision reference at this provider that a command names: `decision.record` (`claim`), `conflict.open` (both), `applicability.evaluate` (`claim`), `conflict.resolve` (`selected`), and for `propose`/`revise` the `dependencies` and claim-kind support roots; `revise` also needs `knowledge.read` on its own claim because `supersedes` names a revision. References to other providers cannot be covered by a local grant and add nothing. Unguarded. |
| HM5-BIND-GRANT | KNOWLEDGE §6 "a grant never lets anyone else do so": a non-authority is `not_authority` whatever grant it names; an authority principal that names a grant is also `not_authority` (CORE §15.5 restricts it to that grant, and no right covers binding). |
| HM5-RESOLVE-EPOCH | KNOWLEDGE §7 says resolve is recorded "with `authority_epoch`", but only §6 says its absence is `invalid_envelope` at step 2. Applied to resolve too (CORE §8 default). |
| HM5-UNKNOWN-ROOTS-PATH | Non-empty roots with `unknown` completeness: `invalid_envelope` at `/payload/support/<i>/ancestry/roots`. Confirmed by the failing fixture read afterwards (step 13), which passed. |
| HM5-SINGLE-ENTRY-OVERLAP | KNOWLEDGE §5 rule 5 "every pair of entries is `shared`, but no root is common to all" is vacuously true for one entry with no roots. Rule 5 requires at least one pair here, so a single complete entry without roots is `undetermined`. Unguarded. |
| HM5-AVAILABILITY | KNOWLEDGE §5 availability counts one per support entry. At this provider: sealed, available and digest-matching counts `available`; `purged` counts `purged`; a missing, unsealed, digest-mismatched, `purge_pending`, corrupt or unavailable artifact counts `unavailable`; other providers count `unknown`. With no entries, "`complete` when all are available" holds vacuously. Unguarded (zero entries). |
| HM5-FACET-ORDER | `applicability` lists one entry per distinct target in order of the target's first evaluation; `conflicts` lists open and resolved records in recorded order. The context snapshot lists only open conflicts ("Open conflicts involving a carried claim are listed", CONTEXT §14). Snapshot with resolved conflicts: unguarded. |
| HM5-SELECT-EXACT | `selected` names one of the two revisions when provider, claim, revision and digest all match; `selected` with any other resolution is `invalid_envelope` at `/payload/selected` ("required exactly for `select`"). |
| HM5-LATEST-DECISION-NULL | A `supersedes_decision` given when no decision exists is `precondition_failed` with `{ latest_decision: null }`. |
| HM5-REASON-STEP | VERIFICATION §5: a missing or extra `reason` is decided at step 2 (it needs no contract); the role and property listing after the contract is read. |
| HM5-OBSERVED-ORDER | "`observed_from` ≤ `observed_until`" names no error. A recorded receipt violating it is `invalid_envelope` at `/payload/observed_until`. Unguarded. |
| HM5-CONTRACT-FORMAT | VERIFICATION §3 names format `combraton-verification-contract/1` but its member table has no `format` member. Here `format` is optional and, if present, must be that value; the media type is not checked (the fixture read afterwards seals contracts as `text/plain`); canonical byte form is not required. Anything else unparseable, a duplicate role or property ID, or an unknown layer is `invalid_format`. |
| HM5-CONTRACT-READ-ORDER | Contract readability: other provider `unreachable`; absent `not_found`; staged or abandoned `not_sealed`; purged, pending purge, corrupt or unavailable `unavailable`; then digest mismatch `artifact_digest_mismatch`; then content `invalid_format`. |
| HM5-JOB-STATES | A job is `queued` in its submit transaction and `running` at the first evaluation after it, with `verification.job.changed { state, evaluator }` for each state. Receipt issue appends the artifact's staged, appended and sealed events, `verification.receipt.issued { reference, job }` with `job` as the job subject, then `verification.job.changed { completed }`. `job` as identifier: unguarded. |
| HM5-OBSERVED-ENVIRONMENT | VERIFICATION §5 distinguishes the requested environment (§4) from the receipt's "as observed". An issued receipt's anchors are only those the script's `observe` steps report, never the requested ones. Ignoring `observe`: unguarded. |
| HM5-SCRIPT-END | A script that runs out without `complete` leaves the job `running`, as context and executor scripts do; only an absent script completes at once. Completing at the end: unguarded. |
| HM5-SCRIPT-REASON | A scripted `not_evaluated` or `indeterminate` property without `reason` records its result as the reason; a reason on `pass` or `fail` is dropped; a property the contract does not name, or one already recorded, is ignored. |
| HM5-VERIFIER-PRINCIPAL | "For an issued receipt, `principal` is the verifier's": the provider's `provider_id`, as for packet producers (CONTEXT §5). The fixture read afterwards expects `conformance-provider`. Issued receipts have `inputs: []`, `provenance: []`, `scope: "verification job <id>"`, `observed_from` = the instant the job started running and `observed_until` = completion; the documents define none of these for issued receipts. |
| HM5-RECEIPT-EVENTS-ORDER | `receipt.record` commits `verification.receipt.recorded` first (CORE §16.3 primary subject), then the artifact's staged, appended and sealed events, all command-origin. Recorded event last: unguarded. |
| HM5-RECEIPT-INSPECT-FEATURE | VERIFICATION §1 lists `verification.record` as "(§5)", and §5 also defines `receipt.inspect`. `receipt.inspect` and `receipt.assess` are base-profile here, so a session with only `verification.jobs` can read issued receipts; `job.inspect` needs `verification.jobs`. |
| HM5-RECEIPT-UNREADABLE | `receipt.inspect` whose sealed bytes cannot be read or fail integrity is `unavailable` (no regenerated content). |
| HM5-RECEIPT-ID-TAKEN | A job whose ID is already a recorded receipt's cannot issue its receipt and stays `running`. |
| HM5-ASSESS-UNREADABLE-RECEIPT | When the receipt's content cannot be used, the checks that need it (contract mismatch, subjects, environment, evaluator, time) are `unverifiable` with the receipt check's reason, although the §7 table gives Subjects, Evaluator and Time no unverifiable column. With the contract unreadable, environment and evaluator are `unverifiable` with `contract_unavailable`. Reporting them `passed` instead: unguarded. |
| HM5-OVERALL-WITH-FAILED-CHECKS | VERIFICATION §7 "Overall" yields `satisfied` when no property is `required`, even if every check failed. Here any failed or unverifiable check caps `overall` at `not_satisfied`. Unguarded. |
| HM5-TIME-BOUNDS | `stale` when `assessed_at ≥ valid_until` or `assessed_at > observed_until + max_age_seconds`, as written. Both boundaries flipped: unguarded. |
| HM5-PACKET-BASIS-TARGET | See defects below. The packet's basis is converted to a Knowledge target (repository `id`, `tree`, non-null `dirty`; `environment`, `build`, `completeness`; `workspace` and `configuration` dropped) before looking up the evaluation. Without the conversion `context.claims-in-packets-are-negotiated-snapshots` fails (step 26), so the fixture relies on some conversion the documents do not state. |
| HM5-LABEL-DEMOTION | A section labeled `binding` whose claim is not `accepted_for_use` with `permitted_use: binding` (including accepted for a lesser use, or `superseded`) is published as `hypothesis`; `stale` takes precedence when historical. |
| HM5-CLAIM-SECTION-ITEM | A `claim_included` item is satisfied only by a section with its `item_id` that carries exactly its reference, as the M4 checks require the item's own section. The table decides, including for a `historical` (invalid-for-target) section under a `hypothesis` or `reference` item, which the table satisfies because it asks only "reliance other than `rejected`". Requiring applicability for those items: unguarded. |
| HM5-CLAIM-READ-TIME | Claims are read, and digests recomputed, when the packet revision is published (the section step only records the reference). An unreadable or altered claim's section is omitted (`unavailable`) and its item gets `knowledge_unavailable` or `claim_digest_mismatch`, ahead of scripted `unmet` reasons. |
| HM5-CLAIM-CHANGES | One `claim_changes` entry per change kind per carried section, in the §14 list order; `applicability_changed` when the result or the evaluation ID differs (a re-evaluation with the same result counts). Result-only comparison: unguarded. |
| HM5-UNVERIFIED-BY-TABLE | `applicability_not_established` in `unverified_items` only for `binding` or `evidence` items, whose satisfaction needed applicability; §14 does not restrict it by reliance. Omitting it altogether: unguarded. |
| HM5-CLAIMS-FEATURE-ORDER | `unsupported_required_feature` lists obligation features and `context.claims` in item order. |
| HM5-DUPLICATE-IDS | Duplicate `support_id` or `condition_id` values within a revision are accepted; nothing forbids them, though `unknown_ancestry` and findings then name one ID twice. |
| HM5-AUTHORITY-SCOPES | Knowledge authority scopes are not CORE §15.3 tracked scopes for a grant's `authority_binding`. |

#### First complete run

`run: 271 fixtures (5 fail, 238 pass, 24 skipped, 4 unsupported), 5 not passing`

- `knowledge.claim-revisions-are-immutable-with-base-checks` — step 14: expected error `invalid_envelope`, received success.
- `verification.assessment-never-passes-missing-unavailable-or-outdated` — step 30: `result/properties`: expected 0 items, found 2.
- `verification.evaluate-contract-returns-a-job-and-pins-its-evaluator` — step 9: `result/recorded`: expected 2 items, found 3.
- `verification.receipts-list-every-role-and-property` — step 12: `details/duplicated`: missing.
- `verification.reconnect-with-changed-evaluator-keeps-pinned-meaning` — step 16: `result/recorded`: expected 1 items, found 2.

Nine of the fourteen M5 fixtures passed on first contact: seven `knowledge.*`, `verification.results-are-scoped-and-grant-no-authority` and `context.claims-in-packets-are-negotiated-snapshots`.

#### Changes after reading the fixtures

Six changes, all in `9de18c9`, each fixture-driven.

- **HM5-VALIDITY-ORDER.** Question: is a `validity` with `from` after `until` refused? KNOWLEDGE §3 only says "The half-open interval `[from, until)`"; neither it nor the schema refuses anything. The fixture (step 14) expects `invalid_envelope` at `/payload/validity`. Done: `from > until` is refused there at step 2; `from == until` (an empty interval) is still accepted, which no fixture checks. Basis: fixture expectation not in the documents.
- **HM5-RECORDED-FILL.** Question: are the results a completion fills in (`not_evaluated`/`not_reported`, `indeterminate`/`evaluator_unavailable`) "recorded" on the job? VERIFICATION §4 says "A verifier records each property result on the job as it is evaluated" and "every property without a recorded result is `indeterminate`", without saying whether that result is then recorded. The first implementation recorded them, with `verification.property.recorded` events. The fixtures (evaluate step 9, reconnect step 16, and evaluate step 15 `recorded: []` for an absent script) expect `recorded` to keep only the evaluator's own results; the filled results appear only in the receipt. Done: no longer recorded, no events. Basis: fixture expectation not in the documents.
- **HM5-LISTING-DETAILS.** Question: the shape of "details `missing`, `duplicated` or `unknown`" (VERIFICATION §4, §5). The first implementation gave those members as lists of names, only when non-empty. The fixture (step 12) expects all three lists, empty ones included: `{ path, missing: ["docs.review"], duplicated: [], unknown: [] }`. Done: all three always present. Basis: fixture expectation not in the documents (the member names matched the reading).
- **HM5-ASSESS-REFERENCE-MISMATCH.** Question: with `receipt_reference_mismatch`, are properties presented from the receipt's mapped bytes? VERIFICATION §7 empties `properties` only "When the contract or the receipt's bytes cannot be read". The fixture (step 30) expects `properties: []` for a reference naming the receipt's artifact with another receipt's digest. Done: a mismatched reference is treated as naming bytes that cannot be read; the receipt check keeps only `receipt_reference_mismatch`. Basis: fixture expectation not in the documents (defensible reading: the referenced bytes do not exist).
- **HM5-UNLISTED-EVALUATOR.** Question: what status does `capability_unavailable` report for an evaluator version the verifier does not list? The first implementation said `unknown`, as CORE §17.1 "A predicate without evidence of support is `unknown`" and this executor's adapter predicates do. Changed after reading `verification.evaluate-contract-returns-a-job-and-pins-its-evaluator` step 12 (`journey` `v7` never configured) and the reconnect fixture step 18 (`v1` removed at restart), which expect `status: "unsupported"`; neither step had been reached, so this was changed before a run showed it. Done: an unlisted `verification.evaluator.*` predicate is `unsupported` at `evaluate_contract` (the snapshot still lists only configured evaluators). Basis: fixture expectation not in the documents, arguably against CORE §17.1 (see defects).
- **HM5-ASSESS-MISSING.** Question: a nonexistent receipt at assessment, for a reader who is authorized. VERIFICATION §7: "A receipt the reader may not read is `permission_denied`, identical to a nonexistent receipt (CORE-12)", which the first implementation followed literally (`permission_denied`, `out_of_scope`, for everyone). The fixture (step 36, found on the second run) expects `not_found` for the authority principal. Done: after step 6, a nonexistent receipt or one at another provider is `not_found`; unauthorized readers are refused at step 6 identically for both. Basis: fixture expectation against the literal text (see defects).

#### Fixture sensitivity

A throwaway probe (not committed) copied this implementation, applied one deviation at a time, and ran the `knowledge.`, `verification.` and `context.claims` fixtures. Noticed: equal-digest roots treated as disjoint (support classes step 17); `demonstrated` ignoring bases (conflicts step 4); null history positions (history step 12); no basis-to-target conversion (claims step 26); a `binding` label not demoted, or demoted to `inferred` (claims step 27); no `lineage_revised` (claims step 27); an invalid-for-target section not historical (claims step 27); `claim` members shown without `context.claims` (claims step 33). Unguarded (every M5 fixture still passed): HM5-SINGLE-ENTRY-OVERLAP, zero-entry availability, HM5-NAMED-REVISION-RIGHTS removed, resolved conflicts in snapshots, HM5-RECEIPT-EVENTS-ORDER, HM5-ASSESS-UNREADABLE-RECEIPT, both HM5-TIME-BOUNDS boundaries, HM5-OVERALL-WITH-FAILED-CHECKS, HM5-SCRIPT-END, ignoring `observe` (HM5-OBSERVED-ENVIRONMENT), the `receipt.issued` job member shape, applicability required for hypothesis items, result-only `applicability_changed`, **format `/2` for requests submitted without `context.claims`** (CMP-9 names `/1` for them, but no M5 fixture observed the difference), and no `applicability_not_established` in `unverified_items`.

#### Defects and gaps in the documents and fixtures

- **HM5-DEFECT-ASSESS-EXISTENCE.** VERIFICATION §7: "A receipt the reader may not read is `permission_denied`, identical to a nonexistent receipt (CORE-12)." Read literally, a nonexistent receipt is `permission_denied` for every reader, and the fixture expects `not_found` for an authority principal. CORE §15.5 intends "the same `permission_denied` for an existing and a nonexistent subject" for unauthorized principals only. Suggested wording, as §3 already has for contracts: "identical for an existing and a nonexistent receipt; an authorized reader gets `not_found`."
- **HM5-DEFECT-PACKET-BASIS-TARGET.** CONTEXT §14: "`applicability` is the latest evaluation of that revision for a target equal to the packet's basis", and KNOWLEDGE §8 compares targets "(canonical JSON)". A packet basis (CONTEXT §3) has `workspace`, `dirty: … | null` and `configuration`; a Knowledge target has no `workspace` or `configuration` and an optional, non-null `dirty`. No packet basis can equal a target in canonical JSON, and the fixture passes only with a conversion (sensitivity above). The conversion used here (drop `workspace` and `configuration`, drop null `dirty`) is not written anywhere; a basis with `configuration`, or a `dirty` workspace whose snapshot is null, is exactly where implementations would diverge.
- **HM5-DEFECT-UNLISTED-PREDICATE.** CORE §17.1: "A predicate without evidence of support is `unknown`, never `supported`." The fixtures expect an evaluator version the verifier never listed, or removed at restart, to be refused as `unsupported`. VERIFICATION §4 or §11 should say that a verifier reports every evaluator version it does not have as `unsupported` (it knows its installed set), or the fixtures should accept `unknown`.
- **HM5-DEFECT-RECORDED.** VERIFICATION §4 does not say that completion-filled results stay out of `recorded`; the fixtures require it (HM5-RECORDED-FILL). Suggested: "`recorded` lists results the evaluator reported; results assigned at completion appear only in the receipt."
- **HM5-DEFECT-OVERALL.** VERIFICATION §7 "Overall" never consults the checks. With no required property, a receipt whose every check failed is `overall: satisfied`, and `present_validity` can then be `established`, against VER-4 ("never becomes a pass"). Implemented with a cap; unguarded.
- **HM5-DEFECT-AUTHORITY-GET.** KNOWLEDGE §6 gives `knowledge.authority.get` → `{ scope, authority, epoch }`; `knowledge.authority.get.result.schema.json` also requires `revision`. Implemented per the schema (equal to the epoch).
- **HM5-GAP-VALIDITY.** KNOWLEDGE §3 states no refusal for `from` after `until`; the fixture refuses it (HM5-VALIDITY-ORDER). `from == until` is unspecified.
- **HM5-GAP-LISTING-DETAILS.** VERIFICATION §4 and §5 name `missing`, `duplicated` and `unknown` without their type or whether empty ones appear.
- **HM5-GAP-CONTRACT-FORMAT.** VERIFICATION §3's member table has no `format` member and no media type rule for readers (HM5-CONTRACT-FORMAT).
- **HM5-GAP-ISSUED-RECEIPT.** For issued receipts the documents do not define `scope`, `inputs`, `provenance`, `observed_from`, the observed environment, or when a job becomes `running` (HM5-VERIFIER-PRINCIPAL, HM5-JOB-STATES, HM5-OBSERVED-ENVIRONMENT).
- **HM5-GAP-STEP7.** KNOWLEDGE §6/§7 and VERIFICATION §4 list step 7 checks without the Core preconditions (HM5-CORE-PRECONDITION-ORDER). No fixture read here separates the placements.
- **HM5-GAP-SUPPORT-VACUOUS.** KNOWLEDGE §5 rule 5 and the availability `complete` state are vacuously true for one rootless entry and for no entries.
- **HM5-GAP-UNVERIFIED-RELIANCE.** CONTEXT §14 "whose latest evaluation for the basis is now `needs_check` or `unknown`" does not say whether it applies to `hypothesis`/`reference` items, whose satisfaction never needed applicability (HM5-UNVERIFIED-BY-TABLE).

#### Coverage limits

- `context.knowledge_provider` peers need the Unix-socket binding; the key stops the provider (exit 2). `execution.claim_revalidation` is not implemented or claimed: it applies only to bindings with `fetch.context`, which this executor cannot use (M4 limit). `composition.claims-in-packets-block-only-the-required-boundary` is skipped, so SCN-16 and the Execution side of CMP-9 are not independently checked.
- Claims are read only from this provider's own knowledge store; a claim at another provider is `knowledge_unavailable`.
- The fixture-sensitivity probe covered 25 deviations over the 14 M5 fixtures; `tests/fixture_sensitivity.py` is not extended.

#### Final run

`run: 271 fixtures (243 pass, 24 skipped, 4 unsupported), 0 not passing` (at `9de18c9`, and repeated after this record). The 14 M5 fixtures pass; the 4 unsupported are backpressure; the 24 skipped are 12 `socket.*` and 12 `composition.*`. `tests/check_vectors.py`, `probe_provider.py` (19/19), `probe_m2.py` (43/43) and `probe_f.py` (31/31) still pass.

### H.M5b Realignment (base 6cec5f6)

> Twelfth spec-only pass, a realignment with the resolution of H.M5 ([M5-DIVERGENCES](../../../docs/work/release-0.1/M5-DIVERGENCES.md)). The branch was fast-forwarded to `6cec5f6`. Read first: that record; `git diff 950f3e0 6cec5f6 -- docs/spec schemas conformance/README.md conformance/schemas docs/work/release-0.1/M5-DIVERGENCES.md docs/work/release-0.1/M5.md` (KNOWLEDGE §2, §3, §4, §5, §6, §7, §8, §10; VERIFICATION §1, §3, §4, §5, §7, §10, §11; CONTEXT §12, §14; the knowledge common schema's finding `reason`; the launch-configuration schema's scripted `reason` rule; the README's `$base64_json` and substitution notes); the current KNOWLEDGE and VERIFICATION in full and CONTEXT §14; CORE §10 and §12 for the step 7 order and the `precondition_failed` details. Not read: `conformance/reference/**`, `conformance/runner/src/**`, `conformance/crosscheck/**`, their history or diffs, and the `mutants` lists of the reference descriptors. The runner was rebuilt at this base and used as a binary. This subsection's readings were written before any fixture was opened; the implementation follows in its own commit, and fixtures are read only after the first complete run, and only those that fail.

#### Change list, from the documents

| # | Resolved text | Before (H.M5) | Change |
|---|---|---|---|
| 1 | KNOWLEDGE §8: dependencies resolve only as exact local references; `unchecked` carries `reason` | Any evaluation of the same provider, claim, revision and digest; no reason; a self-naming dependency `unchecked` only when it named the exact reference | `remote_dependency` for another provider; `self_reference` for the evaluated revision's claim ID and revision, whatever the digest; `unresolved` when no revision with that claim ID, revision and digest exists here; `not_evaluated` without an evaluation of that exact reference for an equal target; `not_established` for a latest `needs_check` or `unknown`. `match`/`mismatch` as before. |
| 2 | KNOWLEDGE §10 "Rights on named revisions" | HM5-NAMED-REVISION-RIGHTS: read on dependency and claim-root claims for propose and revise, on the own claim for revise, on `selected` for resolve | `decision.record`: read on its claim; `conflict.open`: both claims; `applicability.evaluate`: its claim and every local claim its dependencies name; nothing further for resolve, revise or claim content. Remote references add none. |
| 3 | KNOWLEDGE §3 `validity` | HM5-VALIDITY-ORDER: only `from` after `until` refused | `from` at or after `until` is `invalid_envelope` at `/payload/validity` |
| 4 | KNOWLEDGE §3 "Unique IDs" | HM5-DUPLICATE-IDS: accepted | Duplicate `support_id` at `/payload/support`, duplicate `condition_id` at `/payload/conditions`, step 2 |
| 5 | KNOWLEDGE §5 roots | Only non-empty roots with `unknown` refused | `complete` or `partial` with no roots is also `invalid_envelope` at `/payload/support/<i>/ancestry/roots` |
| 6 | KNOWLEDGE §5 availability | HM5-AVAILABILITY: no entries was `complete` | No entries: `unknown` |
| 7 | KNOWLEDGE §6 and §7 step 7: Core revision preconditions first | HM5-CORE-PRECONDITION-ORDER: after the epoch check for decisions and resolutions | Preconditions first for `decision.record` and `conflict.resolve`, then the profile's list (not found or resolved, authority, epoch, digest, supersession, `selected`) |
| 8 | VERIFICATION §4 step 1: an unlisted evaluator version is `unknown` (CORE §17.1) | HM5-UNLISTED-EVALUATOR (fixture-driven): `unsupported` | `unknown`; a listed version reports its configured status |
| 9 | VERIFICATION §4 "ID shared with receipts" and step 0; §4 order after Core preconditions | HM5-RECEIPT-ID-TAKEN: accepted, and the job never completed | `evaluate_contract` whose job ID names a receipt, and `receipt.record` whose receipt ID names a job: `precondition_failed` with `failed: [ { subject, expected: 0 } ]` naming the existing subject. Order for `evaluate_contract`: Core preconditions, receipt collision, evaluator capability, contract, roles. |
| 10 | VERIFICATION §4 "Issued receipt members" | HM5-VERIFIER-PRINCIPAL: `scope: "verification job <id>"`; `receipt.issued` after the artifact's events, with `job` as a subject | `scope` is the contract's `outcome`; `verification.receipt.issued { reference, job: <job ID> }`, then the artifact's events, then `verification.job.changed` to `completed`. Environment observed only and `inputs`/`provenance` empty, as before. |
| 11 | VERIFICATION §11 and the launch schema: a scripted `reason` exactly for `not_evaluated` and `indeterminate` | HM5-SCRIPT-REASON: missing reason filled with the result, extra reason dropped | Both are an invalid launch configuration (exit 2) |
| 12 | VERIFICATION §3 contract format and read order | HM5-CONTRACT-FORMAT: `format` optional | `format` required and equal to `combraton-verification-contract/1`; `subjects` and `properties` non-empty. The read order was already as written. |
| 13 | VERIFICATION §7 table: with the contract unreadable, Environment and Evaluator are `unverifiable` with `contract_unavailable`; with the receipt unusable, Subjects, Environment, Evaluator, Time and the contract comparison are `unverifiable` with the receipt's reason | HM5-ASSESS-UNREADABLE-RECEIPT: with both unusable, `contract_unavailable` was not added to Environment and Evaluator | Both reasons when both apply |
| 14 | VERIFICATION §7 "Overall" | HM5-OVERALL-WITH-FAILED-CHECKS: a cap after the property rule | Written as the new rule: checks first, then empty properties, then required properties. Same results. |
| 15 | CONTEXT §14 "What a claim section can satisfy": a section that is not `historical` | HM5-CLAIM-SECTION-ITEM: a historical section could satisfy a `hypothesis` or `reference` item | Only a non-historical section satisfies; see HM5B-HISTORICAL-CLAIM-REASON |
| 16 | CONTEXT §14 `applicability_changed` compares the result only | HM5-CLAIM-CHANGES: result or evaluation ID | Result only |
| 17 | CONTEXT §12 "Scripted unmet reasons": a scripted `unmet` reason takes precedence, including over a claim check | HM5-CLAIM-READ-TIME: claim reasons ahead of scripted `unmet` | A scripted `unmet` reason wins for an unsatisfied `claim_included` item |

**Already as resolved, unchanged:** the basis-to-target conversion (HM5-PACKET-BASIS-TARGET, now CONTEXT §14); `authority.get` `revision` (the binding's Core revision equals its epoch here); binding under a grant `not_authority` (HM5-BIND-GRANT); `recorded` holding reported results only; `receipt.inspect` of unreadable bytes `unavailable`; `receipt.recorded` before the artifact's events, with `appended` among them (VERIFICATION §5 now leaves those events open); listing details as three lists; observed environment only; a script without `complete` leaves the job running; the time boundaries (HM5-TIME-BOUNDS); `not_found` for an authorized reader naming a missing receipt; reference mismatch empties `properties`; `applicability_not_established` in `unverified_items` only for `binding` and `evidence` items (HM5-UNVERIFIED-BY-TABLE); open conflicts only in snapshots; the snapshot read at publication (an instant the text now leaves to the provider); format `/1` for requests submitted without `context.claims`; `receipt.inspect` and `receipt.assess` in the base profile; `latest_decision: null`; `authority_epoch` required for resolve; `observed_from` after `observed_until` at `/payload/observed_until`; inspect conflicts naming exactly the revision, open and resolved, in recorded order (subjects are listed in creation order).

#### Readings of points the resolved documents leave open

Each was written before any fixture was opened. Basis: own judgment unless stated.

- **HM5B-SELF-OR-REMOTE** (KNOWLEDGE §8 table). A dependency naming another provider, with the evaluated revision's claim ID and revision, fits both `self_reference` ("the evaluated revision's own claim ID and revision, whatever its digest") and `remote_dependency` ("names another provider"). The table does not say whether its rows are ordered. Under §2 a reference naming another provider never names a local revision, so the provider is checked first: `remote_dependency`. Then `self_reference` (same provider, claim ID and revision, any digest), `unresolved`, `not_evaluated`, `not_established`.
- **HM5B-EVALUATE-DEPENDENCY-RIGHTS** (KNOWLEDGE §10). The dependencies whose claims need `knowledge.read` are those of the revision the command names. At step 6 that revision is looked up by provider, claim ID and revision, before the digest check of step 7; a claim or revision that does not exist, or a reference naming another provider, adds no dependency rights (the command then fails `not_found` at step 7). Looking the dependencies up only under an exact digest would be the other reading; it differs only for commands that fail at step 7 anyway.
- **HM5B-STEP7-CORE-ORDER** (CORE §10 step 7 against KNOWLEDGE §6 and VERIFICATION §4). CORE §10 orders step 7 as capabilities, authority epoch, then preconditions. KNOWLEDGE §6 now checks preconditions first and the authority epoch fourth, and VERIFICATION §4 checks the evaluator predicate after the preconditions. The profile text is the more specific and is followed; the tension with CORE's general order is not stated anywhere.
- **HM5B-COLLISION-DETAILS** (VERIFICATION §4). `failed: [ { subject, expected: 0 } ]` names the existing receipt or job. CORE §12 details carry `current` "if permitted"; the entry includes `current` when the principal may read that subject, as for any precondition entry. For `receipt.record`, the collision is checked after the Core preconditions and before the contract, as for `evaluate_contract`'s step 0 (§5 gives no order).
- **HM5B-HISTORICAL-CLAIM-REASON** (CONTEXT §14). A `claim_included` item whose section for it carries its reference but is `historical` (applicability `invalid_for_target`) is not satisfied. For `binding` and `evidence` items the table gives `invalid_for_target` (after `not_accepted`). For `hypothesis` and `reference` items the table has no reason for it; `invalid_for_target` is used, after `not_accepted` for a rejected claim. At the read, a required item of any reliance satisfied at publication whose claim is now `invalid_for_target` "no longer meets" the satisfaction rule, so it appears in `invalidated_items` with `invalid_for_target`; `needs_check` and `unknown` still affect only `binding` and `evidence` items.
- **HM5B-SCRIPTED-UNMET-SCOPE** (CONTEXT §12). The precedence applies to the reason of an item that is not satisfied, including `knowledge_unavailable` and `claim_digest_mismatch`; a scripted `unmet` step does not make a satisfied item unmet, as for the M4 checks.
- **HM5B-QUEUED-LOSS** (VERIFICATION §4). `observed_from` is "when the job started running", and a job is `running` "from its first evaluation". A queued job whose evaluator is lost before it ran completes at its first evaluation: it passes through `running` at that evaluation (with its `verification.job.changed`), so `observed_from` equals `observed_until`.
- **HM5B-ISSUED-EVENTS** (VERIFICATION §4). The receipt artifact's events for an issued receipt are the provider-origin `evidence.artifact.staged`, `appended` and `sealed`, as §5 allows for a holder's own publication.
- **HM5B-UNUSABLE-REASON-ORDER** (VERIFICATION §7). With the contract unreadable and the receipt unusable, Environment and Evaluator list `contract_unavailable` then the receipt's reason, and the Contract check likewise, in the order of the table's cells. The top-level `reasons` list every non-passed check's reasons, deduplicated, in check order.
- **HM5B-TIME-WITHOUT-CONTRACT** (VERIFICATION §7). With the contract unreadable, `max_age_seconds` is unknown; the Time row names no contract reason, so Time is decided from `valid_until` and `observed_until` alone. `properties` is empty and `overall` is `not_satisfied` anyway.
- **HM5B-CONTRACT-MEMBERS** (VERIFICATION §3). "Content that is not a contract as the table above requires": a member the table does not list, or an ill-typed member, is still `invalid_format` here, as in H.M5; the table does not say whether contracts are closed objects. Unchanged.
- **HM5B-DUPLICATE-PATH-ORDER** (KNOWLEDGE §3, §5). Each support entry is validated in order, including its roots rule; duplicate `support_id` values are checked after the entries, and duplicate `condition_id` values after the conditions.
