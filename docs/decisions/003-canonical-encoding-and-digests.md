# 003: Canonical encoding and digest representation

- **Status:** proposed. Needs owner review before release.
- **Date:** 2026-09-13.
- **Owner/authority:** Protocol session under the Protocol 0.1 kickoff; [tracking issue #1](https://github.com/Combraton/protocol/issues/1).
- **Affects:** [ENCODING](../spec/bindings/ENCODING.md), the JSON value rules in [STREAM](../spec/bindings/STREAM.md), the command digest in [Core §6](../spec/profiles/CORE.md#6-command-identity-and-idempotency), and later content digests. Matrix rows CORE-6, CORE-7.
- **Supersedes:** the illustrative `"payload_digest": {"sha256": …}` map in [SPEC §3](../spec/SPEC.md#3-identity-and-envelope). SPEC names are illustrative until frozen with fixtures. If accepted, the SPEC example should be updated in a follow-up change.

## Problem

Idempotency compares command digests. Independent implementations must therefore compute the same digest for the same command, and different digests for different commands. Artifacts and packets need content identity that survives transport. Digests must name their algorithm.

## Evidence consulted

A read-only research pass on 2026-09-13, including a local interoperability probe:

- **[RFC 8785 (JCS)](https://www.rfc-editor.org/rfc/rfc8785).** Requires I-JSON input; sorts members by UTF-16 code units; defines string escaping and ECMAScript number formatting. Explicitly no Unicode normalization. Large numbers must be wrapped in strings.
- **[RFC 7493 (I-JSON)](https://www.rfc-editor.org/rfc/rfc7493).** No duplicate names, no surrogates or noncharacters; integers exact only within ±(2^53 − 1).
- **[RFC 8259 §4](https://www.rfc-editor.org/rfc/rfc8259#section-4).** Parsers disagree on duplicate names.
- **Interoperability probe** (Node 22 `canonicalize` 5.0.0, Python 3.14 `rfc8785` 0.1.4, Go `gowebpki/jcs` v1.0.1, Rust `serde_jcs` 0.2.0 and `serde_json_canonicalizer` 0.3.2):
  - JavaScript, Go and Rust gave `9007199254740993` and `9007199254740992` the same digest; Python refused both.
  - JavaScript, Rust and Python silently took the last of duplicate members; Go refused them.
  - All agreed on UTF-16 member ordering, and none normalized Unicode.
- **[DSSE background](https://github.com/secure-systems-lab/dsse/blob/1d3370f62565/background.md).** Warns that semantically different payloads can share a canonical encoding.
- **[Protobuf "serialization is not canonical"](https://protobuf.dev/programming-guides/serialization-not-canonical/).** Deterministic protobuf serialization is not stable across implementations.
- **Digest representations:**
  - [OCI image-spec descriptor digests](https://github.com/opencontainers/image-spec/blob/v1.1.1/descriptor.md#digests): `algorithm:encoded`, SHA-256 mandatory.
  - [in-toto DigestSet](https://github.com/in-toto/attestation/blob/2dcd055e9f72e746687c306e35f4e59720ff45be/spec/v1/digest_set.md): algorithm map, consumers ignore unrecognized algorithms.
  - [SRI 2](https://www.w3.org/TR/sri-2/): unknown algorithms fail open.

## Decision (proposed)

1. **Value domain stricter than I-JSON.**
   - Integers only, within ±(2^53 − 1), without fraction, exponent or `-0`. Decimals and large numbers go in strings.
   - No duplicate names, surrogates or noncharacters.
   - Enforced when a frame is parsed, before any digest is computed.
2. **Command digest.** `sha256` over the RFC 8785 canonical form of the parsed intent object. Receivers recompute from parsed values.
3. **Content digests.** Over exact stored bytes, never re-serialized.
4. **Digest string.**
   - OCI grammar with lowercase hex.
   - `sha256` mandatory; `sha512` an optional negotiated feature; MD5 and SHA-1 never supported.
   - Unsupported algorithms fail closed with `unsupported_digest_algorithm`.
5. **Two independent implementations.**
   - The Rust runner uses a vetted RFC 8785 crate over its own strict value type.
   - The Rust reference provider implements the canonical form itself.
   - The shared test vectors are also checked in CI by Python `rfc8785` and Node `canonicalize`, two independent non-Rust implementations.

## Alternatives

| Alternative | Why not selected |
|---|---|
| **Digest over exact transmitted bytes for commands** | No canonicalization, but a JSON value nested in a JSON-RPC frame has no preserved bytes in common parsers. Retransmissions with different whitespace would conflict. |
| **Full I-JSON number domain with ECMAScript formatting** | Allows decimals, but number formatting is the most error-prone part of JCS. Silent rounding above 2^53 creates digest collisions between different commands. |
| **Deterministic CBOR** | Rigorous, but changes the wire format and still needs application rules for floats. |
| **Digest maps (in-toto DigestSet)** | Good for export, but "ignore unrecognized algorithms" is the wrong default for idempotency. Map equality is also less direct than string equality. Kept as an export mapping. |
| **Unicode normalization before digesting** | Would merge values that users and tools treat as different bytes. RFC 8785 forbids it. |

## Consequences

- Schemas must represent any non-integer or large quantity as a string with a declared format, such as usage costs, byte counts above 2^53 or timestamps.
- Implementations need a parser that detects duplicate names and non-integer tokens. JavaScript's `JSON.parse` cannot, so TypeScript implementations need a stricter parser.
- Consumers must not recompute a content digest from re-serialized JSON.

## Verification

- `conformance/vectors/encoding.json` holds canonical-form and digest vectors:
  - member order with astral and BMP characters;
  - escapes;
  - identical values written differently;
  - NFC versus NFD;
  - integer bounds;
  - rejected inputs: duplicates, `1.0`, `2^53`, lone surrogates, noncharacters.
- The runner's self-test checks its implementation against the vectors.
- M1 fixtures check that a provider:
  - accepts a command whose correct digest depends on UTF-16 ordering and non-ASCII strings;
  - refuses a wrong digest, an unsupported algorithm and malformed digests.
- Mutants that sort by code point or escape non-ASCII must fail.
