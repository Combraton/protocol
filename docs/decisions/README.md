# Decision records

This directory owns PROTOCOL-local implementation decisions. The accepted architecture is in [the baseline](https://github.com/Combraton/combraton/blob/main/docs/architecture/BASELINE.md). [ADR 001](https://github.com/Combraton/combraton/blob/main/docs/decisions/001-standalone-first-and-evaluation.md) records the accepted standalone-first build/client/evaluation direction. Concrete SDK/model/renderer selections remain unresolved.

Use one small file per meaningful decision. Include title, status (proposed/accepted/superseded), date, owner/authority, concrete problem, affected contracts, alternatives, selected choice, primary evidence or experiment, consequences, verification and superseded sections.

## Index

All current records are **proposed** for the Protocol 0.1 release ([tracking issue #1](https://github.com/Combraton/protocol/issues/1)). None is accepted by the owner yet.

| Record | Status | Subject |
|---|---|---|
| [001](001-conformance-suite-architecture.md) | proposed | Fixtures as data, black-box runner, environment-only test control, mutants, result manifest, Python runner |
| [002](002-schema-language-and-extensibility.md) | proposed | JSON Schema 2020-12, closed objects, `extensions` and `requires` |
| [003](003-canonical-encoding-and-digests.md) | proposed | Integer-only I-JSON domain, RFC 8785 command digests, byte digests for content, `sha256:` strings |
| [004](004-local-stream-binding.md) | proposed | Bounded newline-delimited JSON-RPC over stdio and Unix sockets; close on frame-level failure |
| [005](005-deduplication-generations.md) | proposed | Command intent digest, binding on acceptance, deduplication generations instead of clocks |

Wire/compatibility decisions belong in Protocol; cross-system authority changes belong in Combraton. Link the owning decision instead of maintaining independent copies. An experiment result does not silently select a product direction. Keep ordinary local choices lightweight and record material selections in their implementation PR.
