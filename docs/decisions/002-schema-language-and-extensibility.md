# 002: Schema language and extensibility rules

- **Status:** proposed. Needs owner review before release.
- **Date:** 2026-09-13.
- **Owner/authority:** Protocol session under the Protocol 0.1 kickoff; [tracking issue #1](https://github.com/Combraton/protocol/issues/1).
- **Affects:** every file under `schemas/`; [Core §5](../spec/profiles/CORE.md#5-envelopes); matrix rows REL-1, CORE-3, CORE-4.
- **Supersedes:** nothing. Implements [SPEC §11](../spec/SPEC.md#11-versioning-and-transport): "unknown optional metadata may be preserved; unknown required semantics fail closed". Also the README rule: "Keep schemas language-neutral, with separately versioned bindings".

## Problem

Messages need machine-readable schemas that implementations in Rust, TypeScript, Python, Go and other languages can validate identically. Schemas alone cannot say which unknown additions are safe to ignore. The protocol needs an explicit rule, so that a newer peer's required meaning is never silently dropped by an older one.

## Evidence consulted

A read-only research pass on 2026-09-13 inspected:

- **[JSON Schema 2020-12 core](https://json-schema.org/draft/2020-12/json-schema-core).** `unevaluatedProperties` closes objects across `allOf` and `$ref` composition. There is no discriminator keyword; unions are `oneOf` with `const` tags.
- **[Bowtie 2020-12 compliance data](https://bowtie.report/draft2020-12.json)** (run 2026-09-13, 1,301 tests):
  - Zero failures: Rust `jsonschema` 0.47.0 and `boon` 0.6.1, Go `santhosh-tekuri/jsonschema` v6.0.2, TypeScript `json-schema-library` 11.6.2 and `@hyperjump/json-schema` 1.17.7.
  - Python `jsonschema` 4.26.0: one failure and five errors, from `\p` regex escapes.
  - `ajv` 8.20.0: 80% under default strict settings.
- **[JSON Type Definition, RFC 8927](https://www.rfc-editor.org/rfc/rfc8927).** Native discriminators and closed objects, but no 64-bit integers. Reference tooling was last released in 2020–2021.
- **[CDDL, RFC 8610](https://www.rfc-editor.org/rfc/rfc8610) and [RFC 9165](https://www.rfc-editor.org/rfc/rfc9165).** Strong extension points (sockets, `.feature`), but essentially one maintained JSON validator (`cddl-rs`) and none found for Go.
- **[Protobuf JSON mapping](https://protobuf.dev/programming-guides/json/).** Parsers reject unknown fields by default and generally do not propagate them in JSON.
- **Criticality precedents:**
  - [RFC 7515 §4.1.11](https://www.rfc-editor.org/rfc/rfc7515#section-4.1.11) (JWS `crit`: listed extensions must be understood; unlisted may be ignored).
  - [RFC 5280 §4.2](https://www.rfc-editor.org/rfc/rfc5280#section-4.2) (reject unrecognized critical certificate extensions).
  - [RFC 9113 §5.5](https://www.rfc-editor.org/rfc/rfc9113#section-5.5) (extensions that change semantics must be negotiated first).
  - [RFC 8446 §4.2](https://www.rfc-editor.org/rfc/rfc8446#section-4.2).
  - Kubernetes strict field validation, per [API concepts](https://github.com/kubernetes/website/blob/8650a29/content/en/docs/reference/using-api/api-concepts.md).

## Decision (proposed)

1. **Schema language.** Normative schemas use JSON Schema 2020-12, one file per message type, organized as `schemas/<profile>/<major>/`.
   - Core and profile objects are closed with `unevaluatedProperties: false`.
   - Unions are `oneOf` branches pinned by `const`.
   - Patterns avoid Unicode property escapes (`\p{…}`) for portability.
   - Schemas define structure only. Operation semantics live in profile documents and fixtures.
2. **Closed objects.** A field not defined by the negotiated profile versions and features is `invalid_envelope`. An unknown field cannot be classified as optional, so it is treated as required and refused, like Kubernetes strict validation.
3. **`extensions` and `requires`.**
   - Optional additions go in `extensions`, keyed by `domain/name`.
   - A message's `requires` array lists features and extension keys its meaning depends on, following the JWS `crit` rule: listed means must be understood.
   - Required extensions are part of the command digest; optional ones are not.
4. **Features before use.** A caller sends fields belonging to a feature only if negotiation selected that feature, as HTTP/2 requires for semantic extensions.
5. **Bindings.** Generated or hand-written language bindings are not normative and are versioned separately. The release plan (U5) proposes shipping none as normative in 0.1.

## Alternatives

| Alternative | Why not selected |
|---|---|
| **Open objects that ignore unknown fields** | Easy forward compatibility, but a new required meaning is silently lost by old receivers, violating fail-closed. |
| **JTD** | Semantically neat, but the tooling is abandoned. |
| **CDDL** | Excellent extension model, but thin validator coverage across target languages. |
| **Protobuf with its JSON mapping** | Strong codegen, but drops unknown fields in JSON and is not language-neutral JSON-first. |
| **Authoring in TypeSpec and emitting JSON Schema** | Viable later for authoring convenience. The emitted 2020-12 would still be the normative artifact. Deferred so the first release does not add a Node toolchain to the contract. |
| **Per-field `critical: true` flags (X.509 style)** | Scatters the rule through every object. A single `requires` list is easier to validate and to cover in the digest. |

## Consequences

- An older provider refuses newer fields rather than ignoring them. Callers must negotiate features first, which the handshake makes cheap.
- Python's `jsonschema` gaps are avoided by not using `\p` escapes. A schema lint check enforces this.
- Schema validation passing is necessary, not sufficient. Fixtures still check semantics.

## Verification

- A CI check validates every schema against the 2020-12 metaschema and rejects `\p` escapes.
- Every fixture message is validated against its schema.
- M1 fixtures cover:
  - unknown top-level field (refused);
  - unknown optional extension (accepted);
  - unknown required extension (refused);
  - `requires` naming an absent extension (invalid);
  - duplicate `requires` entry (invalid).
- A mutant provider that accepts unknown fields must fail.
