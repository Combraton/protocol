# Conformance suite

> **Status: draft for Protocol 0.1, milestones M1–M2 (Core command path, grants, events, capabilities; stdio and Unix-socket bindings).** Nothing here is a released conformance claim. Design: [decision 001](../docs/decisions/001-conformance-suite-architecture.md). Plan: [release plan](../docs/work/release-0.1/PLAN.md). Commands: [VERIFICATION](../docs/VERIFICATION.md).

This directory holds the normative, language-neutral conformance material for Protocol:

| Path | Contents |
|---|---|
| `fixtures/` | Declarative JSON fixtures: scripted exchanges with expected outcomes, requirement IDs and the mutants each fixture must fail. 155 fixtures: `stream/` for the binding, `core/` for Core (grants, events and capabilities fixtures are M2), `socket/` for the Unix-socket binding. |
| `vectors/` | Encoding and digest test vectors |
| `schemas/` | Schema for fixture files |
| `participants/` | Descriptors telling the runner how to launch an implementation under test |
| `runner/` | The black-box runner (Rust crate `combraton-conformance`) |
| `reference/` | The reference provider and its 147 mutants (Rust crate `combraton-reference-provider`; does not depend on the runner; not a product). Its `tests/` hold implementation-specific checks, such as constructed cursors, that portable fixtures must not rely on. |
| `scripts/repeat_fixture.py` | Runs one fixture repeatedly; every run must pass on the provider, or fail at the expected step and reason on the mutant (deterministic race evidence) |
| `scripts/peer_user_check.py` | Different-OS-user check for the Unix-socket binding, run as root through passwordless `sudo` (CI) |
| `independent/python-core/` | Independent Core provider in Python written from the documents only, with its divergence log (M2) |
| `crosscheck/` | Independent non-Rust checks of the encoding vectors (Python `rfc8785`, Node `canonicalize`) |

## How it works

The runner reaches an implementation only through the published [stream binding](../docs/spec/bindings/STREAM.md). It validates every frame it receives against the schemas, as well as the fixture's expectations. For every error it checks the symbolic code, the JSON-RPC numeric code and the retry class.

It launches the provider with a data directory and a launch configuration file. Restarts and deduplication retention changes happen through process lifecycle and that configuration. Domain state — subjects, commands, epochs — is created only through real protocol operations. There is no control endpoint.

## Outcomes and result files

`run` writes `manifest.json` (fixture digests, participant, environment, per-fixture outcome and reason) and `transcripts/<fixture>.jsonl`, plus `transcripts/<fixture>.stderr.log` when the participant wrote to standard error. `check-mutants` writes one manifest per mutant under `mutants/<name>/` and a `mutants.json` summary of every mutant-fixture outcome. Per-fixture outcomes are kept distinct:

| Outcome | Meaning | Counts as passing |
|---|---|---|
| `pass` | Every step matched | yes |
| `fail` | A step did not match, or the participant broke the binding | no |
| `timeout` | An expected frame or exit did not arrive in time | no |
| `harness_error` | The runner or fixture could not run the case | no |
| `unsupported` | The participant does not claim a profile or feature the fixture needs | not run |
| `skipped` | The fixture is written for another transport binding | not run |

A mutant is killed only by `fail` or `timeout`.
- **Intended reasons.** A fixture may state `kill_expectations`: for a mutant, the step where it must fail and a substring of the reason. `check-mutants` reports a mutant that fails elsewhere as `WRONG-REASON`. `mutants.json` records every failing step, reason and expectation.
- **Coverage limits.** A fixture that requires a test control or barrier the participant does not declare is `unsupported`, and the manifest lists it under `coverage_limits`. It is never a pass.
- **Upload.** The CI workflow uploads `conformance/results/` from every job, whatever the outcome.

## Writing a fixture

A fixture is a JSON file conforming to `schemas/fixture.schema.json`. The common step types:

