# Claude Code in Protocol

@AGENTS.md

The imported file is this repository's common working agreement. Follow its scope, invariants, verification and handoff requirements; the specification links are reading pointers, not instructions to preload every document.

When planning a contract change, explicitly list provider and consumer assumptions and counterexamples. Do not invent fields from the UI mock or copy an external protocol without analyzing ownership and recovery semantics.

For a large ambiguous task, inspect the relevant sources and make a bounded plan before implementation. For ordinary scoped fixes, proceed without approval rituals. Give subagents explicit relevant constraints and require source-linked results; do not assume their context contains every instruction you read.

When reading or editing another repository, explicitly read its root instructions and relevant specification. Directory access does not guarantee instruction loading. Keep one task owner and preserve other worktrees.

After changing instruction files, confirm the next session loaded them using the installed client's context inspection. Do not use `/init` to replace reviewed instructions with generic generated text.

Complete the agreed standalone release surface and conformance suite before dependent implementations rely on it. Keep normative fixtures here; benchmarks composes/version-pins them without redefining contracts. One PIO/CBR pass does not certify untested profiles. Follow the current standalone-first milestone in the imported instructions; earlier research recommending early desktop integration is superseded.
