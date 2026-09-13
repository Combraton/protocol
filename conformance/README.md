# Conformance suite

> **Status: under construction for Protocol 0.1 milestone M1.** Nothing here is a released conformance claim. Design: [decision 001](../docs/decisions/001-conformance-suite-architecture.md). Plan: [release plan](../docs/work/release-0.1/PLAN.md).

This directory will hold the normative, language-neutral conformance material for Protocol:

| Path | Contents |
|---|---|
| `fixtures/` | Declarative JSON fixtures: scripted exchanges with expected outcomes, requirement IDs and the mutants each negative fixture must fail |
| `vectors/` | Encoding and digest test vectors |
| `schemas/` | Schemas for fixture files, participant descriptors, launch configuration and result manifests |
| `participants/` | Descriptors that tell the runner how to launch an implementation under test |
| `runner/` | The black-box runner (Rust crate) |
| `reference/` | The reference provider and its mutants (Rust crate; does not depend on the runner) |
| `crosscheck/` | Independent non-Rust checks of the encoding vectors (Python `rfc8785`, Node `canonicalize`) |

The runner reaches an implementation only through the published [transport binding](../docs/spec/bindings/STREAM.md). It launches providers with a data directory and a launch configuration. Restarts and retention changes happen through process lifecycle and configuration. Domain state is created only through real protocol operations. There is no hidden control endpoint.

Reproducible commands will be recorded in [VERIFICATION](../docs/VERIFICATION.md) when the runner exists.
