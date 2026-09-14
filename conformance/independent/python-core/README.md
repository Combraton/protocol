# Independent Python Core and Execution provider

> **Status: conformance evidence for Protocol 0.1 (milestones M2 and M3), not a product.** It exists to test the fixtures and the reference provider for assumptions they share.

`combraton-independent-python-core` `0.1.0-dev.0` is a second implementation of the Core provider and of an executor. It speaks the stdio form of the stream binding and implements `core/1` together with the conformance-only `core-test/1` profile, including the Core features `core.grants` (CORE §15), `core.events` (§16), `core.capabilities` (§17) and `core.effects` (§19), and `core.authenticate` as a stdio session answers it (§18). Since the M3 pass it also implements `execution/1` (EXECUTION §1–§14) over the scripted executor of decision 007, with the clock file and store faults. Its value is that it was written **from the published documents only**, without reading the reference provider. Where the documents leave a question open, it records the question instead of copying the reference's answer.

It was written in five spec-only passes:

1. the Core command path (M1 fixtures, base `f42d21a`);
2. the three M2 features (base `3b32037`);
3. bringing those features level with the resolved documents (base `6c64ae4`);
4. Core effects, `execution/1` and the test controls (M3, base `d81e49b`);
5. realignment with the resolution of that pass's findings (base `ad91182`), a final alignment (base `c3e79d6`), the acceptance corrections C1–C3 (base `2d8402a`), and exit causal order (base `25f9c0b`).

The later passes also read the resolution records [M2-DIVERGENCES](../../../docs/work/release-0.1/M2-DIVERGENCES.md) and, in the fifth pass, [M3-DIVERGENCES](../../../docs/work/release-0.1/M3-DIVERGENCES.md). The third to fifth passes read the documents before the fixtures.

## What it was written from

Allowed and read:

- `docs/spec/profiles/CORE.md`, `docs/spec/profiles/EXECUTION.md` (fourth pass), `docs/spec/bindings/STREAM.md`, `docs/spec/bindings/ENCODING.md`
- `docs/decisions/007-execution-test-controls.md`, `docs/work/release-0.1/M3.md` and `docs/work/release-0.1/MATRIX.md` (fourth pass)
- `schemas/**`
- `conformance/README.md` (including its launch configuration section), `docs/VERIFICATION.md`, `conformance/schemas/fixture.schema.json`, `conformance/schemas/launch-config.schema.json`
- `docs/work/release-0.1/M2-DIVERGENCES.md` (second to fifth passes) and `docs/work/release-0.1/M3-DIVERGENCES.md` (fifth pass)
- `conformance/fixtures/**`, `conformance/vectors/encoding.json`
- `conformance/participants/*.json`, used only as format examples for the descriptor
- the transcripts and manifests the runner wrote for this implementation

Not read: `conformance/reference/**`, `conformance/crosscheck/**` and `conformance/runner/src/**`, nor their history. The runner was used only as a black-box binary.

In the first two passes the fixtures were read **before** the code was written, so some choices on points the prose leaves open were informed by what the fixtures expect. [DIVERGENCES.md](DIVERGENCES.md) marks each such choice. The third to fifth passes wrote their change lists from the documents first and read the fixtures afterwards (sections F, G and G.5). The fourth pass committed its implementation before opening any `execution/`, `socket/` or M3 Core fixture; the fifth read the five re-versioned fixtures only after its first run.

## Layout

