#!/usr/bin/env python3
"""Reproducible Protocol source release (v0.1.0 and later). Release tooling, not protocol.

Subcommands, run from a Git checkout of this repository (Python 3 standard library, `git`, `tar`):

  bundle          --commit REV --version V --out DIR
      Export the exact commit with `git archive`, add RELEASE-SOURCE.json (commit, tree, version) and
      BUNDLE-SHA256SUMS (every file of the bundle), and write a deterministic
      combraton-protocol-V-source.tar.gz plus a detached copy of BUNDLE-SHA256SUMS.

  verify-bundle   --archive FILE --work DIR [--full]
      Extract into an empty directory with no Git metadata and run the documented consumer path there:
      bundle checksums, normative inventory (file-system mode), documentation and operation checks,
      locked build and tests, runner self-test, fixture check and the reference provider over stdio.
      --full also runs the Unix-socket reference, the independent provider and both mutant checks.
      Writes verify-bundle.json with each command and its exit status.

  evidence        --run RUN_ID [--run RUN_ID ...] --out FILE
      Download the conformance artifacts of the named GitHub Actions runs (`gh run download`), redact
      run-scoped credentials (`ccred1.` strings), and write a deterministic tar.gz.

  finalize        --tag T --version V --commit SHA --tested-head SHA --ci-run ID [--ci-run ID ...] --assets DIR
      Write release-manifest.json (source commit, tree, tested head, versions, normative inventory
      digest, bundle digest, CI evidence) and release-notes.md (docs/release/0.1/NOTES.md with a
      generated header), then SHA256SUMS over every downloadable asset in DIR.

The normative inventory (docs/release/0.1/inventory.json, scripts/release_inventory.py) covers the files
consumers pin. BUNDLE-SHA256SUMS covers the complete distributed bundle, including the toolchain.
"""
import argparse
import gzip
import hashlib
import io
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
CREDENTIAL = re.compile(rb"ccred1\.[A-Za-z0-9_.-]+")


