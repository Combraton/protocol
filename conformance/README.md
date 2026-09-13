# Conformance suite

> **Status: draft for Protocol 0.1, milestone M1 (Core command path, stdio binding).** Nothing here is a released conformance claim. Design: [decision 001](../docs/decisions/001-conformance-suite-architecture.md). Plan: [release plan](../docs/work/release-0.1/PLAN.md). Commands: [VERIFICATION](../docs/VERIFICATION.md).

This directory holds the normative, language-neutral conformance material for Protocol:

| Path | Contents |
|---|---|
| `fixtures/` | Declarative JSON fixtures: scripted exchanges with expected outcomes, requirement IDs and the mutants each fixture must fail. 153 fixtures: `stream/` for the binding, `core/` for Core (grants, events and capabilities fixtures are M2), `socket/` for the Unix-socket binding. |
| `vectors/` | Encoding and digest test vectors |
| `schemas/` | Schema for fixture files |
| `participants/` | Descriptors telling the runner how to launch an implementation under test |
| `runner/` | The black-box runner (Rust crate `combraton-conformance`) |
| `reference/` | The reference provider and its 78 mutants (Rust crate `combraton-reference-provider`; does not depend on the runner; not a product) |
| `independent/python-core/` | Independent Core provider in Python written from the documents only, with its divergence log (M2) |
| `crosscheck/` | Independent non-Rust checks of the encoding vectors (Python `rfc8785`, Node `canonicalize`) |

## How it works

The runner reaches an implementation only through the published [stream binding](../docs/spec/bindings/STREAM.md). It validates every frame it receives against the schemas, as well as the fixture's expectations. For every error it checks the symbolic code, the JSON-RPC numeric code and the retry class.

It launches the provider with a data directory and a launch configuration file. Restarts and deduplication retention changes happen through process lifecycle and that configuration. Domain state — subjects, commands, epochs — is created only through real protocol operations. There is no control endpoint.

## Writing a fixture

A fixture is a JSON file conforming to `schemas/fixture.schema.json`. The common step types:

| Step | Effect |
|---|---|
| `start`, `stop` | Launch the provider (optionally with configuration overrides); end its input and require it to close and exit |
| `negotiate`, `describe` | Core session steps. Negotiation stores `negotiation` and `generation` variables. |
| `command`, `query` | Build a Core envelope with defaults. Use `set` and `remove` to override fields. The command digest is computed unless `digest` gives a literal. |
| `request`, `notify`, `raw` | Send an arbitrary request, a notification, or raw, padded or unterminated bytes |
| `expect_frame`, `expect_close`, `close_input` | Frame-level expectations |

Expectations are `{"ok": pattern}`, `{"error": code, "details": pattern}`, or `{"any_of": [expectation, …]}` for behaviors the spec leaves to the provider. Object patterns match subsets of members. Array patterns match exactly, in length and order; use `$contains` for membership. Patterns support `$var`, `$ne_var`, `$type`, `$contains`, `$len`, `$absent`, `$any` and `$exact`. `capture` stores JSON-pointer values from a response for later steps. Values the runner sends may use `{"$var": name}`, `{"$unique": prefix}` and `{"$repeat": [text, count]}`, which expands to `text` repeated `count` times. Every frame a participant sends must fit the caller's receive limit: 1 MiB, or the `receive_limits.max_frame_bytes` of the session's successful negotiation.

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

At each start the keys apply in this order: `dedupe`, `events.new_epoch_on_start`, `events.retain_last` (counted across all epochs), then `capabilities`, so a capability change event recorded at start is never discarded by the same start.

Keys a participant does not support make it unable to run fixtures that use them. It should refuse to start (nonzero exit) rather than silently ignore them.

**Socket participants.** For `binding: unix`, the runner creates a `0700` socket directory for each launch. It writes deterministic per-run credentials into the launch configuration and authenticates each session unless a step sets `auto_authenticate: false`. Transcripts therefore contain these synthetic test credentials; never point the runner at a provider holding real credentials.
