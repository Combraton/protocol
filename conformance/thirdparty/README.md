# Third-party consumers: harness interface

> **Status: Protocol 0.1 release candidate, M6 (owner decision M6-Q2).** Test tooling, not protocol. Nothing in this file is a protocol operation, and no provider implements anything described here.

This directory holds minimal third-party implementations written by contracts-only authors: people who read only `docs/spec/**`, `schemas/**` and this file, and never the reference providers, the runner source, the cross-checks or the independent provider. Each implementation is a **client**: it serves no protocol operation and reaches providers over the Unix-socket binding ([STREAM §6](../../docs/spec/bindings/STREAM.md)), authenticating with `core.authenticate` (CORE §18).

The runner launches a client with the fixture step `start_client`, which renders a configuration file, and `conformance/thirdparty/<implementation>/participant.json`, whose `role` is `client`. The runner never reads the client's internal state. Everything a fixture checks about a client is observed through public protocol operations at the reference providers.

## Common rules for every client

- **Launch.** `argv` from `participant.json`, with `--config <file>`. With a mutant, `--mutant <name>` is appended.
- **Configuration.** One JSON file, described below. Credentials appear only in this file, never on the command line, in the environment or in any output (CORE §18.1).
- **Lifecycle.** A long-running client runs until its standard input reaches end of file, then exits with status 0. A one-shot client exits by itself: 0 on success, 1 when the work failed. Standard error is for diagnostics only; the runner saves it beside the transcript.
- **Protocol use:**
  - One session per provider per purpose, each over its own Unix-socket connection.
  - Each session authenticates, calls `core.feature_dependencies` (CORE §4.3), then negotiates. A session that carries a `grant` field also requests `core.grants` (CORE §5, §15); that is a use of the feature, not a dependency the query reports.
  - When the query answers `method_not_found`, the client uses the dependencies stated in the profile documents and still requests them. It never drops a required feature.
  - The client negotiates each profile with the provider that serves it, and relies only on what negotiation selected.
  - Commands use real command identity and digests (CORE §6, ENCODING).
  - References and digests are compared exactly: a digest a provider advertises is never trusted for bytes the client did not hash itself.
- **Clock.** A client given `clock_file` reads the current instant from that file (RFC 3339 UTC, as written by the runner) whenever it needs "now". It never uses its own clock for protocol decisions, and it re-reads the file at least every `poll_ms` milliseconds (default 25).
- **Grants.** The configuration names the grant for each use (`context`, `packet_evidence`, `records`, `evidence`). Every profile operation for that use carries that grant; `core.authenticate`, `core.feature_dependencies` and `core.negotiate` carry none (CORE §15).

Provider entries in a configuration have the form `{ "socket": path, "credential": string }`, keyed by `provider_id`.

- **Not durable.** Clients keep no journal. A fixture never restarts a client, and recovery after a client restart is outside these clients' claimed coverage.
- **Descriptor values** for artifacts a client publishes: `scope` `"thirdparty"`, `retention_class` `"standard"`, `capture` `{ captured_at: now, anchors: [] }` and `coverage` `{ completeness: "complete", covered: ["content"], gaps: [] }`. These are test values; EVIDENCE §3 leaves both scope and retention class to the provider and producer.

## `minimal-executor`: an independent execution kernel (S-A)

**Role.** An independent execution consumer, or kernel. It owns its dispatch boundary and dispatches deterministic fake executions. It is **not** an `execution/1` provider: it serves nothing and claims no Execution conformance. It consumes `context/1` with `context.claims` at the Context provider, and `evidence/1` at the Evidence providers named by packet references. It also publishes its own records as Evidence artifacts at a records provider.

**Configuration** (`format: "combraton-thirdparty-kernel-config/1"`):

| Member | Meaning |
|---|---|
| `principal` | The principal its credentials authenticate |
| `clock_file`, `poll_ms` | The controlled clock (above) |
| `providers` | `{ <provider_id>: { socket, credential } }` for every provider it may reach |
| `context` | `{ provider, grant }`: where it reads packet facts |
| `packet_evidence` | `{ <provider_id>: grant }`: the grant for fetching packet bytes at each Evidence provider a packet reference may name |
| `records` | `{ provider, grant }`: where it publishes check and dispatch records |
| `work` | The work list, below |

Each `work` item:

| Member | Meaning |
|---|---|
| `work_id` | Identifier, `[a-z0-9-]{1,48}` |
| `dispatch_at` | The instant from which the item's dispatch boundary is due |
| `binding.packet` | An exact packet reference `{ packet: { kind: "context.packet", id }, revision, artifact: { provider, artifact: { kind: "evidence.artifact", id }, digest } }` (CONTEXT §2) |
| `binding.obligation` | `required_before_start` or `advisory` |
| `binding.fallback` | For `advisory`: `proceed_with_gap` |

**Evaluating the dispatch boundary.** For each item not yet dispatched, whenever `now` is at or after `dispatch_at`, the kernel evaluates the binding. The state is the first rule that applies:

1. **Wrong bytes.** The SHA-256 digest of the exact bytes received differs from the reference's digest, or the Evidence provider refuses the fetch because the reference's digest is not the artifact's (`artifact_digest_mismatch`): state `unsatisfied`. Such bytes are never held (EXECUTION §13.1 "Fetch").
2. **Unreadable.** The packet bytes cannot be fetched for any other reason, such as an unreachable provider or unavailable bytes; or the result facts cannot be read at the Context provider (`context.packet.inspect` under the context grant); or the facts name a different artifact or digest for that revision: state `unknown`.
3. **Stale.** `invalidated_items` names an item whose obligation is not `advisory`: state `stale`.
4. **Unknown.** `unverified_items` names an item whose obligation is not `advisory`, or a required item of that revision is not `satisfied`: state `unknown`.
5. **Otherwise** `current`.

An item whose obligation the facts do not state counts as required. A binding obligation or fallback this interface does not name is always withheld.

**Decision.**
- **`required_before_start`:** dispatch only when the state is `current`; otherwise withhold.
- **`advisory` with `proceed_with_gap`:** always dispatch, with `gap` true when the state is not `current`.
- **Re-evaluation.** A withheld item is evaluated again at later polls. A new check record is published only when the state or decision differs from the item's previous check.

**Mutants.** These deliberately broken kernels exist only so fixtures can prove they detect them:
- **`ignores-required-boundary`** still evaluates and publishes honest check records, but it dispatches every due item whatever its state.
- **`trusts-advertised-digest`** does not hash the fetched packet bytes, and treats bytes the provider returned for the reference's digest as held.

**Records.** Each record is published at the records provider as a sealed Evidence artifact:
- **Commands:** `evidence.upload.prepare`, `evidence.upload.append` and `evidence.seal`, with precondition revisions as EVIDENCE §4 requires.
- **Content:** canonical JSON (ENCODING) with media type `application/json`.
- **Descriptor:** filled from the schemas. The source is `{ kind: "thirdparty.kernel", id: <work_id> }` and the producer principal is the session principal. The coverage is `complete`, and the capture time is `now`.

| Artifact ID | Content |
|---|---|
| `check.<work_id>.<n>` (`n` from 1, per item) | `{ format: "combraton-thirdparty-kernel-check/1", work_id, boundary: "dispatch", checked_at, packet, state, decision: "dispatch" \| "withhold", reasons }`. `reasons` is a list of short strings for humans; fixtures do not match them. |
| `dispatch.<work_id>` | `{ format: "combraton-thirdparty-kernel-dispatch/1", work_id, dispatched_at, obligation, gap, check: <n> }`, published after its check record. Publishing it **is** the fake execution's dispatch. |

## `minimal-publisher`: an independent Evidence publisher (S-B)

**Role.** An independent Evidence producer (client only). It is one-shot: it publishes the listed artifacts, verifies them and exits.

**Configuration** (`format: "combraton-thirdparty-publisher-config/1"`):

| Member | Meaning |
|---|---|
| `principal`, `clock_file` | As above |
| `providers` | As above |
| `evidence` | `{ provider, grant }` |
| `artifacts` | `[ { id, media_type, content_base64, chunk_bytes } ]`, published in order. `chunk_bytes` bounds each append, which must also respect the provider's `chunk_limit`. |
| `result_file` | Optional: where to write the result |

**Descriptor.** The common descriptor values above, with source `{ kind: "thirdparty.publisher", id: <artifact id> }`.

**Behavior.** For each artifact the publisher:
1. computes the SHA-256 digest over the decoded bytes;
2. prepares with that digest and size;
3. appends the bytes in order, in chunks;
4. seals;
5. fetches the sealed bytes back with `evidence.fetch`, in as many calls as needed, and checks that their digest equals the one it computed.

Any refusal or mismatch ends the run with status 1. On success it writes `{ format: "combraton-thirdparty-publisher-result/1", artifacts: [ { id, digest, size } ] }` to `result_file`, when one is given, and exits 0.

**Mutant `uploads-bytes-differing-from-digest`.** This deliberately broken publisher declares the digest of the configured bytes but appends bytes with one altered byte. The provider must then refuse the seal (EVIDENCE §4, `content_digest_mismatch`), and the publisher exits 1.

## Evidence and claims

| Scenario | Fixture | Coverage claimed | Not claimed |
|---|---|---|---|
| S-A | `composition.thirdparty-kernel-enforces-required-claim-boundary` | A second implementation enforces a required dispatch boundary from CONTEXT §14 read-time facts, with packet bytes checked by digest; advisory work proceeds with its gap; unavailable knowledge is never valid | `execution/1` provider conformance and the EXECUTION §13.1/§13.3 wire shapes |
| S-B | `composition.thirdparty-publisher-feeds-reference-verification` | A second implementation publishes Evidence that the reference Verification provider consumes by exact reference and digest | Verification by a second implementation |
