# 007: Execution test controls outside the protocol

- **Status:** proposed on 2026-09-14, for owner review with the M3 contract.
- **Date:** 2026-09-14.
- **Owner/authority:** repository owner; Protocol session proposes. [Issue #1](https://github.com/Combraton/protocol/issues/1), [M3 task](../work/release-0.1/M3.md).
- **Affects:** [EXECUTION §15](../spec/profiles/EXECUTION.md#15-conformance-and-test-controls), [CORE §13.1](../spec/profiles/CORE.md#131-test-control-is-environment-only), conformance runner, launch configuration and participant descriptors; matrix rows EXE-3, EXE-6, EXE-8, EXE-10, EXE-19, OBS-7, TRN-4, OBS-9, REL-12, REL-13, SCN-2, SCN-3, SCN-10.
- **Extends:** [decision 001](001-conformance-suite-architecture.md) (environment-only test control).

## Problem

M3 needs states that ordinary operations cannot reach on demand:
- a harness that acknowledges, only echoes, or only receives bytes;
- a crash between journal and dispatch;
- a late result from an old attempt;
- a lost acknowledgment or a refused cancellation;
- time passing during a session, for timeouts and idle grant expiry;
- a precise interleaving of two connections, to regress races such as the M2 idle re-check race.

M2 kept all test control in launch configuration and process restarts, and fixed the clock per launch. That cannot express faults inside a session or time advancing in one.

## Options

| Option | Summary | Assessment |
|---|---|---|
| **Environment controls** (selected) | Scripted fake harness in launch configuration; a clock file the runner rewrites; runner-driven process kills; optional named barriers declared by a participant; a runner step that sends without waiting for the response | Keeps the product protocol free of test operations. Portable fixtures use only the portable controls. |
| Test-control protocol operations | `test.clock.set`, `test.fault.inject` methods | Rejected. Test methods leak into product endpoints and invite authority confusion, which decision 001 forbids. |
| Wall-clock sleeps and random interleavings | Real waiting and repeated runs | Rejected. Slow, flaky, and unable to prove a race is gone. |
| Only implementation unit tests | Faults tested inside each implementation | Rejected as the only evidence: a black-box claim needs black-box fixtures. It remains appropriate for private mechanisms, as in M2's reference-only tests. |

## Decision (proposed)

1. **Scripted executor.**
   - The reference provider implements `execution/1` over a deterministic fake harness.
   - Launch configuration selects a script of harness behaviors per submission. The vocabulary is fixed in M3 fixtures and documented in the conformance README.
   - A third-party executor that wants to run fault fixtures implements the same script vocabulary as its test harness. The script is never a protocol message.
2. **Controllable clock.**
   - Launch configuration key `clock.file` names a file in the runner's work directory holding one RFC 3339 UTC instant. The provider reads its clock from that file whenever it needs the time.
   - Runner step `set_clock` rewrites the file. Fixtures only move time forward.
   - `clock.fixed` remains for fixtures that need no movement.
3. **Process faults.** Runner step `kill` sends SIGKILL to the provider process; `start` relaunches it over the same data directory. A fault "between journal and dispatch" is reached by a script behavior that stops at that boundary, or by a barrier.
4. **Barriers, implementation-specific.**
   - A participant descriptor may declare `test_barriers`, a list of names. Launch configuration then enables selected barriers. The provider pauses at a barrier by creating `<name>.reached` in a runner-owned directory, and continues when `<name>.release` appears.
   - Fixtures using barriers list them in `requires_barriers`. Participants that do not declare them report `unsupported`.
   - Barrier names describe one implementation's internal points; they are evidence for that implementation, not portable requirements.
5. **Pipelined sends.** Runner steps `send` (send a request, do not wait) and `expect_response` (wait for the response to a named earlier send, in any order). This also serves the M3 pipelined-request fixtures.
6. **No protocol surface.** No operation, method, event type or error is reserved for testing. A provider launched without a conformance launch configuration exposes none of these controls.

## Consequences

- The M2 idle re-check race gets a deterministic regression. A barrier inside the reference's re-check holds the subscriber at the dangerous point while a pipelined revoke and put run on another connection. The broken ordering then deterministically delivers the put.
- Idle grant expiry becomes a portable fixture on the Unix-socket binding: the runner moves the clock past `expires_at` while the subscriber is idle.
- Timeout fixtures (EXE-19) run in milliseconds rather than waiting.
- The independent implementation must implement the script vocabulary, the clock file and pipelined behavior to run M3 fixtures. It need not implement barriers.

## Verification

- Every M3 fault fixture uses only these controls.
- `check-fixtures` rejects `requires_barriers` fixtures that lack a declaring participant.
- A mutant that ignores `clock.file` fails the timeout and idle-expiry fixtures.
- The barrier regression fails against a mutant that reintroduces the unlocked re-check.
