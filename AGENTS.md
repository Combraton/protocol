# Protocol — working instructions

Build independent interoperability contracts, versioning/compatibility rules and positive/negative conformance fixtures for execution, evidence, knowledge and context profiles. Do not implement the product scheduler, memory engine, desktop or shared database. Contracts must support standalone PIO, CBR and other compatible callers, without Combraton-specific hidden authority.

## Read the right sources

Start with [README](README.md) and [the documentation map](docs/README.md), then [protocol spec](docs/spec/SPEC.md). Read the [accepted baseline](https://github.com/Combraton/combraton/blob/main/docs/architecture/BASELINE.md) and relevant shared/domain sections for boundary changes. Follow [the shared development workflow](https://github.com/Combraton/combraton/blob/main/docs/DEVELOPMENT.md); record its commit/revision for multi-session work. Research and old code are references, not silent overrides of accepted decisions.

## Preserve these boundaries

- One authoritative definition per boundary; shared semantics include identity, authority, observation and recovery, not only field shapes. Conceptual prose/examples are not released machine-readable schemas.
- Preserve explicit unsupported/unknown outcomes and capabilities. Never equate delivery, successful execution, verification and project acceptance.
- Coordinate one owner for a contract change with affected providers/consumers. Agree the required slice and distinguishing fixtures before parallel implementations rely on it.
- Use explicit versions and compatible rollout. Additive consumers can migrate independently; breaking changes need a migration path, not fictional atomic cross-repository merges.

## Current standalone-first milestone

Complete the agreed standalone release surface and conformance suite before dependent implementations rely on it. Keep normative fixtures here; benchmarks composes/version-pins them without redefining contracts. One PIO/CBR pass does not certify untested profiles. Follow [release gates](https://github.com/Combraton/combraton/blob/main/docs/STANDALONE-RELEASES.md) and [ADR 001](https://github.com/Combraton/combraton/blob/main/docs/decisions/001-standalone-first-and-evaluation.md). Comparative evaluation lives in [benchmarks](https://github.com/Combraton/benchmarks); product acceptance remains evidence-based.

## Work and coordination

Inspect the assigned issue/task, branch, head, worktree and uncommitted changes before editing. Preserve unrelated work. For a large task, persist a small plan with outcome, scope, acceptance, dependencies and next step in `docs/work/` or the linked issue; do not rely on chat alone. One owner per task; one isolated worktree per concurrent writer. Agree shared contracts before consumers diverge.

Use subagents when a bounded independent investigation or review will help; pass scope, relevant invariants, source revisions and expected evidence explicitly. Prefer read-only helpers. Parallel writers require separate worktrees and non-overlapping scope/resources. Collect and verify results. Use separate top-level sessions for independently owned component implementations; no recursive swarm or permanent model-to-repo assignment is required.

Changing schema/framing, digest/canonical encoding, idempotency, authority, capability negotiation, event ordering, compatibility or fixture meaning requires affected domain-spec review and primary-source validation. Record accepted choices and superseded sections in the owning [decision record](docs/decisions/README.md). Escalate a needed change of direction, authority or reserved judgment; routine scoped investigation and repair proceed automatically.

## Verify and hand off

Run `python3 scripts/check_docs.py` from the repository root for documentation changes; see [verification](docs/VERIFICATION.md). Product runtime/build/test commands do not exist yet: do not invent them or report product checks as passed. Add reproducible commands when implementation introduces them.

Future product validation must exercise positive and negative fixtures, duplicates, stale basis, unknown capabilities, missing required data and old/new supported combinations. Generated types alone do not prove semantic conformance.

Review the actual diff at recorded base/head. Before a session ends, persist commits/files, commands with exit status and evidence, unresolved facts, active resources and the next action in the task handoff. Treat old handoffs as historical observations; reconcile them with the checkout. Keep public records free of credentials and private transcripts.

Use existing native harnesses to ship v0.1. Combraton self-development is deferred until all four usable v0.1 releases. Do not install ECC/global hooks, select a model or relax runtime permissions merely because a reference suggests it.