| Step | Effect |
|---|---|
| `start`, `stop` | Launch the provider (optionally with configuration overrides); end its input and require it to close and exit |
| `negotiate`, `describe` | Core session steps. Negotiation stores `negotiation` and `generation` variables. |
| `command`, `query` | Build a Core envelope with defaults. Use `set` and `remove` to override fields. The command digest is computed unless `digest` gives a literal. |
| `request`, `notify`, `raw` | Send an arbitrary request, a notification, or raw, padded or unterminated bytes |
| `expect_frame`, `expect_close`, `close_input` | Frame-level expectations |
| `command` or `query` with `"await": false` and `name`; `expect_response` | Pipelined send, and a later check of the named response in any arrival order ([decision 007](../docs/decisions/007-execution-test-controls.md)). Pipelining does not establish commit order. |
| `set_clock` | Replace the controlled clock file atomically with `instant` (forward only unless `allow_backward`) or `raw` content |
| `kill` | SIGKILL the provider process; a later `start` relaunches it over the same data directory |
| `expect_signal_gap` | Check the time between two signals from their files' modification times: `min_ms <= to - from <= max_ms`. Used to verify declared time bounds. |
| `pause_reading`, `resume_reading` | Stop and restart reading a session's connection, so its buffers fill as for a consumer that stopped reading (TRN-4). |
| `collect_until_close` | Read everything a session still delivers until the connection closes, under a real-time bound. The named subscription's event sequences must be contiguous, only its last notification may end it (`forbid_ended` forbids any ending), and `expect_last` matches that last notification. Captures the last `next_cursor` (`capture_cursor`) and the sequence after the last event (`capture_next_sequence`). |
| `query` with `eventually_ms` | Repeat the query until its expectation holds, within a real-time bound; for asynchronous compositions where one participant acts after another's worker finishes. Only the last mismatch is reported. |
| `start_participant`, `stop_participant`, `kill_participant`; `connect` with `participant` | Compositions (Unix-socket binding only, owner decision M4-Q3). Each named participant runs as its own process of the participant under test, with its own data directory, configuration and a stable socket (variable `socket.<name>`), and the shared controlled clock. Sessions reach it with `connect`, `participant` and `principal`. `credential_principals` lists the principals, including peer participants, that may authenticate to it (variables `credential.<principal>`). A stopped or killed participant restarts over the same data directory. Every named participant runs the same descriptor; mixed implementations are not composed yet. |
| `await_barrier`, `release_barrier`, `await_any` | Implementation-specific barriers and signals: wait for a pause point (then clear old signals), release it, or wait until a signal exists or every named response arrived. Real-time bounds; expiry fails the case. |

Expectations are `{"ok": pattern}`, `{"error": code, "details": pattern}`, or `{"any_of": [expectation, …]}` for behaviors the spec leaves to the provider. Object patterns match subsets of members. Array patterns match exactly, in length and order; use `$contains` for membership. Patterns support `$var`, `$ne_var`, `$type`, `$contains`, `$len`, `$absent`, `$any`, `$exact` and `$sha256_base64` (the sha256 digest of the decoded bytes matches the argument pattern, typically a captured digest). `capture` stores JSON-pointer values from a response for later steps. Values the runner sends may use `{"$var": name}`, `{"$unique": prefix}` and `{"$repeat": [text, count]}`, which expands to `text` repeated `count` times. Every frame a participant sends must fit the caller's receive limit: 1 MiB, or the `receive_limits.max_frame_bytes` of the session's successful negotiation.

Negative fixtures exercise invalid, hostile or out-of-order input. Each must list in `kills` at least one reference mutant it fails, and `check-mutants` enforces this. Changing what a fixture means requires incrementing its `version`.

The M1 fixtures were first authored with a throwaway script and are maintained as data from now on.

## Launch configuration

