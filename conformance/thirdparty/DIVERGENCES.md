# Third-party clients: divergence and interpretation log

This log belongs to the contracts-only implementations of `minimal-executor` and `minimal-publisher` (M6, owner decision M6-Q2). They were written from `docs/spec/**`, `docs/decisions/**`, `schemas/**`, `conformance/thirdparty/README.md` (the interface) and the mixed-implementation composition section of `docs/work/release-0.1/M6.md`.

Each entry records a question the documents left open, or answered in a way worth recording, and what the clients do. **Basis** is one of:
- **documents**: the specs, decisions or schemas;
- **interface**: `conformance/thirdparty/README.md`;
- **fixture/transcript**: learned only after the first runner run;
- **own judgment**.

No fixture, interface, spec, schema or runner file was edited.

## Entries written before the first runner run

### TP-1: carrying a grant requires negotiating `core.grants`
- **Location.** Interface "Grants"; CORE §5 (`grant` field), §15; EVIDENCE §1.
- **Question.** The interface requires every profile operation to carry the configured grant. The profile documents list only `core.events` as a dependency of `evidence/1` and `context/1`.
- **Documents.** CORE §5 allows `grant` only when `core.grants` is negotiated. Otherwise the field is unknown and gets `invalid_envelope`. EVIDENCE §1 calls `core.grants` optional.
- **Implemented.** Every session requests `core.grants` as a required Core feature, in addition to the dependencies from `core.feature_dependencies` or the documents. It is a need of this client, not a dependency, and is never mixed into the dependency set.
- **Basis.** Documents and interface.

### TP-2: descriptor `scope` and `retention_class` values
- **Location.** Interface "Records" (kernel) and "Behavior" (publisher); `schemas/evidence/1/common.schema.json` `descriptor_request`; EVIDENCE §3.
- **Question.** The schema requires `scope` and `retention_class`. The interface does not give values, and EVIDENCE §3 calls them opaque and provider-defined.
- **Implemented.** `scope: "thirdparty"` and `retention_class: "standard"`. `capture` carries only `captured_at` (now, from the clock file). `coverage` is `{ completeness: "complete" }`. `work` and `locator` are omitted.
- **Basis.** Own judgment. If a provider refuses these class names, the interface would need to name them.
- **After the first run.** The reference Evidence provider accepted both values, and the fixtures do not match them. They remain unspecified by the interface.

### TP-3: publisher descriptor provenance
- **Location.** Interface, `minimal-publisher` "Behavior".
- **Question.** The kernel's descriptor source and coverage are specified; the publisher's are not.
- **Implemented.** `source: { kind: "thirdparty.publisher", id: <artifact id> }`, producer principal equal to the session principal (EVIDENCE §3), `coverage: complete`, and capture time now, by analogy with the kernel.
- **Basis.** Own judgment.

### TP-4: invalidated or unverified items the facts do not list
- **Location.** Interface evaluation rules 3 and 4; CONTEXT §5 `items`, §14.
- **Question.** An entry in `invalidated_items` or `unverified_items` may name an `item_id` that has no entry in the facts' `items`, so its obligation is not stated.
- **Implemented.** An item whose obligation is not stated counts as not advisory. It therefore makes the state `stale` or `unknown`, and never `current`.
- **Basis.** The interface's wording ("an item whose obligation is not advisory") and CONTEXT §9: an outage or gap never turns a required item into `satisfied`. Own judgment on the fail-closed reading.

### TP-5: obligations and fallbacks the interface does not name
- **Location.** Interface "Decision"; CONTEXT §1, §3 (`wait_until_deadline`, `required_before_transition`).
- **Question.** What does the kernel do with `required_before_transition`, an unknown obligation, or an advisory fallback other than `proceed_with_gap`?
- **Implemented.** It withholds dispatch and records a reason. It has no transitions and does not implement waiting fallbacks. The mutant still dispatches them.
- **Basis.** CONTEXT §1: a required obligation is never silently degraded. Own judgment for the advisory case.

