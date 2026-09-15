#!/usr/bin/env python3
"""Build an accepted older release of this repository for compatibility checks (M6, A1).

The pinned tree is exported with `git archive` into target/pinned-<name>/src (no worktree is
registered) and the reference provider is built into target/pinned-<name>/build. The runner then
uses conformance/participants/pinned/<name>-reference-provider*.json for the older provider, and
`--fixtures target/pinned-<name>/src/conformance/fixtures` to run the older fixture set, unmodified,
against the current providers.

Test tooling only; nothing here is part of the protocol.
"""
import argparse
import pathlib
import subprocess
import sys

PINNED = {
    # Accepted M5 merge (PR #6): the last accepted state before core.feature_dependencies.
    "m5": "6ed4727ac4d92942a52a9cc2d78d6d4630cf6cce",
}


def run(*argv, cwd=None):
    print("+", " ".join(argv), flush=True)
    subprocess.run(argv, cwd=cwd, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("name", choices=sorted(PINNED))
    parser.add_argument("--repo", default=".")
    args = parser.parse_args()
    repo = pathlib.Path(args.repo).resolve()
    commit = PINNED[args.name]
    root = repo / "target" / f"pinned-{args.name}"
    src = root / "src"
    have = subprocess.run(["git", "cat-file", "-e", f"{commit}^{{commit}}"], cwd=repo).returncode == 0
    if not have:
        run("git", "fetch", "--no-tags", "--depth=1", "origin", commit, cwd=repo)
    if not (src / ".pinned-commit").exists() or (src / ".pinned-commit").read_text().strip() != commit:
        subprocess.run(["rm", "-rf", str(src)], check=True)
        src.mkdir(parents=True)
        archive = subprocess.run(["git", "archive", "--format=tar", commit], cwd=repo, check=True, capture_output=True).stdout
        subprocess.run(["tar", "-x", "-C", str(src)], input=archive, check=True)
        (src / ".pinned-commit").write_text(commit + "\n")
    run("cargo", "build", "--locked", "-p", "combraton-reference-provider", "--target-dir", str(root / "build"), cwd=src)
    print(f"pinned {args.name}: {commit} built at {root / 'build' / 'debug'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
