# Evidence profile `evidence/1` — proposed M4 draft

> **Status: proposed draft for Protocol 0.1 milestone M4.** Nothing here is normative until M4 is accepted together with its schemas, fixtures, mutants and independent evidence. Names marked *candidate* may change during M4. Architecture: [SPEC §2, §8–§10, §12](../SPEC.md). Task: [M4](../../work/release-0.1/M4.md). Requirement IDs refer to the [matrix](../../work/release-0.1/MATRIX.md). Sources: cbr `docs/spec/SPEC.md` §3, §4, §10 and `docs/spec/PREPARATION-AND-DELIVERY.md` at `3278393`; pio `docs/spec/STANDALONE-CLIENT.md` at `e65b7c0`.

An **evidence provider** accepts immutable bytes with their provenance, keeps them under declared retention, and lets authorized principals find and fetch them. CBR is one evidence provider; a CI system or runtime observer publishing through this profile is another, and a non-Combraton evidence publisher must be able to implement it without any Combraton library (SPEC §13). A **producer** publishes evidence. A **reader** finds and fetches it.

The profile preserves **what was captured, by whom and under which coverage**. It does not establish that the content is true, complete beyond its declared coverage, or accepted by anyone (§12).

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119 and RFC 8174 when in capitals.

## 1. Dependencies and negotiation

- `evidence/1` depends on `core/1` with feature `core.events` (CORE §16). `core.grants` is optional; without it only authority principals operate.
- Required Core features are published here and enforced at negotiation, as for Execution (owner decision D2, 2026-09-14): a required `evidence/1` request without `core.events` is refused with `unsupported_profile` and a `dependency_not_selected` item; an optional request is left unselected with the same item.
- Optional features (*candidate*): `evidence.manifests` (§6) and `evidence.retention_control` (`hold`, `release` and `purge`, §8). Base providers still report availability and retention class.
- Every command follows the Core command path (CORE §10). Byte chunks, seals and retention changes commit in owner transactions with their events.

## 2. Identities

| Identity | Scope and meaning | Must not be used as |
|---|---|---|
| Artifact subject `{ "kind": "evidence.artifact", "id" }` | One published descriptor. The producer chooses the ID and creates it with precondition revision 0. | The content's identity |
| `digest` | Algorithm-qualified digest of the sealed bytes (ENCODING §3). Two artifacts with equal digests hold equal bytes. | Proof of provenance or authorization |
| Evidence reference `{ "artifact": subject, "digest" }` | How other profiles cite evidence. A consumer MUST check both: the artifact and the exact digest it expects. | A reference by locator |
| `locator` | Where the provider stores or serves the bytes. Informative, may change, never identity, and MUST NOT embed credentials (EVD-1). | An authorization token |
| `upload` | The staged state of an artifact before seal | A second artifact |

An artifact with a given digest is **exactly** the evidence a reference names. The same bytes published twice are two artifacts with the same digest and separate provenance.

## 3. Descriptor

The descriptor is fixed at `prepare` and immutable once sealed:

| Field | Meaning |
|---|---|
| `digest`, `size`, `media_type` | The declared content, verified at seal (§4) |
| `producer` | `{ principal, producer_id? }`. `principal` MUST equal the session principal of the `prepare` command (EVD-4). `producer_id` is the producer's own stable name for itself. |
| `source` | What was captured: `{ kind, id }`, for example a terminal stream, a test report or a build log of a named execution |
| `scope` | The visibility scope the artifact belongs to (a project, session or work binding; opaque to this profile) |
| `capture` | `{ captured_at, uncertainty?: { not_before, not_after }, anchors }`. Anchors identify code trees, dirty snapshots, build and environment identities where relevant (EVD-4). Capture time is metadata, never ordering. |
| `coverage` | Required (EVD-7). `{ completeness: "complete" \| "partial" \| "unknown", covered: [ ... ], gaps: [ ... ] }`. Terminal output is declared as such; it is never a complete tool trace. |
| `work` | Optional work binding reference, for example an execution subject (§9) |
| `visibility` | Who may read it within the scope, as the provider's authorization model states (§7) |
| `retention_class` | Provider-defined retention class name, reported, never inferred |
| `locator` | Optional, informative (§2) |

