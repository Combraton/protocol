# Core profile `core/1` — release draft

> **Status: proposed draft for Protocol 0.1, milestone M1.** Not a released contract. Field and error names become normative only when the release is accepted together with its schemas and conformance fixtures. Architecture: [SPEC](../SPEC.md). Plan: [release plan](../../work/release-0.1/PLAN.md). Requirement IDs refer to the [matrix](../../work/release-0.1/MATRIX.md).

This document defines the Core command path: sessions, negotiation, command and query envelopes, the order of checks, idempotency, preconditions, authority epochs, acknowledgments and errors. Grants, events and subscriptions, capability snapshots and effect reconciliation are Core too; they are specified in milestone M2 and marked **reserved** below.

Wire encoding, digests and transport are defined separately:

- [Canonical encoding and digests](../bindings/ENCODING.md)
- [Local stream binding](../bindings/STREAM.md)

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119 and RFC 8174 when in capitals.

## 1. Terms

| Term | Meaning |
|---|---|
| Participant | Any program speaking this protocol. |
| Provider | The participant that owns the subjects an operation addresses, such as an executor or context service. |
| Caller | The participant sending an operation to a provider. One program can be a caller of one provider and a provider to others. |
| Principal | The identity the provider attributes to a connection, established by the transport binding. A message cannot assert its own principal. |
| Subject | An object owned by the provider, named by `kind` and `id`. |
| Revision | A provider-owned integer per subject. `0` means the subject does not exist. Every committed change increases it. It says nothing about any other subject or provider. |
| Command | An operation that may change provider state. Commands are idempotent under their command identity. |
| Query | An operation that reads state. A query is not recorded as an operation and has no command identity. |
| Operation | A named command or query, written `<profile>.<name>`, such as `core.negotiate`. |

## 2. Identities are distinct

| Identity | Scope | Must not be used as |
|---|---|---|
| Transport request ID (JSON-RPC `id`) | One connection; matches a response to its request | An idempotency key, a message ID or a durable reference |
| `message_id` | One transmission of one envelope; unique per sender | A command identity; a retransmission gets a new `message_id` |
| `command_id` | One intended change, chosen by the caller; unique within the caller principal's deduplication scope | A subject ID, an execution ID or a retry counter |
| Subject `kind` + `id` | One provider-owned object | Proof that an operation happened |
| `operation_ref` | The provider's durable record of an accepted command | A command ID supplied by the caller |

A retransmitted command keeps its `command_id` and content and gets a new `message_id` and transport request ID. A deliberately new attempt is a new command with a new `command_id` (CORE-1).

## 3. Sessions

A session is one authenticated connection under the transport binding.

1. **Before negotiation**, a provider MUST answer only `core.describe` and `core.negotiate`. Any other known operation gets `negotiation_required`; an unknown one gets `method_not_found` (§10).
2. **`core.negotiate` succeeds at most once per session.** A second call gets `already_negotiated`. To change the negotiated set, open a new session.
3. **After negotiation**, an operation of a profile that was not selected gets `profile_not_negotiated`. This applies even when the provider supports that profile.
4. **Session state is not durable.** Closing a session neither cancels nor confirms anything in flight. Callers reconcile by command identity after reconnecting ([STREAM](../bindings/STREAM.md)).

## 4. Describe and negotiate

### 4.1 `core.describe` (query)

This returns the provider's manifest:

- Provider name and version.
- Every supported profile, with its supported major versions and features.
- The profiles the provider declares **unsupported**; for a 0.1 release this includes `coordination` and `remote-trust`.
- Limits (§9).
- The deduplication generation window (§6.3).

A provider MUST NOT list a profile as supported unless it also supports every profile that profile depends on (REL-2).

### 4.2 `core.negotiate` (query)

**Request.** The caller identifies itself (`caller: { name, version }`) and gives `receive_limits: { max_frame_bytes }`, the largest frame it accepts from the provider (at least 1 MiB). It lists the profiles it wants. For each profile it gives:

- `majors`: acceptable major versions.
- `required`: whether the session is useless without this profile.
- `required_features`: features the caller relies on.
- `optional_features`: features the caller can use if present.

