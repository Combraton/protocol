# 007: Execution test controls outside the protocol

- **Status:** accepted with refinements on 2026-09-14. The owner accepted the overall approach and required three refinements before coding: exact race ordering, clock-file robustness, and isolation with coverage limits. They are written into this record.
- **Date:** 2026-09-14.
- **Owner/authority:** repository owner; Protocol session implements. [Issue #1](https://github.com/Combraton/protocol/issues/1), [M3 task](../work/release-0.1/M3.md).
- **Affects:**
  - [EXECUTION §15](../spec/profiles/EXECUTION.md#15-conformance-and-test-controls) and [CORE §13.1](../spec/profiles/CORE.md#131-test-control-is-environment-only);
  - the conformance runner, launch configuration, participant descriptors and fixture schema;
  - matrix rows EXE-3, EXE-6, EXE-8, EXE-10, EXE-19, OBS-7, OBS-9, TRN-4, REL-12, REL-13, SCN-2, SCN-3 and SCN-10.
- **Extends:** [decision 001](001-conformance-suite-architecture.md) (environment-only test control).

## Problem

M3 needs states that ordinary operations cannot reach on demand:
- a harness that acknowledges, only echoes, or only receives bytes;
- a crash between journal and dispatch;
- a late result from an old attempt;
- time passing during a session, for timeouts and idle grant expiry;
- a precise interleaving of two connections, to regress races such as the M2 idle re-check race.

M2 kept all test control in launch configuration and restarts, and fixed the clock per launch.

## Options

| Option | Assessment |
|---|---|
| **Environment controls** (selected): scripted fake harness, clock file, runner-driven process kills, declared barriers with provider signals, runner-side pipelined sends | Keeps the product protocol free of test operations. Portable fixtures use only portable controls. |
| Test-control protocol operations | Rejected: test methods leak into product endpoints (decision 001). |
| Sleeps and repeated random interleavings | Rejected: slow, flaky, and unable to show a race is gone. |
| Implementation unit tests only | Rejected as the only evidence; kept for private mechanisms. |

## Decision

### 1. Scripted executor

- The reference executor drives a deterministic fake harness whose script is selected by launch configuration.
- The vocabulary is normative only for the conformance tests that use it. A production executor may satisfy it through a test adapter and needs no production scripting engine.
- The script is never a protocol message.

### 2. Controllable clock

- **Configuration.** A fixture's start step asks for `clock: { "controlled": instant }`. The runner creates `<work>/test-controls/clock` holding that RFC 3339 UTC instant and launches the provider with `clock: { "file": path }`.
- **Atomic update.** Runner step `set_clock` writes the new instant to a temporary file in the same directory and renames it over `clock`, so a provider never reads a partial write.
- **Forward only.** The runner refuses a backward `set_clock` as a harness error, unless the step sets `allow_backward` to test provider robustness.
- **Provider handling:**
  - At launch, a missing or malformed clock file refuses the start with a nonzero exit.
  - During a run, the provider reads the file whenever it needs the time.
  - A missing or malformed file keeps the last good instant and writes a diagnostic to standard error. It never falls back to the system clock.
  - An instant earlier than the last good one is ignored with a diagnostic, so virtual time never moves backward.
- **Virtual versus real time.** Virtual time governs only protocol-visible decisions: expiry, timeouts, deadlines and `recorded_at`. Runner watchdogs, the provider's idle re-check interval and barrier watchdogs use real monotonic time. Freezing the virtual clock cannot hang a suite.

### 3. Process faults

Runner step `kill` sends SIGKILL to the provider process; `start` relaunches it over the same data directory.

### 4. Barriers and signals (implementation-specific)

- **Declaration.** A participant descriptor declares barrier names under `claims.test_barriers`. A fixture lists what it needs in `requires_barriers`. The start step's `barriers` enables them, and the runner passes `test_barriers: { "directory", "enabled" }` in launch configuration.
- **Pausing.** An enabled barrier pauses **once**, at its first hit:
  - the provider creates `<name>.reached`;
  - it waits until `<name>.release` exists;
  - a real-time provider watchdog of 30 s continues with a diagnostic if the runner never releases.
- **Signals.** A provider may also emit **signals**, files named `<signal>.signal` that mark a point without pausing. The reference emits `processing.lock.contended` when a request finds the processing lock held.
- **Runner steps:**
  - `await_barrier` waits for `.reached` under a real-time bound, then clears old signal files.
  - `release_barrier` creates `.release` atomically.
  - `await_any` waits until any named signal exists or all named responses have arrived, under a real-time bound. Expiry is a failure, never a pass.
- **Coverage limits.**
  - A participant that does not declare a required barrier or control reports `unsupported`. The runner records it in the result manifest's `coverage_limits`. It never counts as a pass.
  - Barrier names describe one implementation's internal points; they are evidence for that implementation only.

### 5. Pipelined sends

A `command` or `query` step with `"await": false` and a `name` sends the request without waiting. `expect_response` checks the named response later, whatever the arrival order; responses to other pending requests are buffered. Pipelining does not establish commit order. Fixtures that depend on order synchronize on signals or responses.

### 6. Isolation

- No operation, method, event type or error is reserved for testing.
- The clock file, barrier directory and script exist only in the runner's per-case work directory and are named only in the conformance launch configuration.
- A provider launched without a conformance launch configuration exposes none of these controls, and all barrier and signal code paths are inert.

## Exact ordering: the M2 idle re-check race

**The bug.** A subscriber's idle re-check did (a) re-authorize the subscription, then (b) read committed events and deliver them. It did not hold the processing lock, so a revoke and a later put could commit between (a) and (b), and the put reached a revoked subscriber. The fix holds the lock across (a) and (b).

**Setup.**
- The owner issues grant `g-1` to `agent-1`.
- Session **A** (`agent-1`) subscribes under `g-1` with `kinds: ["core-test.subject"]`.
- Barrier `subscription.recheck.after_authorization` sits between (a) and (b). It is enabled at start and first hit in A's post-response drain after subscribing.

| # | Runner | Correct provider (lock held from (a) through (b)) | Mutant `recheck-outside-lock` |
|---|---|---|---|
| 1 | `await_barrier` A, then clear signals | A has re-authorized `g-1` and pauses **holding** the lock | A has re-authorized `g-1` and pauses **without** the lock |
| 2 | Main: send revoke `g-1` (no wait) | The revoke's handler finds the lock held and emits signal `processing.lock.contended`, then blocks | The revoke commits and responds |
| 3 | Main: send put `s-1` (no wait) | Not yet read: the connection is busy with the blocked revoke | The put commits and responds |
| 4 | `await_any`: signal `processing.lock.contended`, **or** both responses (real-time bound 10 s) | Satisfied by the signal. No commit is awaited while the lock is held. | Satisfied by both responses. Both commits happened while A was paused. |
| 5 | `release_barrier` | A reads (b): nothing new; releases the lock. The revoke commits, then the put. | A reads (b): sees the put and delivers it |
| 6 | `expect_response` revoke and put | Both succeed | Both already succeeded |
| 7 | A: `expect_notification` within 2 s: `items: []`, `ended: authorization_lost` | A's next re-check sees the revocation and ends the subscription. The put, committed after the revoke, is never delivered. | **Fails**: the first notification carries the put (`params/ended` missing) |
| 8 | A: `expect_no_notification` | Passes | — |

**Why nothing here depends on timing:**
- In the correct provider, step 4 can only be satisfied by the signal: the revoke cannot commit while A holds the lock. The signal is emitted by a failed `try_lock`, a definite fact rather than an elapsed time.
- In the mutant, step 4 can only be satisfied by both responses. No signal is emitted, because nothing holds the lock, and both commits provably precede the release.
- Step 1 clears stale signals only after A is paused. While A is paused, no other session holds the lock, so no stale contention can appear.

**Bounded failure handling:**
- If neither condition happens within the bound, the case fails with the reason recorded.
- The provider's barrier watchdog guarantees it cannot stay paused forever.
- Shutdown kills the process if it does not exit.

## Consequences

- The M2 race has a deterministic regression; the mutant fails at step 7 with a stated reason, recorded by `check-mutants`.
- Idle grant expiry becomes a portable fixture on the Unix-socket binding. Grant expiry during a session becomes a portable fixture on stdio.
- Timeout fixtures run in milliseconds.
- To run clock fixtures, the independent implementation must implement the clock file and declare `clock.file` under `claims.test_controls`. Until then those fixtures are recorded as coverage limits for it. It need not implement barriers.

## Verification

- `check-fixtures` validates the new steps and rejects `requires_barriers` names no participant descriptor declares.
- `check-mutants` records each mutant's failing step and reason. A fixture may state an expected failing step and reason (`kill_expectations`); a mutant that fails elsewhere is reported as killed for the wrong reason.
- Mutants:
  - `recheck-outside-lock` fails the race regression at step 7;
  - `clock-file-ignored` fails the expiry fixtures;
  - `clock-follows-backward-time` and `clock-malformed-resets` fail the clock robustness fixture.
