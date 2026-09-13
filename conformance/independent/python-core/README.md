# Independent Python Core provider

> **Status: conformance evidence for Protocol 0.1 (milestone M2), not a product.** It exists to test the fixtures and the reference provider for assumptions they share.

`combraton-independent-python-core` `0.1.0-dev.0` is a second implementation of the Core provider. It speaks the stdio form of the stream binding and implements `core/1` together with the conformance-only `core-test/1` profile, including the M2 Core features `core.grants` (CORE §15), `core.events` (§16) and `core.capabilities` (§17). Its value is that it was written **from the published documents only**, without reading the reference provider. Where the documents leave a question open, it records the question instead of copying the reference's answer.

It was written in two passes by separate spec-only helpers: the Core command path (M1 fixtures, base `f42d21a`), then the three M2 features (base `3b32037`). The second pass also read the resolution record [M2-DIVERGENCES](../../../docs/work/release-0.1/M2-DIVERGENCES.md).

## What it was written from

Allowed and read:

- `docs/spec/profiles/CORE.md`, `docs/spec/bindings/STREAM.md`, `docs/spec/bindings/ENCODING.md`
- `schemas/**`
- `conformance/README.md` (including its launch configuration section), `docs/VERIFICATION.md`, `conformance/schemas/fixture.schema.json`, `conformance/schemas/launch-config.schema.json`
- `docs/work/release-0.1/M2-DIVERGENCES.md` (second pass)
- `conformance/fixtures/**`, `conformance/vectors/encoding.json`
- `conformance/participants/reference-provider.json`, used only as a format example for the descriptor
- the transcripts and manifests the runner wrote for this implementation

Not read: `conformance/reference/**`, `conformance/crosscheck/**` and `conformance/runner/src/**`. The runner was used only as a black-box binary.

The fixtures were read **before** the code was written. Some choices on points the prose leaves open were therefore informed by what the fixtures expect. [DIVERGENCES.md](DIVERGENCES.md) marks each such choice.

## Layout

| File | Contents |
|---|---|
| `provider.py` | Entry point: stream framing (STREAM 1–5), JSON-RPC mapping, the CORE 10 processing order, negotiation, authorization (step 6), capability checks (step 7), subscriptions, and the Core and core-test operations |
| `valuedomain.py` | Strict iterative parser for the value domain, RFC 8785 canonical form for integer-only values, digests, and size/depth measurement |
| `envelope.py` | Hand-written closed-object and type validation that mirrors the schemas (no JSON Schema library) |
| `grants.py` | Resource coverage, grant usability and delegation bounds (CORE 15) |
| `events.py` | Cursors and the ordered read of events, epoch changes and retention gaps (CORE 16.4) |
| `state.py` | Durable state in SQLite: subjects (including grants), bound command records, deduplication window, event stream, epochs, retention snapshot, capability snapshot |
| `tests/check_vectors.py` | Checks `valuedomain.py` against `conformance/vectors/encoding.json` |
| `tests/probe_provider.py` | Probes M1 behavior the fixtures do not exercise; each probe is tagged with its DIVERGENCES entry |
| `tests/probe_m2.py` | Probes grants, events and capabilities behavior the fixtures do not exercise (DIVERGENCES section E tags) |
| `tests/fixture_sensitivity.py` | Applies one deliberate deviation at a time to a temporary copy and reports which fixtures notice (`m1`, `m2` or both) |

It needs Python 3.12 or later and uses only the standard library.

## Running

From the repository root:

```sh
cargo build -p combraton-conformance --locked
./target/debug/combraton-conformance run --participant conformance/participants/independent-python-core.json --out conformance/results/independent-python-core
python3 conformance/independent/python-core/tests/check_vectors.py
python3 conformance/independent/python-core/tests/probe_provider.py
python3 conformance/independent/python-core/tests/probe_m2.py
python3 conformance/independent/python-core/tests/fixture_sensitivity.py m2
```

To run the provider by hand:

```sh
python3 conformance/independent/python-core/provider.py --data-dir DIR --config FILE
```

`FILE` is a launch configuration such as:

```json
{"format": "combraton-conformance-config/1", "principal": "agent-1", "authority_principals": ["owner"],
 "limits": {"max_depth": 6}, "dedupe": {"advance_on_start": 2, "retain_generations": 1},
 "events": {"new_epoch_on_start": true, "retain_last": 10},
 "capabilities": {"core-test.writes": "unsupported"}, "clock": {"fixed": "2030-01-01T00:00:00Z"}}
```

The keys follow the conformance README's launch configuration table:

- `principal` (a string) is the session principal and names its deduplication scope. `authority_principals` defaults to `[principal]`; `provider_id` defaults to `conformance-provider`.
- `limits` partially overrides the defaults (frame 1 MiB, payload 262,144 bytes, string 65,536 bytes, array 1,024 items, depth 32).
- On each start, `dedupe.advance_on_start` moves `current` forward by that many generations, and `dedupe.retain_generations` keeps that many generations, counting `current`.
- On each start, `events.new_epoch_on_start` closes the current stream epoch, vouched through its last sequence. `events.retain_last` then discards all but the newest N events, folding them into the snapshot that retention gaps report.
- `capabilities` sets the status of `core-test.writes`, the only capability this provider has. Without it the predicate is `supported` on the evidence of the start's store write.
- `clock.fixed` fixes the provider clock; otherwise the system clock (UTC, seconds) is used.

Unknown configuration keys, unknown capability names and ill-typed values stop the provider with exit status 2. Where the README leaves a meaning open (for example the order of these start-time changes), DIVERGENCES D-TESTCTL and E-START-ORDER record the choice.

## Result

The first complete run of the M1 pass passed all 57 fixtures then in the suite. No behavior was changed in response to a fixture failure.

The first complete run of the M2 pass, with `core.grants`, `core.events` and `core.capabilities` claimed, passed 107 of 108 fixtures. The failure, `core.events.retention-gap-returns-snapshot`, is a disagreement between that fixture and CORE §16.4 as this implementation reads it (DIVERGENCES E-GAP-TO). It was left failing rather than worked around, so `run` exits 1 for this participant until the fixture or the text changes.

Passing is weaker evidence than it looks. `tests/fixture_sensitivity.py` shows which deliberate deviations from the documents still pass every fixture; see DIVERGENCES sections D and E.4.

## Limits

- Stdio binding only; there is no Unix-socket form. Requests on a connection are processed one at a time, and subscription notifications are sent only after the response to the request being processed.
- One principal per process, taken from the launch configuration. Cross-principal behavior (grants held by others, per-principal deduplication) is exercised across restarts over the same data directory, never concurrently.
- Grants: provider-held records only (no bearer tokens). The only authority scope is `core-test`. Rights are defined only for `core-test` and `core.events.read`. Effects and reconciliation are not implemented (M2+/M3).
- Events: one stream per data directory with a random ID. Retention and new epochs happen only through the launch configuration. Subscriptions live only as long as the session and are re-authorized before each delivery (E-SUB-REAUTH).
- Capabilities: the only predicate is `core-test.writes`, and only `core-test.subject.put` depends on it. Its status changes only between starts.
- `core.digest-sha512` is implemented, and sha512 digests are accepted only in sessions that negotiated it (D-SHA512).
- No extension is understood. Any extension key listed in `requires` is refused, and optional extensions are dropped (`unknown_extensions: "drop"`).
- `core-test/1` is always exposed, because this provider only runs under the conformance launch configuration (D-TESTPROFILE).
- Durability relies on SQLite with `synchronous=FULL`. Crash-consistency has not been exercised beyond the suite's clean restarts.
