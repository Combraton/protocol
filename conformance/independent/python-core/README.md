# Independent Python Core provider

> **Status: conformance evidence for Protocol 0.1 (milestone M2), not a product.** It exists to test the fixtures and the reference provider for assumptions they share.

`combraton-independent-python-core` `0.1.0-dev.0` is a second implementation of the Core provider. It speaks the stdio form of the stream binding and implements `core/1` together with the conformance-only `core-test/1` profile. Its value is that it was written **from the published documents only**, without reading the reference provider. Where the documents leave a question open, it records the question instead of copying the reference's answer.

## What it was written from

Allowed and read:

- `docs/spec/profiles/CORE.md`, `docs/spec/bindings/STREAM.md`, `docs/spec/bindings/ENCODING.md`
- `schemas/**`
- `conformance/README.md`, `docs/VERIFICATION.md`, `conformance/schemas/fixture.schema.json`
- `conformance/fixtures/**`, `conformance/vectors/encoding.json`
- `conformance/participants/reference-provider.json`, used only as a format example for the descriptor
- the transcripts and manifests the runner wrote for this implementation

Not read: `conformance/reference/**`, `conformance/crosscheck/**` and `conformance/runner/src/**`. The runner was used only as a black-box binary.

The fixtures were read **before** the code was written. Some choices on points the prose leaves open were therefore informed by what the fixtures expect. [DIVERGENCES.md](DIVERGENCES.md) marks each such choice.

## Layout

| File | Contents |
|---|---|
| `provider.py` | Entry point: stream framing (STREAM 1–5), JSON-RPC mapping, the CORE 10 processing order, negotiation, and the core-test operations |
| `valuedomain.py` | Strict iterative parser for the value domain, RFC 8785 canonical form for integer-only values, digests, and size/depth measurement |
| `envelope.py` | Hand-written closed-object and type validation that mirrors the schemas (no JSON Schema library) |
| `state.py` | Durable state in SQLite: subjects, bound command records, deduplication window |
| `tests/check_vectors.py` | Checks `valuedomain.py` against `conformance/vectors/encoding.json` |
| `tests/probe_provider.py` | Probes behavior the fixtures do not exercise; each probe is tagged with its DIVERGENCES entry |
| `tests/fixture_sensitivity.py` | Applies one deliberate deviation at a time to a temporary copy and reports which fixtures notice |

It needs Python 3.12 or later and uses only the standard library.

## Running

From the repository root:

```sh
cargo build -p combraton-conformance --locked
./target/debug/combraton-conformance run --participant conformance/participants/independent-python-core.json --out conformance/results/independent-python-core
python3 conformance/independent/python-core/tests/check_vectors.py
python3 conformance/independent/python-core/tests/probe_provider.py
python3 conformance/independent/python-core/tests/fixture_sensitivity.py
```

To run the provider by hand:

```sh
python3 conformance/independent/python-core/provider.py --data-dir DIR --config FILE
```

`FILE` is a launch configuration such as:

```json
{"format": "combraton-conformance-config/1", "principal": "conformance-caller",
 "limits": {"max_depth": 6}, "dedupe": {"advance_on_start": 2, "retain_generations": 1}}
```

- `principal` names the deduplication scope. Any JSON value is accepted and compared by its canonical form.
- `limits` partially overrides the defaults (frame 1 MiB, payload 262,144 bytes, string 65,536 bytes, array 1,024 items, depth 32).
- On each start, `dedupe.advance_on_start` moves `current` forward by that many generations.
- `dedupe.retain_generations` keeps that many generations, counting `current`. It raises `oldest_retained` and discards older records.

Unknown configuration keys stop the provider with exit status 2. The semantics of these keys are this implementation's choice; see DIVERGENCES D-TESTCTL.

## Result

The first complete run passed all 57 fixtures. No behavior was changed in response to a fixture failure.

That result is weaker than it looks. `tests/fixture_sensitivity.py` shows that most deliberate deviations from the documents still pass all 57 fixtures; see DIVERGENCES section D.

## Limits

- Stdio binding only; there is no Unix-socket form. Requests on a connection are processed one at a time.
- Grants, events, capability snapshots and effects (all M2) are not implemented. Authorization (CORE 10 step 6) is skipped.
- Only one principal per process, taken from the launch configuration. Cross-scope deduplication (CORE-12) is untested.
- `core.digest-sha512` is implemented, and sha512 digests are accepted only in sessions that negotiated it (D-SHA512).
- No extension is understood. Any extension key listed in `requires` is refused, and optional extensions are dropped (`unknown_extensions: "drop"`).
- `core-test/1` is always exposed, because this provider only runs under the conformance launch configuration (D-TESTPROFILE).
- Durability relies on SQLite with `synchronous=FULL`. Crash-consistency has not been exercised beyond the suite's clean restarts.
