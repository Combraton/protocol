#!/usr/bin/env python3
"""Different-OS-user check for the Unix-socket binding (STREAM section 6, decision 006).

The provider must close a connection from another operating-system user without sending a
frame. The socket directory is 0700, so an ordinary second user cannot even reach the socket.
The check therefore connects as root via passwordless `sudo -n`: root bypasses directory
permissions, so the provider's own peer-credential check (SO_PEERCRED on Linux, getpeereid on
macOS) is what refuses it. A same-user client in the same run is the positive control.

Outcomes are written to <out>/result.json and kept distinct:
- pass: the other-user client got the expected behavior and the same-user control got a response;
- fail: it did not;
- unsupported: this host cannot run the check (no passwordless sudo, or already running as root).

Exit status: 0 pass; 1 fail, or unsupported with --require; 77 unsupported otherwise.

Usage (from the repository root, after `cargo build`):
  python3 conformance/scripts/peer_user_check.py --out conformance/results/peer-user
  python3 conformance/scripts/peer_user_check.py --mutant skip-peer-check --expect accepted \\
      --out conformance/results/peer-user-mutant
"""
import argparse
import json
import os
import platform
import shutil
import socket
import subprocess
import sys
import tempfile
import time

CLIENT = r"""
import json, os, socket, sys
path = sys.argv[1]
frame = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "core.describe",
                    "params": {"operation": "core.describe", "message_id": "peer-1", "payload": {}}}) + "\n"
received = b""
closed = False
try:
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(3)
    s.connect(path)
    try:
        s.sendall(frame.encode())
    except OSError:
        pass
    try:
        while b"\n" not in received:
            chunk = s.recv(65536)
            if not chunk:
                closed = True
                break
            received += chunk
    except socket.timeout:
        pass
    except OSError:
        closed = True
    connected = True
except OSError as error:
    connected = False
    received = str(error).encode()
print(json.dumps({"uid": os.geteuid(), "connected": connected, "closed": closed,
                  "bytes": len(received) if connected else 0,
                  "response_is_frame": connected and received.startswith(b"{")}))
"""


def run_client(path, as_root):
    command = [sys.executable, "-c", CLIENT, path]
    if as_root:
        command = ["sudo", "-n"] + command
    completed = subprocess.run(command, capture_output=True, text=True, timeout=20)
    try:
        return json.loads(completed.stdout.strip().splitlines()[-1])
    except (IndexError, json.JSONDecodeError):
        return {"error": completed.stderr.strip() or completed.stdout.strip(), "exit": completed.returncode}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", default=".")
    parser.add_argument("--provider", default="target/debug/combraton-reference-provider")
    parser.add_argument("--mutant", action="append", default=[])
    parser.add_argument("--expect", choices=["refused", "accepted"], default="refused")
    parser.add_argument("--out", default="conformance/results/peer-user")
    parser.add_argument("--require", action="store_true", help="treat an unsupported host as failure")
    args = parser.parse_args()
    os.makedirs(args.out, exist_ok=True)
    result = {"format": "combraton-peer-user-check/1", "os": platform.system(), "arch": platform.machine(),
              "provider": args.provider, "mutants": args.mutant, "expect": args.expect}

    def finish(outcome, **fields):
        result.update(fields, outcome=outcome)
        with open(os.path.join(args.out, "result.json"), "w") as handle:
            json.dump(result, handle, indent=2)
            handle.write("\n")
        print(f"peer-user-check: {outcome}" + (f" ({fields.get('reason')})" if fields.get("reason") else ""))
        if outcome == "pass":
            return 0
        if outcome == "unsupported" and not args.require:
            return 77
        return 1

    if os.geteuid() == 0:
        return finish("unsupported", reason="running as root; the provider must run as an ordinary user")
    if shutil.which("sudo") is None or subprocess.run(["sudo", "-n", "true"], capture_output=True).returncode != 0:
        return finish("unsupported", reason="passwordless sudo is not available to act as a different user")

    work = tempfile.mkdtemp(prefix="cpu", dir="/tmp")  # short path: macOS limits socket paths to 104 bytes
    provider = None
    try:
        socket_dir = os.path.join(work, "s")
        os.mkdir(socket_dir, 0o700)
        os.chmod(socket_dir, 0o700)
        data_dir = os.path.join(work, "data")
        os.mkdir(data_dir)
        config = os.path.join(work, "config.json")
        with open(config, "w") as handle:
            json.dump({"format": "combraton-conformance-config/1", "principal": "owner"}, handle)
        path = os.path.join(socket_dir, "p.sock")
        argv = [os.path.join(args.repo, args.provider), "--data-dir", data_dir, "--config", config,
                "--schemas", os.path.join(args.repo, "schemas"), "--socket", path]
        for mutant in args.mutant:
            argv += ["--mutant", mutant]
        stderr_path = os.path.join(args.out, "provider-stderr.log")
        with open(stderr_path, "w") as stderr:
            provider = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=stderr)
        deadline = time.time() + 10
        while not os.path.exists(path):
            if provider.poll() is not None or time.time() > deadline:
                return finish("fail", reason="provider did not create its socket", provider_exit=provider.poll())
            time.sleep(0.05)
        control = run_client(path, as_root=False)
        other = run_client(path, as_root=True)
        result.update(provider_uid=os.geteuid(), same_user=control, other_user=other)
        if not control.get("response_is_frame"):
            return finish("fail", reason="same-user control received no response frame")
        if other.get("uid") == os.geteuid() or "uid" not in other:
            return finish("fail", reason="could not run the client as a different user")
        if args.expect == "refused":
            refused = other.get("connected") and other.get("bytes") == 0 and other.get("closed")
            return finish("pass" if refused else "fail",
                          reason=None if refused else "a different user's connection was not closed without a frame")
        accepted = bool(other.get("response_is_frame"))
        return finish("pass" if accepted else "fail",
                      reason=None if accepted else "expected the mutant to answer a different user")
    finally:
        if provider is not None:
            try:
                provider.stdin.close()
                provider.wait(timeout=5)
            except (OSError, subprocess.TimeoutExpired):
                provider.kill()
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
