# 005: Deduplication identity, binding and generations

- **Status:** accepted on 2026-09-13. Owner approval on 2026-09-13 ("yes things looks good"), with the instruction to merge PR #2. Changes need a new versioned decision.
- **Date:** 2026-09-13.
- **Owner/authority:** Protocol session under the Protocol 0.1 kickoff; [tracking issue #1](https://github.com/Combraton/protocol/issues/1).
- **Affects:** [Core §6 and §10](../spec/profiles/CORE.md#6-command-identity-and-idempotency); every command operation in every profile. Matrix rows CORE-7, CORE-8, CORE-9, CORE-10.
- **Supersedes:** nothing. Makes [SPEC §4](../spec/SPEC.md#4-command-processing) concrete.

## Problem

[SPEC §4](../spec/SPEC.md#4-command-processing) requires three things. A duplicate command returns its prior result. A conflicting payload under the same identity is `idempotency_conflict`. When the deduplication record has expired, the provider returns `dedupe_history_unavailable` and must "not guess that it is new and repeat a side effect".

Four questions follow:

1. What exactly makes two transmissions "the same command"?
2. When is a command identity bound?
3. How does a provider know an unknown identity might have been forgotten, rather than never seen?
4. Can the answer be tested without controlling wall-clock time?

## Evidence

**Common practice treats a pruned key as new, which SPEC forbids.** The [Stripe idempotent requests reference](https://docs.stripe.com/api/idempotent_requests), fetched 2026-09-13, says:

- "You can remove keys from the system automatically after they're at least 24 hours old. We generate a new request if a key is reused after the original is pruned."
- "The idempotency layer compares incoming parameters to those of the original request and errors if they're not the same."
- "If incoming parameters fail validation, or the request conflicts with another request that's executing concurrently, we don't save the idempotent result because no API endpoint initiates the execution. You can retry these requests."

**Parameter-mismatch errors are established practice.** The [Amazon EC2 idempotency guide](https://docs.aws.amazon.com/ec2/latest/devguide/ec2-api-idempotency.html), fetched 2026-09-13, returns `IdempotentParameterMismatch` when a client token is reused with different parameters, and scopes idempotency per Region or Availability Zone.

Inference from these sources:

- Comparing request content under a key (Stripe, EC2) is established practice, and so is not recording requests that never began executing (Stripe).
- Time-based pruning with treat-as-new is exactly the behavior SPEC rules out. Any time-to-live scheme also needs synchronized or trusted clocks to decide whether a token is too old.

## Decision

1. **Command intent digest.** Two transmissions are the same command when they have the same deduplication key and the same digest over these intent members: `operation`, `subject`, `preconditions`, `requires`, `payload`, and the required extensions.
   - Transmission metadata is excluded: message ID, transport ID, authority epoch, correlation, causation, optional extensions and generation. A reconnecting caller can retransmit under a new epoch without creating a conflict.
2. **Scope.** The deduplication key is (principal's deduplication scope, `command_id`). The same ID from another scope is unrelated and must not reveal the first scope's use. Principal scopes are specified with grants in M2.
3. **Binding on durable acceptance.** An identity is bound only when the owner transaction commits. Commands rejected before that are evaluated again on retransmission. This matches Stripe's "we don't save" rule for requests that never started.
4. **Order.** The deduplication lookup precedes preconditions and epoch checks. A retransmitted successful create returns its original acknowledgment even though a fresh evaluation would now fail.
5. **Generations instead of clocks.**
   - Each provider publishes a window `[oldest_retained, current]` of integer deduplication generations and retains every bound command issued under a generation in that window.
   - Callers persist each command with the generation current when they issued it.
   - An unknown command whose generation is below `oldest_retained` gets `dedupe_history_unavailable`. One above `current` is invalid. One inside the window is new.
   - Neither side needs a clock, and the provider can discard old records in bulk by generation.

## Alternatives

| Alternative | Why not selected |
|---|---|
| **Time-to-live keys with treat-as-new (Stripe)** | Violates SPEC; can repeat an opaque side effect after an outage. |
| **Time-to-live keys with a caller-supplied issue time** | Detects "possibly forgotten" but depends on clock agreement between processes and on callers not lying about time. |
| **Retaining every command identity forever as a compact tombstone** | Simple and exact, but grows without bound. SPEC allows tombstones for dangerous effects "until explicit lifecycle closure". Profiles may still require permanent tombstones for specific non-repeatable effects, in addition to generations. |
| **Provider-issued nonces per command** | Needs an extra round trip before every command and still has to expire nonces. |
| **Digest over the payload only** | Would let a retransmission with a different subject, operation or precondition return another command's result. |
| **Binding identities on rejection too** | Makes transient refusals such as overload permanent for that identity, and records unauthenticated or invalid traffic. |

## Consequences

- Callers must journal the generation with each command before sending it. A command journaled but never sent, and then delayed past retention, is refused as `dedupe_history_unavailable`. This is deliberately conservative: the caller must inspect state instead of assuming nothing happened.
- Providers must declare and honor a retention window. A provider that loses records inside its declared window is non-conforming. Fixtures detect the observable part: a restart that loses records within the window.
- Because rejected commands do not bind, a caller could reuse an identity after a rejection with different content. That is harmless because the first transmission had no effect. Callers SHOULD still use a fresh identity for a different intent.

## Verification

M1 fixtures cover:

- replay returns an equal acknowledgment and the applied count stays unchanged;
- conflicting intent;
- a retransmitted create after success;
- a retransmission under a new epoch;
- a generation above `current`;
- a restart that advances and discards generations, then a replay (`dedupe_history_unavailable`);
- a rejected command resent after the precondition is fixed.

The fixtures fail mutants that:

- re-execute duplicates;
- ignore the digest;
- dedupe by transport ID;
- treat forgotten identities as new;
- check preconditions before deduplication.