A `prepare` whose locator carries credentials (user information, query tokens or signed parameters) is `invalid_envelope` at `/payload/locator`.

## 4. Upload and seal

| Operation | Kind | Semantics |
|---|---|---|
| `evidence.upload.prepare` | command | Creates the artifact in state `staged` with its descriptor (precondition revision 0). Outcome `{ artifact, state: "staged", chunk_limit }`. |
| `evidence.upload.append` | command | Payload `{ offset, data_base64 }`. `offset` MUST equal the bytes received so far, and the result MUST NOT exceed the declared size. Replaying the same command returns its receipt; a different chunk at an already-written offset is refused. |
| `evidence.seal` | command | Verifies that the received size equals the declared size and the digest of the received bytes equals the declared digest, then makes the artifact `sealed`. |

- **Partial uploads cannot be sealed** (EVD-2). Too few bytes: `upload_incomplete` with `received` and `declared`. A digest mismatch: `content_digest_mismatch` with the digest the provider computed. Nothing changes; the artifact stays `staged`.
- **Seal is idempotent.** Retransmitting the seal command replays its outcome. A new seal command on an already sealed artifact with the same declared digest returns `{ state: "sealed", already_sealed: true }` at the unchanged revision and appends no event.
- **Unverified seal is never reported.** A provider MUST NOT report `sealed` for bytes it did not verify.
- **Abandoned uploads.** A provider MAY discard staged bytes after a declared staging timeout; the artifact then reports availability `unavailable` with reason `staging_expired`, never `not_found`.
- **Chunks** are bounded by the negotiated frame limit and the provider's `chunk_limit`. Out-of-band transfer is a Remote trust concern and out of scope for 0.1.

## 5. Inspect, query and fetch

| Operation | Kind | Semantics |
|---|---|---|
| `evidence.inspect` | query | `{ artifact }` → descriptor, state, revision, availability (§8), holds and, for manifests, completeness (§6) |
| `evidence.query` | query | Filters on producer, source, work binding, media type, digest and scope, with a bounded page and an opaque cursor. Lists only artifacts the principal may read. |
| `evidence.fetch` | query | `{ artifact, digest, offset?, max_bytes? }` → `{ offset, data_base64, next_offset, size, digest }` for a sealed, available artifact. A `digest` that does not match the artifact's is refused with `digest_not_current` and no bytes, so a reader never mistakes other bytes for the evidence it cited. |

- **Exact bytes.** Fetched bytes are the sealed bytes. A provider MUST NOT regenerate, re-encode or normalize them.
- **Integrity on read.** If stored bytes no longer match the digest, the provider MUST NOT serve them: availability becomes `unavailable` with reason `integrity_failed`.
- **Existence is not revealed** (EVD-5, CORE-12). An unauthorized principal gets the same `permission_denied` for an existing and a nonexistent artifact; `evidence.query` omits unreadable artifacts and reports `filtered: true` as `core.events.read` does.

## 6. Compound manifests (`evidence.manifests`)

- A manifest is an artifact with media type `application/vnd.combraton.evidence-manifest+json` (*candidate*) whose content lists children `{ role, digest, size, media_type, required }`.
- At seal, the provider validates the content and records the manifest's children.
- `evidence.inspect` on a manifest reports `completeness: { state: "complete" | "incomplete", missing_required: [ digests ] }`, evaluated when read against sealed, available artifacts the provider holds (EVD-3). A manifest with a missing or purged required child is `incomplete`; a provider MUST NOT report it complete.
- A manifest does not authorize its children; each child is read under its own authorization.

## 7. Authorization and rights

Under a grant (CORE §15):