### TP-6: command identity, retransmission and restart
- **Location.** CORE §2, §6.2–§6.3; STREAM §4; EVIDENCE §4 "Interrupted uploads".
- **Question.** The clients have no durable journal. How should command IDs be chosen?
- **Implemented.**
  - Command IDs are deterministic: `tp-kernel.<op>.<artifact>[.<offset>]` and `tp-publisher.<op>.<artifact>[.<offset>]`, or a digest of those parts when they do not fit the identifier grammar.
  - After a lost connection or `retry: same_command`, the whole prepare, append and seal sequence is retransmitted under the same identities and first generations, so bound commands replay.
  - Record bytes are fixed when a record is queued, so a retransmission never changes content.
  - A restarted kernel starts check numbering at 1 again and would collide with records already sealed. The interface does not describe restarts, and no recovery is claimed.
- **Basis.** Documents; own judgment for the restart limit.

### TP-7: Context features the kernel negotiates
- **Location.** M6.md S-A topology; CONTEXT §1, §14.
- **Question.** The kernel sends only `context.packet.inspect`. Which Context features must it request?
- **Implemented.** `context.required_before_start`, `context.advisory` and `context.claims`, all required. Without `context.claims`, `invalidated_items` lists only authority corrections and `unverified_items` is absent. A kernel without the feature would silently miss claim invalidation, so it is requested as required.
- **Basis.** Documents (CONTEXT §14, "Sessions without `context.claims`") and M6.md.

### TP-8: an unreachable Context provider that also holds its packets
- **Location.** Interface evaluation rules 1 and 2; M6.md S-A "an unreadable provider makes it unknown".
- **Question.** When the Context provider also serves `evidence/1` for its own packets and cannot be reached, rule 1 (bytes not fetched: `unsatisfied`) applies before rule 2 (facts unreadable: `unknown`).
- **Implemented.** The interface's rule order. Such an outage is recorded `unsatisfied`, and required work is withheld either way. M6.md's "unknown" holds when only the facts are unreadable.
- **Basis.** Interface. It is consistent with M6.md for the scenarios described there, where the Knowledge provider, not the Context provider, is stopped.

### TP-9: what a fetch response is trusted for
- **Location.** Interface "References and digests"; EVIDENCE §5; EXECUTION §13.1 "Fetch".
- **Question.** Which fields of an `evidence.fetch` response does the client rely on?
- **Implemented.**
  - Bytes are accepted only when `availability.state` is `available`, `artifact` and `offset` echo the request, `size` is stable across calls, and `next_offset` equals `offset` plus the decoded length.
  - The assembled bytes are hashed by the client. The response's `digest` is never used as evidence.
  - Any inconsistency counts as "cannot be fetched".
  - The publisher also compares the seal outcome's digest and size with its own, but only the fetch-back hash counts as verification.
- **Basis.** Documents and interface.

### TP-10: the publisher mutant with empty content
- **Location.** Interface, mutant `uploads-bytes-differing-from-digest`.
- **Question.** Content with zero bytes has no byte to alter.
- **Implemented.** The mutant flips the lowest bit of the last byte. For empty content it cannot differ, so it behaves as the correct publisher.
- **Basis.** Own judgment.

### TP-11: dispatch record fields under the kernel mutant
- **Location.** Interface, mutant `ignores-required-boundary`, and the `dispatch.<work_id>` record.
- **Question.** For required work dispatched against the boundary, what are `gap` and `check`?
- **Implemented.** The check record stays honest (`decision: "withhold"`). The dispatch record has `gap: state != current` and `check` set to the number of the latest check of that item.
- **Basis.** Own judgment.

### TP-12: receive limits and fetch sizes
- **Location.** CORE §4.2, STREAM §1.5, EVIDENCE §4 "Chunk sizing".
- **Implemented.**
  - `receive_limits.max_frame_bytes` is 4 MiB. Fetches ask for 256 KiB at a time, and the provider may return fewer bytes.
  - `context.packet.inspect` asks for a 1-byte excerpt, because complete bytes come only from `evidence.fetch`.
  - Frames the clients send are bounded by the provider's negotiated `max_frame_bytes`, and appends by `chunk_limit`.
- **Basis.** Documents; the sizes are own judgment.

## Changes and findings after the first runner run

First run, at commit `fbc71fe`: both fixtures failed.
- `composition.thirdparty-kernel-enforces-required-claim-boundary` failed at step 29 (0-based): `check.w-valid.1` was `not_found` within 20000 ms.
- `composition.thirdparty-publisher-feeds-reference-verification` failed at step 5: the publisher exited with status 1.

The fixtures were opened only after that run.

