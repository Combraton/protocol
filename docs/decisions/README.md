# Decision records

This directory owns PROTOCOL-local implementation decisions. The accepted architecture is in [the baseline](https://github.com/Combraton/combraton/blob/main/docs/architecture/BASELINE.md). [ADR 001](https://github.com/Combraton/combraton/blob/main/docs/decisions/001-standalone-first-and-evaluation.md) records the accepted standalone-first build/client/evaluation direction. Concrete SDK/model/renderer selections remain unresolved.

Use one small file per meaningful decision. Include title, status (proposed/accepted/superseded), date, owner/authority, concrete problem, affected contracts, alternatives, selected choice, primary evidence or experiment, consequences, verification and superseded sections.

## Index

Records 001–005 were accepted by the owner on 2026-09-13 for the Protocol 0.1 release ([tracking issue #1](https://github.com/Combraton/protocol/issues/1)); decision 006 settles the Unix-socket credential. Decision 007 was accepted with refinements on 2026-09-14 for M3.

| Record | Status | Subject |
|---|---|---|
| [001](001-conformance-suite-architecture.md) | accepted | Fixtures as data, black-box runner, environment-only test control, mutants, result manifest, Rust runner and reference with non-Rust cross-checks |
| [002](002-schema-language-and-extensibility.md) | accepted | JSON Schema 2020-12, closed objects, `extensions` and `requires` |
| [003](003-canonical-encoding-and-digests.md) | accepted | Integer-only I-JSON domain, RFC 8785 command digests, byte digests for content, `sha256:` strings |
| [004](004-local-stream-binding.md) | accepted | Bounded newline-delimited JSON-RPC over stdio and Unix sockets; close on frame-level failure |
| [005](005-deduplication-generations.md) | accepted | Command intent digest, binding on acceptance, deduplication generations instead of clocks |
| [006](006-unix-socket-principal-credential.md) | accepted | Unix-socket placement and peer check, `ccred1` credential files and `core.authenticate` |
| [007](007-execution-test-controls.md) | accepted with refinements | Execution test controls outside the protocol: scripted executor, clock file, process kills, barriers, pipelined sends |

Wire/compatibility decisions belong in Protocol; cross-system authority changes belong in Combraton. Link the owning decision instead of maintaining independent copies. An experiment result does not silently select a product direction. Keep ordinary local choices lightweight and record material selections in their implementation PR.