| Right | Covers |
|---|---|
| `evidence.publish` | `prepare`, `append` and `seal` on artifacts the grant's resources cover |
| `evidence.read` | `inspect`, `query` results and `fetch` of covered artifacts, and their events |
| `evidence.hold` | `hold` and `release` on covered artifacts |
| `evidence.purge` | `purge` on covered artifacts |

Resources name kind `evidence.artifact` with optional `id` or `id_prefix`. The producer of a `prepare` is always the session principal (§3); a grant never lets one principal publish as another.

## 8. Availability and retention

- **Availability** is observed separately from state: `available`, `partial` (some bytes readable, for example a manifest with unavailable children or a truncated store), `unavailable` (with `reason`), `purged`.
- **Holds** (`evidence.retention_control`): `evidence.hold` records `{ hold_id, holder: reference, reason, expires_at? }`, for example a context packet, an execution outcome or an open gate. `evidence.release` ends a hold. A hold is a retention obligation, not a copy.
- **Purge** (`evidence.purge`): discards the bytes and keeps the descriptor as a tombstone with availability `purged`. The outcome and the `evidence.artifact.purged` event carry a **proof-loss report** (EVD-6): `{ artifact, digest, purged_at, released_holds, affected }`, where `affected` lists the references the provider knows depend on it (holds, manifests listing it as a child, packets citing it).
  - An artifact with active holds is refused with `hold_active` unless the purge names those holds in `release_holds`; they are then released and listed.
  - A purged artifact is never `not_found` to a principal that may read it, and it never silently disappears from `query`.

## 9. Work bindings

A work binding constrains where a producer may publish and as whom (EVD-8, SPEC §9).

- A grant issued for a unit of work names the destination through its resources (for example `id_prefix` for that work's artifacts) and the producer through its holder. The artifact's `work` field names the bound work.
- A `prepare` outside the grant's resources is `permission_denied` with `out_of_scope`. A `prepare` whose `work` does not match the work named by the grant's binding is `permission_denied` with reason `binding_violation` (*candidate*).
- An executor that publishes its output as evidence does so as an ordinary producer under such a grant. The execution outcome references the sealed artifacts (EXE-21, §10).

## 10. Execution outcome references (EXE-21)

Cross-profile with EXECUTION §6:
- A completion record MAY carry `outputs: [ { role, evidence: { artifact, digest } } ]`, and `execution.inspect` lists them.
- Each reference names a sealed artifact the executor published under its work binding. A caller resolves it through `evidence.inspect` and `evidence.fetch` under its own authorization; the reference itself grants nothing.
- An executor MUST NOT list an artifact it has not sealed, and a reference's digest MUST equal the sealed digest.

## 11. Events

Evidence events use Core event records with subject `{ "kind": "evidence.artifact", "id" }`. *Candidate* types:

| Type | Payload |
|---|---|
| `evidence.artifact.staged` | `{ descriptor }` |
| `evidence.artifact.sealed` | `{ digest, size }` |
| `evidence.artifact.held` / `evidence.artifact.released` | `{ hold_id, holder }` |
| `evidence.availability.changed` | `{ availability, reason? }` |
| `evidence.artifact.purged` | the proof-loss report |

## 12. What evidence does not establish

A sealed artifact establishes that these exact bytes were received from this authenticated principal with this declared provenance and coverage. It does not establish:
- that the content is true, or that a claim derived from it holds;
- completeness beyond the declared coverage; terminal output is not a complete tool trace;
- independence: repeated publication of shared material is not corroboration;
- that the bytes will stay available beyond their retention class and holds.

## 13. Conformance and test controls

- **Reference participants only.** Fixtures use a reference evidence provider with a scripted store. No fixture depends on CBR's memory engine or storage.
- **Store faults** through launch configuration (decision 007 environment controls): corrupt stored bytes (integrity on read), make an artifact unavailable, expire staging.
- Purge and holds are driven through the protocol operations themselves; no operation is reserved for testing.