def sh(*argv, cwd=None, capture=False, check=True):
    print("+", " ".join(str(a) for a in argv), flush=True)
    result = subprocess.run([str(a) for a in argv], cwd=cwd, check=False, capture_output=capture)
    if check and result.returncode != 0:
        if capture:
            sys.stderr.write(result.stdout.decode(errors="replace") + result.stderr.decode(errors="replace"))
        raise SystemExit(f"command failed ({result.returncode}): {' '.join(str(a) for a in argv)}")
    return result


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def deterministic_targz(source_dir, prefix, out_path, mtime):
    """tar.gz of source_dir under prefix/: sorted entries, fixed owner and time, gzip without a timestamp."""
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.PAX_FORMAT) as tar:
        entries = sorted(p for p in pathlib.Path(source_dir).rglob("*"))
        for path in entries:
            rel = path.relative_to(source_dir).as_posix()
            info = tar.gettarinfo(str(path), arcname=f"{prefix}/{rel}")
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            info.mtime = mtime
            if info.isfile():
                info.mode = 0o755 if os.access(path, os.X_OK) else 0o644
                with open(path, "rb") as f:
                    tar.addfile(info, f)
            elif info.isdir():
                info.mode = 0o755
                tar.addfile(info)
            elif info.issym():
                tar.addfile(info)
    with open(out_path, "wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0, compresslevel=9) as gz:
            gz.write(buffer.getvalue())


def bundle_sums(directory):
    lines = []
    for path in sorted(pathlib.Path(directory).rglob("*")):
        if path.is_file() and not path.is_symlink():
            rel = path.relative_to(directory).as_posix()
            if rel == "BUNDLE-SHA256SUMS":
                continue
            lines.append(f"{sha256_file(path)}  {rel}")
    return "\n".join(lines) + "\n"


def cmd_bundle(args):
    commit = sh("git", "rev-parse", f"{args.commit}^{{commit}}", cwd=ROOT, capture=True).stdout.decode().strip()
    tree = sh("git", "rev-parse", f"{commit}^{{tree}}", cwd=ROOT, capture=True).stdout.decode().strip()
    commit_time = int(sh("git", "show", "-s", "--format=%ct", commit, cwd=ROOT, capture=True).stdout.decode().strip())
    name = f"combraton-protocol-{args.version}"
    out = pathlib.Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        stage = pathlib.Path(tmp) / name
        stage.mkdir()
        archive = sh("git", "archive", "--format=tar", commit, cwd=ROOT, capture=True).stdout
        subprocess.run(["tar", "-x", "-C", str(stage)], input=archive, check=True)
        source = {"format": "combraton-release-source/1", "name": name, "version": args.version, "commit": commit, "tree": tree,
                  "note": "Exported with git archive from this commit; BUNDLE-SHA256SUMS lists every other file of the bundle."}
        (stage / "RELEASE-SOURCE.json").write_text(json.dumps(source, indent=2) + "\n")
        sums = bundle_sums(stage)
        (stage / "BUNDLE-SHA256SUMS").write_text(sums)
        target = out / f"{name}-source.tar.gz"
        deterministic_targz(stage, name, target, commit_time)
        (out / f"{name}-BUNDLE-SHA256SUMS").write_text(sums)
    print(f"bundle: {target} sha256 {sha256_file(target)} ({len(sums.splitlines())} files listed)")


def cmd_verify_bundle(args):
    archive = pathlib.Path(args.archive).resolve()
    work = pathlib.Path(args.work).resolve()
    if work.exists() and any(work.iterdir()):
        raise SystemExit(f"{work} must be empty")
    work.mkdir(parents=True, exist_ok=True)
    sh("tar", "-xzf", archive, "-C", work)
    tops = [p for p in work.iterdir() if p.is_dir()]
    if len(tops) != 1:
        raise SystemExit("the archive must contain exactly one top-level directory")
    root = tops[0]
    if (root / ".git").exists():
        raise SystemExit("the extracted bundle unexpectedly contains Git metadata")
    record = {"format": "combraton-bundle-verification/1", "archive": archive.name, "archive_sha256": sha256_file(archive), "steps": []}

    def step(label, *argv):
        result = subprocess.run([str(a) for a in argv], cwd=root, capture_output=True)
        tail = (result.stdout + result.stderr).decode(errors="replace").strip().splitlines()[-3:]
        record["steps"].append({"step": label, "command": " ".join(str(a) for a in argv), "exit": result.returncode, "tail": tail})
        print(f"{'ok ' if result.returncode == 0 else 'FAIL'} {label}: {tail[-1] if tail else ''}", flush=True)
        if result.returncode != 0:
            (work / "verify-bundle.json").write_text(json.dumps(record, indent=2) + "\n")
            raise SystemExit(f"bundle verification failed at: {label}")

    listed = {}
    for line in (root / "BUNDLE-SHA256SUMS").read_text().splitlines():
        digest, rel = line.split("  ", 1)
        listed[rel] = digest
    present = {p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file() and p.name != "BUNDLE-SHA256SUMS"}
    mismatched = [rel for rel, digest in listed.items() if rel not in present or sha256_file(root / rel) != digest]
    extra = sorted(present - listed.keys())
    record["steps"].append({"step": "bundle checksums", "files": len(listed), "mismatched": mismatched, "extra": extra})
    if mismatched or extra:
        (work / "verify-bundle.json").write_text(json.dumps(record, indent=2) + "\n")
        raise SystemExit(f"bundle checksums differ: mismatched {mismatched[:5]}, extra {extra[:5]}")
    print(f"ok  bundle checksums: {len(listed)} files", flush=True)
    runner = "./target/debug/combraton-conformance"
    step("normative inventory", "python3", "scripts/release_inventory.py", "--verify")
    step("documentation", "python3", "scripts/check_docs.py")
    step("operations documented", "python3", "scripts/check_operations.py")
    step("build", "cargo", "build", "--workspace", "--locked")
    step("tests", "cargo", "test", "--workspace", "--locked")
    step("runner version", runner, "version")
    step("reference version", "./target/debug/combraton-reference-provider", "--version")
    step("encoding self-test", runner, "self-test")
    step("fixture check", runner, "check-fixtures")
    step("reference provider (stdio)", runner, "run", "--participant", "conformance/participants/reference-provider.json")
    if args.full:
        step("reference provider (Unix socket)", runner, "run", "--participant", "conformance/participants/reference-provider-unix.json", "--out", "conformance/results/reference-unix")
        step("independent provider", runner, "run", "--participant", "conformance/participants/independent-python-core.json", "--out", "conformance/results/independent-python-core")
        step("mutants (stdio)", runner, "check-mutants", "--participant", "conformance/participants/reference-provider.json")
        step("mutants (Unix socket, including client mutants)", runner, "check-mutants", "--participant", "conformance/participants/reference-provider-unix.json", "--out", "conformance/results/reference-unix")
    (work / "verify-bundle.json").write_text(json.dumps(record, indent=2) + "\n")
    print(f"bundle verification passed: {work / 'verify-bundle.json'}")


def cmd_evidence(args):
    out = pathlib.Path(args.out).resolve()
    with tempfile.TemporaryDirectory() as tmp:
        stage = pathlib.Path(tmp) / out.name.removesuffix(".tar.gz")
        stage.mkdir()
        runs = []
        for run in args.run:
            target = stage / f"run-{run}"
            sh("gh", "run", "download", run, "--dir", target, cwd=ROOT)
            info = json.loads(sh("gh", "run", "view", run, "--json", "headSha,event,conclusion,url,workflowName,createdAt", cwd=ROOT, capture=True).stdout)
            runs.append({"run": run, **info})
        redacted = 0
        for path in stage.rglob("*"):
            if path.is_file():
                data = path.read_bytes()
                new, count = CREDENTIAL.subn(b"ccred1.<redacted>", data)
                if count:
                    path.write_bytes(new)
                    redacted += count
        (stage / "EVIDENCE.json").write_text(json.dumps({"format": "combraton-release-evidence/1", "runs": runs, "credentials_redacted": redacted,
            "note": "GitHub Actions conformance artifacts: result manifests, transcripts, mutant summaries, compatibility and composition results. Run-scoped test credentials are redacted."}, indent=2) + "\n")
        deterministic_targz(stage, stage.name, out, 0)
    print(f"evidence: {out} sha256 {sha256_file(out)} ({redacted} credential strings redacted)")


def cmd_finalize(args):
    assets = pathlib.Path(args.assets).resolve()
    commit = sh("git", "rev-parse", f"{args.commit}^{{commit}}", cwd=ROOT, capture=True).stdout.decode().strip()
    tree = sh("git", "rev-parse", f"{commit}^{{tree}}", cwd=ROOT, capture=True).stdout.decode().strip()
    tested = sh("git", "rev-parse", f"{args.tested_head}^{{commit}}", cwd=ROOT, capture=True).stdout.decode().strip()
    tested_tree = sh("git", "rev-parse", f"{tested}^{{tree}}", cwd=ROOT, capture=True).stdout.decode().strip()
    if tree != tested_tree:
        raise SystemExit(f"the release commit's tree {tree} differs from the tested head's tree {tested_tree}")
    show = lambda path: sh("git", "show", f"{commit}:{path}", cwd=ROOT, capture=True).stdout
    inventory = json.loads(show("docs/release/0.1/inventory.json"))
    cargo = show("Cargo.toml").decode()
    version = re.search(r'^version = "([^"]+)"', cargo, re.M).group(1)
    if version != args.version:
        raise SystemExit(f"workspace version {version} differs from {args.version}")
    ci = []
    for run in args.ci_run:
        info = json.loads(sh("gh", "run", "view", run, "--json", "headSha,event,conclusion,url,workflowName", cwd=ROOT, capture=True).stdout)
        ci.append({"run": run, **info})
    name = f"combraton-protocol-{args.version}"
    bundle = assets / f"{name}-source.tar.gz"
    manifest = {
        "format": "combraton-release-manifest/1",
        "tag": args.tag,
        "version": args.version,
        "source_commit": commit,
        "source_tree": tree,
        "tested_head": tested,
        "tested_head_tree": tested_tree,
        "versions": {
            "protocol": "0.1",
            "profiles": {"core": 1, "execution": 1, "evidence": 1, "context": 1, "knowledge": 1, "verification": 1},
            "bindings": {"stream": 1, "encoding": 1},
            "combraton-conformance": version,
            "combraton-reference-provider": version,
            "combraton-independent-python-core": version,
            "thirdparty-minimal-executor": version,
            "thirdparty-minimal-publisher": version,
            "rust_toolchain": re.search(r'channel = "([^"]+)"', show("rust-toolchain.toml").decode()).group(1),
        },
        "normative_inventory": {"path": "docs/release/0.1/inventory.json", "files": inventory["totals"]["files"], "listing_sha256": inventory["totals"]["listing_sha256"]},
        "bundle": {"asset": bundle.name, "sha256": sha256_file(bundle), "file_checksums": f"{name}-BUNDLE-SHA256SUMS"},
        "ci": ci,
    }
    (assets / "release-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    notes = show("docs/release/0.1/NOTES.md").decode()
    header = (f"Tag `{args.tag}` → commit `{commit}` (tree `{tree}`), the verified merge of the tested head `{tested}` (same tree).\n\n"
              f"- Normative inventory: {inventory['totals']['files']} files, listing SHA-256 `{inventory['totals']['listing_sha256']}`\n"
              f"- Source bundle: `{bundle.name}`, SHA-256 `{manifest['bundle']['sha256']}`\n"
              + "".join(f"- CI: [{c['workflowName']} run {c['run']}]({c['url']}) on `{c['headSha'][:12]}` ({c['event']}): {c['conclusion']}\n" for c in ci)
              + "\nEvery asset's SHA-256 is in `SHA256SUMS`; `release-manifest.json` records the same facts machine-readably.\n\n")
    (assets / "release-notes.md").write_text(header + notes)
    names = sorted(p.name for p in assets.iterdir() if p.is_file() and p.name != "SHA256SUMS")
    (assets / "SHA256SUMS").write_text("".join(f"{sha256_file(assets / n)}  {n}\n" for n in names))
    print((assets / "SHA256SUMS").read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("bundle"); p.add_argument("--commit", required=True); p.add_argument("--version", required=True); p.add_argument("--out", required=True)
    p = sub.add_parser("verify-bundle"); p.add_argument("--archive", required=True); p.add_argument("--work", required=True); p.add_argument("--full", action="store_true")
    p = sub.add_parser("evidence"); p.add_argument("--run", action="append", required=True); p.add_argument("--out", required=True)
    p = sub.add_parser("finalize")
    for flag in ("--tag", "--version", "--commit", "--tested-head", "--assets"):
        p.add_argument(flag, required=True)
    p.add_argument("--ci-run", action="append", required=True)
    args = parser.parse_args()
    {"bundle": cmd_bundle, "verify-bundle": cmd_verify_bundle, "evidence": cmd_evidence, "finalize": cmd_finalize}[args.command](args)


if __name__ == "__main__":
    main()
