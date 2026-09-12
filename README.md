# Combraton Protocol

Independent semantic contracts for controllers, agentic execution, evidence and context services.

> Bootstrap documentation only. No product runtime, released API, installation command or performance claim is established here. The reviewed architecture is `architecture-v1-20260912`, published as `public-development-v1-20260913`. Canonical specifications are available through [the documentation map](docs/README.md). This README is an overview, not the full specification.

The protocol connects independently useful systems without requiring Combraton's desktop, internal database, programming language or scheduler. It owns no execution engine or memory store. Other applications can implement only the profiles they need, negotiating required semantics explicitly.

## Profiles

| Profile | Responsibility |
|---|---|
| Core | Identity, versions, command causality, scoped authority, errors and observation cursors |
| Execution | Attempt/delivery binding, lifecycle, cancellation, reconciliation and usage |
| Evidence | Artifact descriptors, sealing, provenance, retrieval and retention |
| Knowledge | Claim revisions, applicability, conflicts and scoped reliance |
| Context | Request basis, obligations, timing, budgets, packets, gaps and delivery bindings |
| Verification | Subjects, contracts, evaluator results, covered properties and validity |
| Coordination | Optional project/workflow/template/history semantics for controllers |
| Remote trust | Negotiated identity, delegation, revocation and transfer across trust boundaries |

Profiles are architectural responsibilities. Concrete schemas, operation names and compatibility fixtures are not yet published. Start with the small profile subset needed by the first execution/evidence/context flow; do not design every optional method before integration.

## Participants and independence

- [PIO](https://github.com/Combraton/pio) implements execution semantics and can run standalone.
- [CBR](https://github.com/Combraton/cbr) implements evidence/knowledge/context semantics and can run standalone.
- [Combraton](https://github.com/Combraton/combraton) composes the services and owns project direction/readiness/acceptance.
- Other executors, memory providers, observers and control planes can implement corresponding profiles without importing product internals.

PIO and CBR may communicate directly under an explicit caller binding. Direct communication does not transfer project authority. MCP/ACP/native APIs remain integration options at their appropriate boundaries, not replacements for durable cross-product semantics.

## Contract invariants

- One command identity cannot name conflicting payloads.
- Observation, acknowledgment, execution completion and acceptance are distinct facts.
- Unknown required semantics are rejected; absence of observation is not proof of success.
- Source frontiers describe observed coverage, not an atomic world snapshot.
- Packets bind exact content, relevant source/environment basis, required items and declared omissions.
- Context timing does not alter authority/reliance; expiry cannot waive a required condition.
- Retries reconcile ambiguous external effects; opaque actions are not universally exactly-once.
- No shared writable database is required between participants.

## First milestone

Define a minimal versioned schema slice and deterministic conformance fixtures for work submission, execution identity, evidence sealing and context binding. Include duplicate/conflicting requests, stale ownership, missing required context, incomplete evidence, late delivery and absent capabilities. Validate small independent clients before product code depends on the wire contract.

Keep schemas language-neutral, with separately versioned bindings and a tested consumer compatibility matrix. Local JSON-RPC over bounded authenticated transport is a starting preference; encoding, framing and digest rules must be specified and tested.

See [BOOTSTRAP](https://github.com/Combraton/combraton/blob/main/BOOTSTRAP.md). This bootstrap contains no generated SDK or selected license; the repository is public and its project license remains to be selected.

## Working on this repository

Read [AGENTS.md](AGENTS.md), [CLAUDE.md](CLAUDE.md), [the documentation map](docs/README.md), and [verification](docs/VERIFICATION.md). Use existing native harnesses for development. **Combraton self-development is deferred until usable v0.1 releases of all four projects.** Public visibility does not select a license; no project license has been added yet.