| File | Contents |
|---|---|
| `provider.py` | Entry point: stream framing (STREAM 1–5), JSON-RPC mapping, the CORE 10 processing order, negotiation (including the Core features `execution/1` requires), authorization (step 6), capability checks (step 7), store faults, subscriptions, the idle re-check thread, and the Core and core-test operations |
| `executor.py` | Core effects and `execution/1`: effect records, attempts and obligations; admission, deliveries, completions, cancellation, reconciliation, the five timeouts and restart recovery; the optional features; and the scripted executor that interprets the launch configuration's `executor` scripts |
| `execution_envelope.py` | Closed-object validation of the M3 operations, gating submit members on negotiated features |
| `clock.py` | The provider clock: fixed, clock file (decision 007) or system clock |
| `errors.py` | The symbolic protocol error |
| `valuedomain.py` | Strict iterative parser for the value domain, RFC 8785 canonical form for integer-only values, digests, and size/depth measurement |
| `envelope.py` | Hand-written closed-object and type validation that mirrors the Core schemas (no JSON Schema library) |
| `grants.py` | Resource coverage, grant usability and delegation bounds (CORE 15) |
| `events.py` | Cursors and the ordered read of events, epoch changes and retention gaps (CORE 16.4) |
| `state.py` | Durable state in SQLite: subjects (including grants, executions, effects and the controller lease), bound command records, deduplication window, event stream, epochs, retention snapshot, capability snapshot, output spools |
| `tests/check_vectors.py` | Checks `valuedomain.py` against `conformance/vectors/encoding.json` |
| `tests/probe_provider.py` | Probes M1 behavior the fixtures do not exercise; each probe is tagged with its DIVERGENCES entry |
| `tests/probe_m2.py` | Probes grants, events and capabilities behavior the fixtures do not exercise (DIVERGENCES section E tags) |
| `tests/probe_f.py` | Probes the third-pass decisions: visibility, `filtered`, unvouched events, subscription ends, binding scopes, `core.authenticate` (section F tags) |
| `tests/fixture_sensitivity.py` | Applies one deliberate deviation at a time to a temporary copy and reports which fixtures notice (groups `m1`, `m2`, `f`; not extended to M3) |

It needs Python 3.12 or later and uses only the standard library.

## Running

From the repository root:

```sh
cargo build --workspace --locked
./target/debug/combraton-conformance run --participant conformance/participants/independent-python-core.json --out conformance/results/independent-python-core
python3 conformance/independent/python-core/tests/check_vectors.py
python3 conformance/independent/python-core/tests/probe_provider.py
python3 conformance/independent/python-core/tests/probe_m2.py
python3 conformance/independent/python-core/tests/probe_f.py
python3 conformance/independent/python-core/tests/fixture_sensitivity.py f
```

To run the provider by hand:

```sh
python3 conformance/independent/python-core/provider.py --data-dir DIR --config FILE
```

`FILE` is a launch configuration such as:

```json
{"format": "combraton-conformance-config/1", "principal": "agent-1", "authority_principals": ["owner"],
 "limits": {"max_depth": 6}, "dedupe": {"advance_on_start": 2, "retain_generations": 1},
 "events": {"new_epoch_on_start": true, "unvouched_last": 1, "retain_last": 10},
 "capabilities": {"core-test.writes": "unsupported"}, "clock": {"file": "work/clock"},
 "executor": {"adapter": {"enforcement": "mediated", "steering": "live"}, "capacity": 1,
              "scripts": {"e-1": [{"deliver": "provider_ack_id"}, {"runtime": "active"}, {"exit": {"code": 0}}]}},
 "faults": {"commit_unavailable": [{"operation": "execution.submit", "times": 1}]}}
```

The keys follow the conformance README's launch configuration table:

- `principal` (a string) is the session principal and names its deduplication scope. `authority_principals` defaults to `[principal]`; `provider_id` defaults to `conformance-provider`.
- `limits` partially overrides the defaults (frame 1 MiB, payload 262,144 bytes, string 65,536 bytes, array 1,024 items, depth 32).
- On each start, `dedupe.advance_on_start` moves `current` forward by that many generations, and `dedupe.retain_generations` keeps that many generations, counting `current`.
- On each start, `events.new_epoch_on_start` closes the current stream epoch, vouched through its last sequence minus `events.unvouched_last` (default 0). Events after that position leave the stream; subject state is unchanged. `events.retain_last` then discards all but the newest N stream events, folding them into the snapshot that retention gaps report. A new epoch also tells restart recovery that the journal is not intact.
- `capabilities` sets the status of `core-test.writes`. Without it the predicate is `supported` on the evidence of the start's store write. Adapter predicates from `executor.adapter.predicates` are listed beside it.
- `clock.fixed` fixes the provider clock. `clock.file` names a file holding one instant, read at start (a missing or malformed file exits with status 2) and on every request and idle re-check; a missing, malformed or earlier instant keeps the last good one. Otherwise the system clock (UTC, seconds) is used.
- `executor` is the scripted executor: adapter enforcement, echo proof, predicates, steering, enforced bounds and context boundaries; per-execution `scripts` and a `default_script`; `recovery_policy`, `host_id`, `capacity`, `budget_pools`, `context_packets`, `installations` and `output_spool_bytes`. Every script step of the launch-configuration schema is accepted; EXECUTION §15.1 fixes their meaning under conformance; DIVERGENCES G.3 and G.5 record the points it leaves open.
- `faults` makes the next owner transactions of an operation roll back and answer `unavailable`, or commit and answer `internal_error`.

Unknown configuration keys, `credentials` (a socket-binding key), the backpressure bounds under `events`, enabled `test_barriers`, unknown capability names and ill-typed values stop the provider with exit status 2. Where the README leaves a meaning open, DIVERGENCES D-TESTCTL, E-START-ORDER, F-UNVOUCHED-CONFIG and section G.3 record the choice.

## Result

The first complete run of the M1 pass passed all 57 fixtures then in the suite. No behavior was changed in response to a fixture failure.

The first complete run of the M2 pass, with `core.grants`, `core.events` and `core.capabilities` claimed, passed 107 of 108 fixtures. The failure, `core.events.retention-gap-returns-snapshot`, is a disagreement between that fixture and CORE §16.4 as this implementation reads it (DIVERGENCES E-GAP-TO). It was left failing rather than worked around.

The third pass, against the documents at `6c64ae4`, passed every applicable fixture on its first complete run: 138 pass, and the 6 `socket.*` fixtures are not applicable to a stdio participant. No behavior was changed in response to a fixture failure.

The fourth pass, against the documents at `d81e49b`, claims `execution/1`, `core.effects`, nine Execution features and the `clock.file`, `executor.script` and `store.faults` test controls. Of 211 fixtures:

| Run | pass | fail | timeout | harness_error | unsupported | skipped |
|---|---|---|---|---|---|---|
| First complete run | 176 | 19 | 0 | 0 | 4 | 12 |
| Final run | 181 | 14 | 0 | 0 | 4 | 12 |

Two genuine misreadings were fixed after the first run (DIVERGENCES G-EXIT-RUNTIME, G-COALESCE-ADJACENT). The 14 remaining failures were Execution fixtures that expected names or behavior the documents did not state, or in one case contradicted (DIVERGENCES G.2); they were left failing.

The Protocol session resolved those findings at `ad91182`: the conventions the fixtures relied on are now EXECUTION §15.1, and the recovery event defect was fixed in the reference and the fixtures. The fifth pass rebased onto that commit and followed the resolved documents (DIVERGENCES G.5):

| Run (fifth pass) | pass | fail | timeout | harness_error | unsupported | skipped |
|---|---|---|---|---|---|---|
| First run and two repeats | 195 | 0 | 0 | 0 | 4 | 12 |

Every applicable fixture passed, and `run` exited 0. G.5 recorded the points still underspecified. The Protocol session resolved them at `c3e79d6`. At that base a `stale_dispatch` from a generation never issued is fenced, the one change of the final alignment (DIVERGENCES G.6):

| Run (final alignment, base `c3e79d6`) | pass | fail | timeout | harness_error | unsupported | skipped |
|---|---|---|---|---|---|---|
| Final run and one repeat | 196 | 0 | 0 | 0 | 4 | 12 |

