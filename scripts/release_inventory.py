#!/usr/bin/env python3
"""Protocol 0.1 release inventory (M6, A6): per-file SHA-256 checksums of the files consumers pin.

  python3 scripts/release_inventory.py            # write docs/release/0.1/inventory.json
  python3 scripts/release_inventory.py --verify   # check the working tree against it (exit 1 on any difference)

Files are listed from Git (`git ls-files`), so untracked or ignored files never enter the inventory.
Checksums are over the exact committed bytes; the aggregate digest is SHA-256 over the listing's
canonical JSON, so any added, removed or changed file changes it.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
import sys

SCOPE = [
    "LICENSE",
    "docs/spec/",
    "schemas/",
    "conformance/schemas/",
    "conformance/fixtures/",
    "conformance/vectors/",
]
INVENTORY = "docs/release/0.1/inventory.json"


def listing(root):
    tracked = subprocess.run(["git", "ls-files", "-z"], cwd=root, check=True, capture_output=True).stdout.decode().split("\0")
    paths = sorted(p for p in tracked if p and any(p == s or (s.endswith("/") and p.startswith(s)) for s in SCOPE))
    files = []
    for path in paths:
        data = (root / path).read_bytes()
        files.append({"path": path, "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)})
    return files


def document(files):
    canonical = json.dumps(files, separators=(",", ":"), sort_keys=True, ensure_ascii=False).encode()
    return {
        "format": "combraton-release-inventory/1",
        "release": "0.1",
        "scope": SCOPE,
        "files": files,
        "totals": {"files": len(files), "bytes": sum(f["bytes"] for f in files), "listing_sha256": hashlib.sha256(canonical).hexdigest()},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    root = pathlib.Path(__file__).resolve().parent.parent
    current = document(listing(root))
    target = root / INVENTORY
    if not args.verify:
        target.write_text(json.dumps(current, indent=2) + "\n", encoding="utf-8")
        print(f"inventory: {current['totals']['files']} files, listing sha256 {current['totals']['listing_sha256']}")
        return 0
    recorded = json.loads(target.read_text(encoding="utf-8"))
    old = {f["path"]: f for f in recorded["files"]}
    new = {f["path"]: f for f in current["files"]}
    problems = []
    for path in sorted(old.keys() | new.keys()):
        if path not in new:
            problems.append(f"missing: {path}")
        elif path not in old:
            problems.append(f"not in inventory: {path}")
        elif old[path]["sha256"] != new[path]["sha256"]:
            problems.append(f"changed: {path}")
    for problem in problems:
        print(problem)
    same = not problems and recorded["totals"]["listing_sha256"] == current["totals"]["listing_sha256"]
    print(f"inventory verify: {current['totals']['files']} files, listing sha256 {current['totals']['listing_sha256']}: {'ok' if same else 'DIFFERS'}")
    return 0 if same else 1


if __name__ == "__main__":
    sys.exit(main())