A participant under test is launched with a data directory and a JSON launch configuration file. The file is test environment, not product configuration or protocol ([CORE §13.1](../docs/spec/profiles/CORE.md#131-test-control-is-environment-only)). Its schema is `schemas/launch-config.schema.json`.

| Key | Meaning |
|---|---|
| `format` | `combraton-conformance-config/1` |
| `principal` | Session principal for this launch (a string) |
| `authority_principals` | Authority principals (CORE §15.1); defaults to `[principal]` |
| `provider_id` | Provider identity used as grant audience; defaults to `conformance-provider` |
| `limits` | Partial override of the provider's receive limits (CORE §9) |
| `dedupe.advance_on_start` | At this start, `current += N` |
| `dedupe.retain_generations` | At this start, `oldest_retained = max(oldest_retained, current − R + 1)`, discarding records filed under older generations |
| `events.new_epoch_on_start` | At this start, begin a new stream epoch whose previous epoch is vouched through its last sequence (CORE §16.1) |
| `events.unvouched_last` | With `new_epoch_on_start`, vouch for the previous epoch only through its last sequence minus this many events (default 0; no effect without `new_epoch_on_start`; a value above the last sequence vouches through 0). The unvouched events leave the stream: they are never delivered and do not count toward a later `retain_last`. Subject state is unchanged; only the vouched position moves. |
| `events.retain_last` | At this start, discard all but the newest N events (CORE §16.4) |
| `capabilities` | Map of capability name to status, such as `{"core-test.writes": "unsupported"}` (CORE §17.4) |
| `clock.fixed` | Fixed provider clock instant `YYYY-MM-DDTHH:MM:SSZ` (CORE §15.2) |
| `clock.file` | Path of a clock file holding one instant; the provider reads protocol-visible time from it whenever needed. A missing or malformed file refuses the start. During a run, a missing, malformed or backward instant keeps the last good one. Fixtures write `clock: {"controlled": instant}` (or `controlled_raw` to test refusal) and the runner supplies the path. Participants declare support under `claims.test_controls: ["clock.file"]`. |
| `executor` | Scripted executor test adapter for `execution/1` (decision 007, owner decision Q4). `adapter` sets `enforcement`, `echo_proves_delivery` and `predicates`. `recovery_policy` is `resume` (default) or `terminate`. `scripts` maps an execution ID to a list of one-key steps, and `default_script` applies otherwise. The steps are `deliver` (a proof class), `crash` (`before_dispatch` or `after_write`, which exits the process), `reconcile_finds`, `runtime`, `host_restart`, `complete` (`completion_id`, `content`, optional `generation`), `exit`, `on_cancel`, `wait_until`, `wait_for` (`cancel`, `steer` or an answered `action`), `stale_dispatch` (`generation`: a dispatcher from that host generation tries to send) and `stall`. Optional-feature steps: `steer_deliver` (a proof class), `steer_behavior`, `request_action` (`action_id`, `owner`), `workspace` (`head`, `dirty_paths`, `untracked_paths`, `probes`), `agent_reports_commit`, `usage` (`invocation_id`, `basis`, `measure`, `amount`), `context_delivery` (`binding_id`, `boundary`, `harness`), `transition`, `transport_errors` (the next effect attempts end with unknown outcomes) `probe_status` (a read-class status probe) and `observe_basis` (the execution's observed repositories and environment change). Executor settings: `host_id`, `capacity`, `budget_pools` (`measure`, `limit`), `context_packets` (`ref` and `digest`, or a packet `reference`), `installations` for discovery, `observed_basis` (what revalidation can observe), `peers` (providers reached over their sockets to fetch packets) and `evidence_outputs` (the evidence provider for completion outputs); adapter `steering`, `enforced_bounds` and `context_boundaries`. A process restart runs recovery (EXECUTION §7.1); `events.new_epoch_on_start` makes the journal not intact for that recovery. Steps advance no later than the provider's next request or idle re-check. Participants declare support under `claims.test_controls: ["executor.script"]`. Normative only for the conformance tests that use it. |
| `context` | Scripted context preparation for `context/1` (CONTEXT §12): `scripts` maps the ID of the request that starts a job to one-key steps, `default_script` applies otherwise. Steps: `wait_until`, `investigate`, `section`, `coverage`, `unmet`, `omit`, `correction`, `conditions`, `publish`, `end` and `stall`. Packets are sealed as Evidence artifacts in the provider's own store. Participants declare `claims.test_controls: ["context.script"]`. Normative only for the conformance tests that use it. |
| `context.executor` | `{ provider_id, socket, credential, grant? }`: a separately running executor where the context provider runs its jobs' investigation executions (script step `execute`: `execution`, `brief?`, `depth`, `call_budget`). They carry `origin` and no context bindings. |
| `context.evidence_provider` | `{ provider_id, socket, credential, grant? }`: a separately running evidence provider where the context provider seals its packets, reached as an ordinary authenticated client under a grant issued there. Without it, packets are sealed in the provider's own store. |
| `evidence_store` | Scripted evidence store for `evidence/1` (decision 007): `corrupt` and `unavailable` list artifact IDs whose stored bytes fail integrity or cannot be served; `staging_timeout_seconds` abandons staged uploads with reason `staging_expired`; `deletion_delay_seconds` confirms physical deletion that long after a purge request, on the controlled clock. Holds and purges run through protocol operations. Participants declare `claims.test_controls: ["evidence.store"]`. Normative only for the conformance tests that use it. |
| `faults` | Injected store faults (decision 007): `commit_unavailable` and `response_internal_error`, each a list of `{ "operation", "times" }`. The first makes the next owner transactions for that operation roll back and answer `unavailable`; the second commits them and answers `internal_error`. Participants declare `claims.test_controls: ["store.faults"]`. |
| `test_barriers` | `{ "directory", "enabled" }`: implementation-specific barriers to pause at once each; the start step's `barriers` sets it. Participants declare names under `claims.test_barriers`; the reference declares `subscription.recheck.after_authorization` and `session.closed`, and emits signals `processing.lock.contended` (a request found the processing lock held), `session.closed` (a Unix-socket session ended and its provider state was released), `backpressure.stall.started` (output would exceed the bound and the provider waits for room), `backpressure.limit.reached` (a consumer made no room within the bound) and `backpressure.connection.closed` (the connection was closed for backpressure). Both reference participants declare the backpressure signals. A fixture that waits for a signal lists it under `requires_barriers`. |

At each start the keys apply in this order: `dedupe`, `events.new_epoch_on_start`, `events.retain_last` (counted across all epochs), then `capabilities`, so a capability change event recorded at start is never discarded by the same start.

Keys a participant does not support make it unable to run fixtures that use them. It should refuse to start (nonzero exit) rather than silently ignore them.

**Socket participants.** For `binding: unix`, the runner creates a `0700` socket directory for each launch. It writes deterministic per-run credentials into the launch configuration and authenticates each session unless a step sets `auto_authenticate: false`. Transcripts therefore contain these synthetic test credentials; never point the runner at a provider holding real credentials.
