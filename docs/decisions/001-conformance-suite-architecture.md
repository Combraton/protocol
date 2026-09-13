# 001: Conformance suite architecture

- **Status:** accepted on 2026-09-13. Owner approval on 2026-09-13 ("yes things looks good"), with the instruction to merge PR #2. Changes need a new versioned decision.
- **Date:** 2026-09-13.
- **Owner/authority:** Protocol session under the Protocol 0.1 kickoff; [tracking issue #1](https://github.com/Combraton/protocol/issues/1).
- **Affects:** every release fixture, the result manifest and how PIO, CBR and benchmarks run conformance. Matrix rows REL-5, REL-6, REL-8, CMP-4.
- **Supersedes:** nothing. [SPEC](../spec/SPEC.md) required normative fixtures and independent adversarial participants but selected no mechanism.

## Problem

Protocol owns normative fixtures for stateful services. Benchmarks runs them without redefining them. A fixture has to tell a correct implementation from a plausible wrong one. It must work for implementations in any language. It must reach states such as "provider restarted" or "deduplication history discarded" without inventing a backdoor into provider state. Tests written by the same author as the reference implementation can share its mistakes.

## Evidence consulted

A read-only research pass on 2026-09-13 inspected these sources at pinned commits:

| Source | What it shows |
|---|---|
| [Bowtie harness protocol](https://github.com/bowtie-json-schema/bowtie/blob/caa22a6f6e8e647b1ee8edca31978db396a7841f/docs/implementers.rst) | Per-implementation harnesses speak JSON lines on stdio. Expected results live in case data, not in harnesses. Sequence IDs are opaque. |
| [MCP conformance](https://github.com/modelcontextprotocol/conformance/blob/7169291ec0b68eb370fddcd9947313ab0d5e4156/README.md) | Server and client modes. Every wire message is validated against the schema. Deliberately broken servers carry expected failures ([negative tests](https://github.com/modelcontextprotocol/conformance/blob/7169291ec0b68eb370fddcd9947313ab0d5e4156/src/scenarios/server/negative.test.ts)). A missing test hook is reported as failure, not skip ([untestable](https://github.com/modelcontextprotocol/conformance/blob/7169291ec0b68eb370fddcd9947313ab0d5e4156/src/scenarios/untestable.ts)). |
| [gRPC interop descriptions](https://github.com/grpc/grpc/blob/b8f09d9168d856020d236bd32f39195f7b5aa2cf/doc/interop-test-descriptions.md) | Named cases with procedure and client/server assertions. A separate adversarial server for negative HTTP/2 cases. |
| [h2spec verifier](https://github.com/summerwind/h2spec/blob/af83a65f0b6273ef38bf778d400d98892e7653d8/spec/verifier.go) | Malformed frames with expected errors. Explicitly tolerates a set of legal failure behaviors. |
| [Autobahn case grading](https://github.com/crossbario/autobahn-testsuite/blob/b8a5120d905e30470e4475785c48e4cedc35f6cd/autobahntestsuite/autobahntestsuite/case/case.py) | Alternative expected traces graded OK or non-strict. The same cases test clients and servers. |
| [Maelstrom protocol](https://github.com/jepsen-io/maelstrom/blob/480a8197026db356cd999d4e74018964d19b399a/doc/protocol.md) | Black-box JSON-lines nodes. Definite versus indefinite errors. The harness owns process lifecycle and faults. |
| [Wycheproof formats](https://github.com/C2SP/wycheproof/blob/3fa63dd0344abb611f1fb1d77e119938603ea230/doc/formats.md) | Vectors tagged by the bug class they target, with valid/invalid/acceptable results. |
| [Kafka Trogdor](https://github.com/apache/kafka/blob/4d781f506a777c43eb93b359ed8a171e032d2df4/trogdor/README.md) and [W3C WebDriver security](https://w3c.github.io/webdriver/#security) | Cautionary: test-injection endpoints become unauthenticated authority paths unless separated and explicitly enabled. |

Inference drawn from these sources, not stated by any of them: once fixtures are data, the runner's language matters mainly for reuse and for avoiding shared libraries with the implementations under test.

## Decision

1. **Fixtures are declarative JSON data** with their own schema. Each carries:
   - a stable ID and integer version;
   - requirement IDs and spec anchors;
   - required profiles and features;
   - the participant role under test;
   - scripted steps: send, expect, raw bytes, session open/close, restart;
   - captures and matchers, including explicitly graded alternatives (`conformant`, `acceptable`);
   - the mutants the fixture must fail.

   Editing a fixture's meaning requires a new version.
2. **The runner is a black box.** It reaches implementations only through the published transport bindings. It validates every received message against the schemas as well as the fixture's expectations.
3. **Environment control is not a protocol surface.**
   - The runner owns the provider process. It launches the provider from a participant descriptor with a data directory and a configuration file: limits, deduplication retention policy, generation advancement on start.
   - Restart, disconnect and retention changes happen through process lifecycle and configuration.
   - Domain state (subjects, commands, epochs, grants) is reached only through real operations. The conformance-only `core-test/1` profile provides the minimum test subject ([CORE §13](../spec/profiles/CORE.md#13-conformance-only-test-profile-core-test1)).
   - A product endpoint refuses control method names.
   - If a later milestone needs an in-process hook, it gets its own decision record.
4. **Negative fixtures must bite.** The reference provider supports named mutants, each a deliberate violation. CI requires three things:
   - the reference passes every fixture;
   - each mutant fails at least one fixture that declares it;
   - each negative fixture declares at least one mutant it fails.
5. **Outcomes are explicit:** `pass`, `pass_acceptable`, `fail`, `untestable`, `not_applicable`, `timeout`, `harness_error`. `untestable` and `harness_error` never count as passing. `not_applicable` applies only to profiles or features the participant does not claim.
6. **The result manifest is a scoped claim.** It records:
   - suite version and fixture digests;
   - runner version;
   - participant identity and version;
   - negotiated profiles and features, with the describe snapshot;
   - environment, per-case outcome and transcript locations.

   A conformance claim is derived from the manifest, never asserted separately.
7. **The runner and reference provider are Rust; independence checks are not.**
   - Owner input on 2026-09-13: PIO, CBR and the control plane will be written in Rust.
   - The runner and reference provider are separate crates in one Cargo workspace, with a committed lockfile and pinned toolchain. Neither crate depends on the other.
   - The runner's strict parser must not rely on `serde_json` defaults. The research probe showed those defaults silently keep the last duplicate member and round integers above 2^53.
   - The reference provider's parser and canonical encoder are written separately from the runner's.
   - Encoding vectors are cross-checked in CI by two non-Rust implementations: Python `rfc8785` and Node `canonicalize`.
   - The spec-only independent implementation (M2) is written in a non-Rust language, so a mistake shared by Rust implementations remains detectable.

## Alternatives

- **Fixtures as code** (h2spec, Autobahn, MCP) is more expressive, but ties the suite to one language and is harder to review as a contract. An escape hatch for property checks can be added later if data proves insufficient.
- **Checker-driven randomized histories** (Maelstrom, etcd robustness) find unknown bugs but are nondeterministic. They belong in benchmarks as reliability scenarios, alongside normative scripted fixtures rather than replacing them.
- **In-band test operations for environment control** avoid a launcher but put test semantics inside product profiles, and still cannot express a restart.
- **A control socket served by the provider** (gofail/Trogdor style) reaches more states but creates exactly the hidden authority path the architecture forbids. Deferred unless a later fixture proves it unavoidable.
- **Python runner (first draft of this record).** Shares no libraries with Rust services, but adds a second toolchain to every consumer's CI and gives the Rust implementations no Rust exemplar. The independence it bought is kept instead by non-Rust vector cross-checks and a non-Rust M2 implementation.
- **TypeScript runner** matches MCP ergonomics but adds a toolchain no consumer uses.

## Consequences

- Providers must accept a data directory and a configuration file when launched for conformance. That is a small test-harness obligation, not a product API.
- Some states (clock expiry, crash mid-transaction) may be unreachable through lifecycle and configuration alone. They are reported as `untestable` until a separately decided mechanism exists; they are never silently skipped.
- Mutants share the reference's blind spots. The independent implementation and PIO/CBR review mitigate that.

## Verification

M1 CI runs the suite against the reference provider (all pass) and against every mutant (each fails its declared fixtures). A schema check validates every fixture and result manifest. Commands are recorded in [VERIFICATION](../VERIFICATION.md) when introduced.