### TP-13: `core.grants` was intended but never requested (implementation bug)
- **Location.** TP-1; CORE §5.
- **Question.** Why did every `evidence.upload.prepare` and `evidence.fetch` answer `invalid_envelope`?
- **What happened.** TP-1 was written before the run, but the sessions requested only the dependency set. The client logs showed `core/1[core.events]` and `invalid_envelope` on the first prepare, so the `grant` field was an unknown field (CORE §5.1). The kernel therefore recorded `w-valid` as `unsatisfied`, and its records were never published.
- **Implemented.** Every session now lists `core` with `core.grants` among its required profiles. Both fixtures then pass.
- **Basis.** Documents (CORE §5, §15) and the interface. The diagnosis came from the client stderr logs the runner saved, not from fixture content.

### TP-14: diagnostics
- **Change.** Refusal messages in client logs now name the error's `details.path`, `reason`, `limit`, `features`, `computed` or `received` when present. Identical consecutive log lines are collapsed into a repeat count. Credentials are still redacted, and the runner's stderr logs were checked to contain no `ccred1` string.
- **Basis.** Own judgment. The first run's kernel log held about 64 KB of one repeated line.

### TP-15: the state is per packet revision, not per binding obligation
- **Location.** Interface evaluation rules 3 and 4; fixture work item `w-adv`.
- **Question.** For an advisory binding to a packet whose **required** item is invalidated, is the state `stale` or `current`?
- **Documents and interface.** Rules 3 and 4 refer to the obligations of the packet's items ("an item whose obligation is not advisory"), not to the binding's obligation.
- **Implemented, unchanged.** `w-adv` is recorded `stale` with decision `dispatch` and `gap: true`. The fixture expects exactly this.
- **Basis.** Interface; confirmed by fixture.

### TP-16: a withheld item's later check after the Knowledge provider stops
- **Location.** Interface "Re-evaluation"; CONTEXT §14 "At the read".
- **Observation.** After `knw` stopped, `w-req` (bound to `r-1`, whose required claim had been rejected) was re-evaluated. The Context provider then reported the item as unverified rather than invalidated, so the kernel published `check.w-req.2` with state `unknown`, still withheld. The fixture does not look at `check.w-req.2`; it checks only that `dispatch.w-req` does not exist.
- **Implemented, unchanged.** A new check record is published because the state differs from the previous check, as the interface requires.
- **Basis.** Interface; observed in the client log.

### TP-17: what the digest-mismatch case exercises
- **Location.** Fixture work item `w-tampered`; interface rule 1; EVIDENCE §5 "Reference mismatch".
- **Observation.** `w-tampered` binds `packet.r-2.1` with the digest of `packet.r-1.1`. The reference provider refuses the fetch with `artifact_digest_mismatch`, so the kernel records `unsatisfied` with reason `packet bytes not fetched: artifact_digest_mismatch`.
- **Coverage.** The kernel's own hash of the bytes it assembled is not what decides this case. Bytes served under the right digest but altered, as the `evidence_store.serve_altered_bytes` control would produce, are not exercised by this fixture. The kernel would still refuse them (`fetched packet bytes do not match the reference digest`), but no fixture run shows it.
- **Basis.** Fixture and transcript. Nothing changed; recorded as a coverage limit.

## Mutant runs

| Mutant | Fixture | Outcome | Failing step (0-based) and reason | Cause |
|---|---|---|---|---|
| `minimal-executor=ignores-required-boundary` | `composition.thirdparty-kernel-enforces-required-claim-boundary` | `fail` | 51: `expected error not_found, received success` for `evidence.inspect` of `dispatch.w-req` | The mutant published an honest `check.w-req.1` (`stale`, `withhold`) and then dispatched anyway (`dispatch.w-req`, `gap: true`). It also dispatched `w-tampered`. |
| `minimal-publisher=uploads-bytes-differing-from-digest` | `composition.thirdparty-publisher-feeds-reference-verification` | `fail` | 5: `client "publisher" exited with status 1; 0 is required` | The provider refused the seal of `tp-contract` with `content_digest_mismatch` (client log), and the publisher exited 1. |

## Unresolved

Nothing fails in these runs. Open interface questions, for the owner and not for the clients: TP-2 (descriptor `scope` and `retention_class`), TP-6 (kernel restart) and TP-8 (Context provider outage when it also holds the packets).