**Provider rules:**

- For each profile, select the highest major version both sides accept.
- Select the intersection of features.
- **Refuse the whole negotiation** if a `required` profile cannot be selected or a required feature is missing. Use `unsupported_version` when no common major exists, `unsupported_profile` when the profile is unknown or declared unsupported, and `unsupported_required_feature` for a missing feature. The error `details.unsatisfied` lists every unsatisfied item as `{ profile, feature?, reason }`, with reasons `unknown_profile`, `declared_unsupported`, `no_common_major`, `unknown_feature` or `dependency_not_selected`. When items fail for different reasons, the code is chosen in this order: `unsupported_profile`, then `unsupported_version`, then `unsupported_required_feature`.
- Optional profiles and features that cannot be selected are reported in `unselected` with a reason. They are not errors.
- Include `core` implicitly; a caller cannot deselect it.

**Result:** the selected profiles with major version and features, `unselected` items, effective limits, and the current deduplication window.

Unknown feature names in `optional_features` are ignored and reported as unselected. Unknown names in `required_features` are refused (CORE-3).

## 5. Envelopes

A **command** is sent as the `params` of a transport request. All command envelope fields are listed here; the schema is `schemas/core/1/command.schema.json`.

| Field | Required | Meaning |
|---|---|---|
| `operation` | yes | Operation name. MUST equal the transport method name. |
| `message_id` | yes | Per-transmission identity. |
| `command_id` | yes | Command identity (§6). |
| `dedupe_generation` | yes | Deduplication generation under which the caller issued this command (§6.3). |
| `subject` | yes | `{ "kind": string, "id": string }` primary subject. |
| `preconditions` | yes | Array, possibly empty, of `{ "subject": …, "revision": integer ≥ 0 }` (§7). |
| `authority_epoch` | no | Epoch the caller acts under, when the operation's profile requires one (§8). |
| `requires` | yes | Array, possibly empty, of feature names or extension keys this command's meaning depends on (§5.1). |
| `correlation` | no | Opaque caller metadata object. Not interpreted; not part of the command digest. |
| `caused_by` | no | Array of references explaining why the command exists. Not part of the command digest. |
| `extensions` | no | Object keyed by namespaced extension keys (§5.1). |
| `command_digest` | yes | Algorithm-qualified digest of the command intent ([ENCODING](../bindings/ENCODING.md)). |
| `payload` | yes | Operation-specific object. |

A **query** has `operation`, `message_id`, optional `requires`, optional `extensions` and `payload`. It has no command identity, preconditions or digest.

### 5.1 Closed objects, `requires` and extensions

Envelope and payload objects are **closed**. A field that the negotiated profile version and features do not define makes the message invalid (`invalid_envelope`). An unknown field cannot be classified as safe to ignore, so it is treated as required semantics and refused (CORE-4).

Optional additions that a receiver may safely ignore go in `extensions`. Each key is a domain the extension author controls, a slash, and a name, such as `example.org/trace`.

- A feature name is `<profile>.<feature>`, such as `core.digest-sha512`, and never contains a slash.
- An extension key always contains exactly one slash.

That is how a `requires` entry is told apart.

- An extension is **optional** unless its key appears in `requires`. A receiver MAY ignore an optional extension it does not understand. When the operation defines stored content, the receiver MUST either store the extension unchanged with that content or declare in its manifest that it drops unknown extensions.
- An extension key or feature name listed in `requires` MUST be understood and negotiated. Otherwise the message is refused with `unsupported_required_feature`, before any state change.
- `requires` entries MUST be unique. An entry containing a slash MUST also be present in `extensions`. A violation of either rule is `invalid_envelope`. An empty `requires` means nothing beyond the negotiated profile versions is required.
- A caller MUST NOT send fields that belong to a feature the session did not select.

This mirrors the "critical" marking used by JWS `crit` and X.509 critical extensions. The decision record [002](../../decisions/002-schema-language-and-extensibility.md) gives the evidence and alternatives.

## 6. Command identity and idempotency

### 6.1 Command intent and digest

The **command intent** is the JSON object with exactly these members of the envelope:

- `operation`
- `subject`
- `preconditions`
- `requires`
- `payload`
- `extensions` — always present in the intent, as an object containing only the extensions whose keys are listed in `requires`; an empty object if there are none

`command_digest` is the digest of the intent under [ENCODING](../bindings/ENCODING.md). The provider recomputes it and refuses a mismatch with `digest_mismatch` (CORE-6).

`message_id`, `dedupe_generation`, `authority_epoch`, `correlation`, `caused_by` and optional extensions are excluded. A retransmission can change them without becoming a different command. For example, a caller that reconnects under a new authority epoch can retransmit the same command.

### 6.2 Deduplication scope and binding

The deduplication key is **(principal's deduplication scope, `command_id`)**. The same `command_id` from a different scope is a different command. A provider MUST NOT reveal that another scope used it (CORE-12, M2).

A command identity is **bound** when the provider durably accepts the command (§10). A command rejected before acceptance — invalid, stale precondition, stale epoch, unsupported feature — does not bind its identity. A later transmission is evaluated afresh and may get a different result if state has changed.

Once bound:

- A command with the same key and the **same** `command_digest` returns the stored acknowledgment and outcome unchanged, with `replay: true` (CORE-7).
- A command with the same key and a **different** `command_digest` is refused with `idempotency_conflict`. The error names the command ID only; it does not disclose the stored intent (CORE-7).

### 6.3 Deduplication generations

A provider cannot keep every command record forever. It also must never treat a forgotten command as new, because that could repeat a side effect (CORE-8). Generations make forgetting explicit without relying on clocks:

1. **Provider window.** The provider publishes a window `{ "oldest_retained": g_old, "current": g_cur }` in `core.describe` and `core.negotiate`. It MUST retain the records of every bound command issued under a generation ≥ `g_old`.
2. **Advancing.** The provider may advance `current` at any time. It may raise `oldest_retained` only by discarding records of generations below the new value.
3. **Issuing.** A caller issues a new command under the `current` generation it last observed. It persists the command, including the generation, before sending, so a retransmission after a crash carries the original generation.

**Lookup.** When a command arrives, with `g` its `dedupe_generation`:

| Condition | Result |
|---|---|
| `g > current` | `invalid_envelope`: a generation the provider never issued. |
| A record exists for the key | Rules of §6.2. |
| No record and `g < oldest_retained` | `dedupe_history_unavailable`. The provider cannot tell whether the command was accepted earlier. The caller must reconcile by inspecting the subject. It must not simply resend under a new command ID if a duplicate effect would be harmful. |
| No record and `oldest_retained ≤ g ≤ current` | New command; continue processing. |

A command journaled by a caller long ago but never sent also gets `dedupe_history_unavailable`. That is conservative on purpose: the provider cannot distinguish "never received" from "received and forgotten".

## 7. Preconditions and revisions

Each precondition entry names a subject and the revision the caller expects:

- `revision: 0` means the subject MUST NOT exist.
- `revision: n > 0` means the subject MUST exist at exactly revision `n`.

All preconditions of a command are checked together. If any fails, the command is refused with `precondition_failed` and **no** part of it takes effect. The error lists every failed entry. Where the principal may read the subject, the entry includes its current revision (CORE-9).

Preconditions are checked **after** the idempotency lookup. A retransmitted create whose original succeeded therefore returns the original acknowledgment, even though the subject now exists and a fresh evaluation would fail.

A revision is meaningful only for its owning provider and subject. A caller MUST NOT use one provider's revision or event position as a precondition on another provider's subject.

## 8. Authority epochs

Some operations act under an authority that can be taken over, such as a controller lease or a grant epoch; their profiles define which. For those operations the provider tracks a current epoch per authority scope and checks `authority_epoch`:

| Condition | Result |
|---|---|
| Epoch absent where the operation requires one | `invalid_envelope` |
| Epoch lower than the current epoch | `stale_authority_epoch`. The error includes the current epoch where the principal may know it. |
| Epoch higher than any the provider issued | `unknown_authority_epoch` |

The epoch check comes after the idempotency lookup. A retransmission of an already-accepted command still returns its stored result after an epoch change (CORE-10).

**Reserved for M2:** how epochs are issued and advanced by grants and leases.

## 9. Limits

The provider declares these limits in its manifest and negotiation result:

| Limit | Meaning |
|---|---|
| `max_frame_bytes` | Transport frame limit, enforced by the binding before JSON decoding. |
| `max_payload_bytes` | Canonical encoded size of `payload`. |
| `max_string_bytes` | Longest string value, in UTF-8 bytes. |
| `max_array_items` | Longest array. |
| `max_depth` | Deepest nesting of objects and arrays. |

A message within the frame limit that exceeds another limit is refused with `limit_exceeded`, naming the limit. Profiles may define tighter per-field limits (CORE-5).

## 10. Command processing order

A provider MUST apply these steps in order. The first failing step determines the error. Steps 1–4 happen before any durable change.

| Step | Check | Error on failure |
|---|---|---|
| 1 | In this order: operation known; session negotiated (unless the operation is `core.describe` or `core.negotiate`); operation's profile selected | `method_not_found`, `negotiation_required`, `profile_not_negotiated` |
| 2 | Envelope and payload decode within limits; objects closed; types valid | `limit_exceeded`, `invalid_envelope` |
| 3 | Every `requires` entry negotiated and understood | `unsupported_required_feature` |
| 4 | Digest algorithm supported; `command_digest` matches the recomputed digest | `unsupported_digest_algorithm`, `digest_mismatch` |
| 5 | Deduplication lookup (§6) | `invalid_envelope`, `idempotency_conflict`, `dedupe_history_unavailable`; or return the stored result |
| 6 | Authorization — **reserved M2**: grant, audience, rights | `permission_denied` |
| 7 | Authority epoch (§8) and preconditions (§7) | `stale_authority_epoch`, `unknown_authority_epoch`, `precondition_failed` |
| 8 | In one owner transaction: commit the command record binding its identity, the state change, resulting events and effect records | `unavailable` if the provider cannot commit; nothing is bound |
| 9 | Return the acknowledgment and outcome | — |

**Why step 5 precedes step 6** in this draft: the stored result is returned only to the same deduplication scope, which already implies prior authorization. **Open for M2:** whether a revoked principal may still read the stored result of its own earlier command.

A query applies steps 1–3 and then the operation's read rules. It creates no command record.

## 11. Acknowledgment and outcome

A successful command returns:

```json
{
  "acknowledgment": {
    "command_id": "cmd-7",
    "command_digest": "sha256:…",
    "operation_ref": "op-41",
    "subject": {"kind": "core-test.subject", "id": "s-1"},
    "revision": 1,
    "effect_refs": []
  },
  "outcome": {},
  "replay": false
}
```

An acknowledgment means the provider durably recorded the command and its immediate state change. It does not mean any external effect happened, succeeded, was verified or was accepted by anyone. Each profile defines what its outcome establishes.

A replay returns an `acknowledgment` and `outcome` byte-for-byte equal under canonical encoding to the original, with `replay: true`.

## 12. Errors

Errors use the transport's error object. The symbolic `data.code` is normative; numeric transport codes are fixed by the binding. Every error `data` contains:

- `code`: symbolic error.
- `retry`: one of `no`, `same_command`, `after_reconcile`, `after_renegotiate`.
- `details`: object, possibly empty, with the members listed below.

| Code | Retry | Meaning | `details` |
|---|---|---|---|
| `invalid_envelope` | `no` | Schema violation, unknown field, or impossible value | `path`, `reason` |
| `limit_exceeded` | `no` | A declared limit other than the frame limit was exceeded | `limit`, `maximum` |
| `negotiation_required` | `after_renegotiate` | Operation before successful negotiation | — |
| `already_negotiated` | `no` | Second negotiation in one session | — |
| `method_not_found` | `no` | Operation unknown to the provider | `operation` |
| `profile_not_negotiated` | `after_renegotiate` | Operation's profile not selected in this session | `profile` |
| `unsupported_version` | `no` | Negotiation: no common major version for a required profile | `unsatisfied` |
| `unsupported_profile` | `no` | Negotiation: required profile unknown, declared unsupported, or missing a dependency | `unsatisfied` |
| `unsupported_required_feature` | `no` | Negotiation: a required feature is unavailable; or a message's `requires` names something not negotiated or understood | `unsatisfied` in negotiation; `features` for a message |
| `unsupported_digest_algorithm` | `no` | Digest algorithm not supported | `algorithm`, `supported` |
| `digest_mismatch` | `no` | `command_digest` differs from the recomputed digest | `expected` |
| `idempotency_conflict` | `no` | Command identity bound to a different intent | `command_id` |
| `dedupe_history_unavailable` | `after_reconcile` | Command identity may have been used but its record was discarded | `oldest_retained` |
| `stale_authority_epoch` | `after_reconcile` | Caller's epoch was superseded | `current_epoch` if permitted |
| `unknown_authority_epoch` | `no` | Epoch never issued | — |
| `precondition_failed` | `after_reconcile` | One or more revision preconditions unsatisfied | `failed`: entries with `subject`, `expected`, and `current` if permitted |
| `not_found` | `no` | Query target absent or not visible to this principal | — |
| `permission_denied` | `no` | Reserved M2 | Reserved |
| `unavailable` | `same_command` | Provider temporarily cannot process; nothing was bound | — |
| `internal_error` | `after_reconcile` | Provider failed in an undefined way; outcome unknown | — |

`retry: same_command` means retransmitting the identical command, with the same `command_id`, is safe. It never means "send a new command". A caller that loses a response retransmits the same command; `unavailable` and `internal_error` do not tell the caller whether the command was bound.

## 13. Conformance-only test profile `core-test/1`

Core behavior needs some subject to act on, so the conformance suite defines `core-test/1`. It is **not a product profile**. Providers MAY implement it only to run Core fixtures, and MUST NOT expose it outside test configurations.

Fixtures reach every domain state — revisions, epochs, bound commands — only through these ordinary operations. The separate test-control channel changes only the environment (§13.1).

| Operation | Kind | Semantics |
|---|---|---|
| `core-test.authority.claim` | command | Subject `{ "kind": "core-test.authority", "id": "core-test" }`. Advances the authority epoch of scope `core-test` by one; outcome `{ "epoch": n }`. Precondition on the authority subject's revision, which equals the epoch. No `authority_epoch` field. Models a controller takeover. |
| `core-test.subject.put` | command | Payload `{ "value": string, "labels"?: { string: string } }`. `labels` exists only so fixtures can exercise canonical member ordering with arbitrary keys; it is not stored. With precondition revision `0`, creates the subject at revision 1. With precondition revision `n`, replaces its value at revision `n + 1`. Requires `authority_epoch` for scope `core-test`. Outcome `{ "value": … }`. |
| `core-test.subject.get` | query | Payload `{ "subject": … }`. Returns `{ "revision": n, "value": … }` or `not_found`. |
| `core-test.subject.applied_count` | query | Payload `{ "subject": … }`. Returns how many commands changed the subject. Fixtures use it to detect a re-executed duplicate without trusting the acknowledgment. |

The primary subject's precondition entry is mandatory for `core-test.subject.put`. The authority scope starts at epoch 0, where the authority subject does not exist.

### 13.1 Test control is environment-only

Conformance runs need states that ordinary operations cannot reach in bounded time, such as a provider restart or discarded deduplication history. The suite gets them from a **test-control channel** ([conformance](../../../conformance/README.md)).

That channel:

- Is a separate connection with its own protocol, served by a test launcher, never by the product endpoint.
- May restart the provider (preserving durable state), advance or discard deduplication generations, drop connections and set retention configuration.
- MUST NOT create, modify or delete subjects, commands, epochs or grants.

A provider's product endpoint MUST refuse control-channel method names with `method_not_found`.

## 14. What Core does not establish

- An acknowledgment is not execution, delivery, verification or acceptance.
- A missing response is not failure, and a closed session is not cancellation.
- A revision orders changes to one subject at one provider; it is not a global clock.
- Negotiated support for a feature says the provider implements it. It does not say the underlying system, such as a harness, can enforce it. Those limits are profile-specific capability facts.
