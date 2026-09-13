# Conformance suite

> **Status: draft for Protocol 0.1, milestone M1 (Core command path, stdio binding).** Nothing here is a released conformance claim. Design: [decision 001](../docs/decisions/001-conformance-suite-architecture.md). Plan: [release plan](../docs/work/release-0.1/PLAN.md). Commands: [VERIFICATION](../docs/VERIFICATION.md).

This directory holds the normative, language-neutral conformance material for Protocol:

| Path | Contents |
|---|---|
| `fixtures/` | Declarative JSON fixtures: scripted exchanges with expected outcomes, requirement IDs and the mutants each fixture must fail. 83 fixtures: `stream/` for the binding, `core/` for Core (grants and events fixtures are M2). |
| `vectors/` | Encoding and digest test vectors |
| `schemas/` | Schema for fixture files |
| `participants/` | Descriptors telling the runner how to launch an implementation under test |
| `runner/` | The black-box runner (Rust crate `combraton-conformance`) |
| `reference/` | The reference provider and its 56 mutants (Rust crate `combraton-reference-provider`; does not depend on the runner; not a product) |
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

Expectations are `{"ok": pattern}` or `{"error": code, "details": pattern}`. Patterns match subsets and support `$var`, `$ne_var`, `$type`, `$contains`, `$len`, `$absent`, `$any` and `$exact`. `capture` stores JSON-pointer values from a response for later steps.

Negative fixtures exercise invalid, hostile or out-of-order input. Each must list in `kills` at least one reference mutant it fails, and `check-mutants` enforces this. Changing what a fixture means requires incrementing its `version`.

The M1 fixtures were first authored with a throwaway script and are maintained as data from now on.
