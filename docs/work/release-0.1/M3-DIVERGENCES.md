# M3 divergences — independent pass resolution record

- **Task:** M3 step 8 of [Protocol 0.1](PLAN.md), [issue #1](https://github.com/Combraton/protocol/issues/1).
- **Source:** section G of the independent implementation's [divergence log](../../../conformance/independent/python-core/DIVERGENCES.md). A spec-only helper wrote it from the documents at `d81e49b`, without reading the reference provider, the runner source or the cross-checks.
- **Result before resolution:** the helper's first complete run was 176 pass, 19 fail, 4 unsupported, 12 skipped. It then fixed two implementation mistakes of its own (G-EXIT-RUNTIME, G-COALESCE-ADJACENT). The final run was 181 pass, 14 fail, 4 unsupported, 12 skipped, with the same 14 failures in each of three repeats.

Each entry is resolved in the specification, the fixtures, the reference provider, or more than one of these. Nothing was resolved by changing a document to match an implementation without a reason in the contract.

## A. Real defects

| Tag | Defect | Resolution |
|---|---|---|
| G-RECOVERY-HOST-EVENT | The stale-dispatcher fixture required an exact event list without `execution.host.changed`, although recovery advances the host generation and EXECUTION §9 requires the event. The reference provider had the same omission. | **reference:** recovery that advances the generation appends `execution.host.changed` after its other events. **fixtures:** `execution.stale-dispatcher-is-fenced-after-recovery` v2 and `execution.crash-during-dispatch-is-reconciled-without-resending` v2 list it. **mutant:** `recovery-host-change-silent`. **spec:** EXECUTION §9 row and §15.1 event order. |
| G-IDLE-EXPIRY-FIXED-CLOCK | CORE §16.5 still said the launch clock is fixed for a process's lifetime, contradicting the clock file | **spec:** sentence removed |
| G-EFFECT-HISTORY-CODE | `effect_history_unavailable` was named in CORE §19.2 but missing from the error registry and schema, so no provider could send it | **spec:** CORE §12 row (`after_reconcile`). **schema:** `error-data.schema.json`. **reference and runner:** retry class. |

## B. Contradictions or gaps between documents

| Tag | Resolution |
|---|---|
| G-EPOCH-ABSENT | **spec:** CORE §8 lets a profile whose epoch exists only once claimed treat an absent epoch as epoch 0, as EXECUTION §11.3 does |
| G-REFUSED-AXES | **spec:** EXECUTION §3.1 — a refused execution's `delivery` is `failed_before_delivery`, and its other axes keep their initial values; a queued execution's `delivery` is `pending`. **reference:** refusal at submit and by queue timeout now sets it. **fixtures:** `execution.admission-refuses-unenforceable-or-unknown-requirements` v2, `execution.context-bindings-gate-admission` v2. |
| G-EVENT-PROOF-CLASS | **spec:** EXECUTION §9 — `proof_class` appears only when the evidence is a proof class |
| G-CORRELATION-EVENTS | **matrix:** EXE-20 now says correlation is returned by `execution.inspect`, not by events, matching EXECUTION |
| G-TIMEOUT-SCOPE | **spec:** EXECUTION §8 states when each timeout applies. **reference:** the delivery timeout now applies only while delivery is `pending` (it previously also applied to `ambiguous`, which the reconciliation timeout covers). |

## C. Fixture expectations the documents did not state

The fixtures hard-coded identifiers, evidence class names, script-step meanings and event orders that the contract leaves to executors. Under owner decision Q4, the scripted vocabulary is normative only for its conformance tests, so these are now **conformance executor conventions** in [EXECUTION §15.1](../../spec/profiles/EXECUTION.md#151-conformance-executor-conventions). A production executor may choose differently, and callers must not depend on them.

| Tag | Now stated in §15.1 |
|---|---|
| G-OBLIGATION-ID, G-LEASE-ID | Record, effect and obligation identifiers, including `<effect>.evidence` and `<execution>.workspace` |
| G-EVIDENCE-NAMES, G-DISPATCH-INTENT | Evidence classes, including the `dispatch_intent` observation for the write-ahead marker |
| G-DELIVER-STEP, G-STALL | `deliver` is one dispatch attempt and has no effect once delivery is not `pending`; `stall` never dispatches; `exit` sets runtime `exited` |
| G-OVERDUE-ORDER | Event order at a delivery timeout and at recovery |
| G-ACTIONS-ABSENT, G-RESPONSE-STATUS | `actions` and `steering` appear once they have an entry; answered action responses are acknowledged by the scripted harness |

Also recorded there: effect kinds, retry attempt limits and capacity release. Values the fixtures depend on and the helper matched by coincidence, such as `<execution>.delivery-1`, are covered by the same identifiers table.

## D. Fixture changed because it asserted something the contract allows either way

| Tag | Resolution |
|---|---|
| G-NOTIFY-BATCH | CORE §16.5 lets a provider split items across notifications. `execution.watch-execution-facts-with-subscriptions` v2 uses a script that does not dispatch, so exactly one event follows the submit. |

## E. Decisions recorded by the helper and left open

These are unguarded choices the contract allows. They are recorded, not required:
- G-CLOCK-READING, G-CLOCK-RESTART: when the clock file is read, and whether the last good instant survives a restart;
- G-EXEC-REVISION: how much an execution's revision rises per script step (fixtures capture revisions);
- G-ADMISSION-EVENT: `delivery_id` on a direct admission event;
- G-INSPECT-GATING, G-ADAPTER-PREDICATES: feature members of inspect for sessions without the feature, and adapter predicates in `core.capabilities`;
- G-CAPACITY beyond the conformance convention; G-CRASH-EXIT exit status; G-CRASH-TIMING.

## F. Coverage limits

- **Clock file.** `core.events.subscription-ends-at-grant-expiry` and `core.grants.test-clock-never-moves-backward` were coverage limits for the independent participant. It now implements `clock.file`, and both run and pass. These limits are **resolved**.
- **Backpressure signals, barriers and the Unix socket.** They remain coverage limits for the independent participant: its runs report them as `unsupported` or `skipped`, never as passes.

## G. Follow-up

The helper realigns its implementation with the resolved documents, in a further spec-only pass on the same branch, rebased onto the resolution commit.
