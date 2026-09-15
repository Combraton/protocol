#!/usr/bin/env python3
"""Check that every operation with a schema is documented in its profile document (M6, A7).

An operation is any `<name>.params`, `<name>.result` or `<name>.notification` schema under
schemas/<profile>/<major>/. It must appear, in backticks, in the profile document that owns it.
"""
import pathlib
import sys

DOCS = {
    "core": "docs/spec/profiles/CORE.md",
    "core-test": "docs/spec/profiles/CORE.md",
    "execution": "docs/spec/profiles/EXECUTION.md",
    "evidence": "docs/spec/profiles/EVIDENCE.md",
    "context": "docs/spec/profiles/CONTEXT.md",
    "knowledge": "docs/spec/profiles/KNOWLEDGE.md",
    "verification": "docs/spec/profiles/VERIFICATION.md",
}


def main():
    root = pathlib.Path(__file__).resolve().parent.parent
    failures = 0
    total = 0
    for profile_dir in sorted((root / "schemas").iterdir()):
        if not profile_dir.is_dir():
            continue
        for major_dir in sorted(p for p in profile_dir.iterdir() if p.is_dir()):
            operations = set()
            for schema in major_dir.glob("*.schema.json"):
                name = schema.name[: -len(".schema.json")]
                for suffix in (".params", ".result", ".notification"):
                    if name.endswith(suffix):
                        operations.add(name[: -len(suffix)])
            if not operations:
                continue
            document = DOCS.get(profile_dir.name)
            if document is None:
                print(f"schemas/{profile_dir.name}/{major_dir.name}: no profile document is registered")
                failures += 1
                continue
            text = (root / document).read_text(encoding="utf-8")
            for operation in sorted(operations):
                total += 1
                if f"`{operation}`" not in text:
                    print(f"{operation}: has schemas but is not documented in {document}")
                    failures += 1
    print(f"operations: {total} with schemas, {failures} undocumented")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