The owner's acceptance corrections at `2d8402a` added runtime `not_started` for executions that have not begun (C1), restated general dispatch, evidence, fencing, ordering and obligation rules (C3), and tightened backpressure timing (C2). After the corresponding changes (DIVERGENCES G.7):

| Run (base `2d8402a`) | pass | fail | timeout | harness_error | unsupported | skipped |
|---|---|---|---|---|---|---|
| Before the changes | 193 | 3 | 0 | 0 | 4 | 12 |
| After the changes, and one repeat | 196 | 0 | 0 | 0 | 4 | 12 |

The Protocol session resolved those points at `25f9c0b`. There an observed exit precedes the runtime change it causes, which is the one change of DIVERGENCES G.8:

| Run (base `25f9c0b`) | pass | fail | timeout | harness_error | unsupported | skipped |
|---|---|---|---|---|---|---|
| Before the change | 194 | 2 | 0 | 0 | 4 | 12 |
| After the change, and one repeat | 195 | 1 | 0 | 0 | 4 | 12 |

The remaining failure, `execution.recovery-revalidates-before-dispatch` version 2, expects `execution.inspect` to list a satisfied obligation. EXECUTION §4 says inspect returns open obligations, so it is left failing (G8-INSPECT-OBLIGATIONS) and `run` exits 1. The two clock-file fixtures that were coverage limits before this pass, `core.events.subscription-ends-at-grant-expiry` and `core.grants.test-clock-never-moves-backward`, now run and pass. The four backpressure fixtures remain `unsupported`, and the 12 `socket.*` fixtures are skipped.

Passing is weaker evidence than it looks. `tests/fixture_sensitivity.py` shows which deliberate deviations from the documents still pass every fixture; see DIVERGENCES sections D, E.4 and F.5. It has not been extended to the M3 fixtures.

## Limits

- Stdio binding only; there is no Unix-socket form and no credential store. `core.authenticate` always answers `already_authenticated` (CORE §18.2). Requests on a connection are processed one at a time under one lock, which an idle re-check thread also takes every 100 ms; notifications are sent only after the response to the request being processed.
- One principal per process, taken from the launch configuration. Cross-principal behavior (grants held by others, per-principal deduplication) is exercised across restarts over the same data directory, never concurrently.
- Grants: provider-held records only (no bearer tokens). The tracked authority scopes are `core-test` and `execution.controller:<host id>`. Rights are defined for `core-test`, `core.events.read`, `core.effects.abort_obligation` and the Execution operations.
- Events: one stream per data directory with a random ID. Retention, new epochs and unvouched events happen only through the launch configuration. Subscriptions live only as long as the session; they are re-authorized after every request and end with a final notification when their grant stops authorizing or an item cannot fit the caller's receive limit. An expiry alone is reported at the next request. `core.events.backpressure` is not implemented. It therefore makes no backpressure guarantee (CORE §16.5): the stdio writer blocks when the caller stops reading, with no room deadline and no closure (DIVERGENCES G.7, G.8).
- Capabilities: `core-test.writes` and the configured adapter predicates; only `core-test.subject.put` depends on a capability. Statuses change only between starts.
- Effects: records are never discarded, so `effect_history_unavailable` never occurs. Effects exist only for Execution operations; every M1 and M2 command returns `effect_refs: []`.
- Execution: one scripted host. The harness is the launch configuration's script, interpreted by the conformance executor conventions of EXECUTION §15.1; there is no real adapter, no inline brief and no barrier or signal. A scripted crash exits with status 1 between requests.
- `core.digest-sha512` is implemented, and sha512 digests are accepted only in sessions that negotiated it (D-SHA512).
- No extension is understood. Any extension key listed in `requires` is refused, and optional extensions are dropped (`unknown_extensions: "drop"`).
- `core-test/1` is always exposed, because this provider only runs under the conformance launch configuration (D-TESTPROFILE).
- Durability relies on SQLite with `synchronous=FULL`. Crash consistency has been exercised only through scripted crashes and SIGKILL restarts.
