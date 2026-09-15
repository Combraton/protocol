# minimal-publisher: an independent Evidence publisher (S-B)

A contracts-only third-party client for Protocol 0.1 (M6, owner decision M6-Q2). Python 3, standard library only. Run it as `python3 publisher.py --config <file> [--mutant uploads-bytes-differing-from-digest]`.

## Role, and what it is not

- **What it is.** A one-shot Evidence producer. It publishes the configured artifacts, verifies each one by fetching it back, and exits: 0 on success, 1 on any refusal or mismatch.
- **What it is not.**
  - It is not an Evidence provider. It serves nothing.
  - It is not a verifier. Receipts and assessments are the reference Verification provider's work, reached by the harness.

## Sessions and negotiation

The publisher opens one Unix-socket session with `evidence.provider`. It runs `core.authenticate`, then `core.feature_dependencies`, then `core.negotiate`. When the query answers `method_not_found`, it falls back to EVIDENCE §1. It requests as required `evidence/1`, and `core/1` with the dependencies (`core.events`) and `core.grants`. `core.grants` is needed because every Evidence operation carries the configured grant (TP-1).

## Behavior

For each artifact, in order, the publisher:

1. decodes `content_base64` and computes the SHA-256 digest of those bytes;
2. sends `evidence.upload.prepare` with that digest and size and precondition revision 0;
3. sends `evidence.upload.append` in chunks of at most `min(chunk_bytes, chunk_limit)` decoded bytes, each with a precondition on the artifact's current revision;
4. sends `evidence.seal` with a precondition on the current revision;
5. fetches the sealed bytes with `evidence.fetch`, in as many calls as needed. It hashes exactly the bytes received and compares the result with the digest it computed. The provider's advertised digest is never taken as evidence.

Command IDs are derived from the artifact ID, so a sequence retransmitted after a lost connection replays. On success it writes `{ format: "combraton-thirdparty-publisher-result/1", artifacts: [ { id, digest, size } ] }` as canonical JSON to `result_file`, when one is given.

Credentials are read only from the configuration file. They are never put on the command line, in the environment or in a log line.

## Mutant

`--mutant uploads-bytes-differing-from-digest` declares the digest and size of the configured bytes, but appends them with the last byte altered. The provider must refuse the seal with `content_digest_mismatch` (EVIDENCE §4), and the publisher exits 1.

## Written from

- `docs/spec/SPEC.md`
- `docs/spec/bindings/STREAM.md` and `docs/spec/bindings/ENCODING.md`
- `docs/spec/profiles/CORE.md` (§3–§6, §10–§12, §15, §18)
- `docs/spec/profiles/EVIDENCE.md`
- `docs/spec/profiles/VERIFICATION.md` §1 (dependency fallback table only)
- `docs/decisions/003`, `006`
- `schemas/core/1`, `schemas/evidence/1`, `schemas/stream/1`
- `conformance/thirdparty/README.md` and `docs/work/release-0.1/M6.md` (mixed-implementation composition)

The shared client code is in `../common/combraton_client.py`.

## Coverage

- **Demonstrated** (fixture `composition.thirdparty-publisher-feeds-reference-verification`). A second implementation publishes Evidence: prepare, chunked append and seal, with an exact digest over the bytes it sent and a fetch-back digest check. The reference Verification provider then consumes it by exact reference and digest.
- **Not claimed.**
  - verification by a second implementation;
  - manifests, holds, purges and work-bound grants;
  - recovery from a crash between commands: it keeps no durable command journal;
  - the Evidence and Verification providers, which are the reference.
