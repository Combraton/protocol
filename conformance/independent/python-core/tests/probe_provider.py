"""Probes of M1 behavior the fixtures do not exercise (see DIVERGENCES.md).

Drives provider.py over its stdio binding. Each probe asserts the behavior
this implementation chose; the choice and its spec basis are recorded in
DIVERGENCES.md under the probe's tag.

Usage: python3 conformance/independent/python-core/tests/probe_provider.py
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
IMPL = os.path.dirname(HERE)
sys.path.insert(0, IMPL)
import valuedomain as V  # noqa: E402

PROVIDER = os.path.join(IMPL, "provider.py")
SUBJ = {"kind": "core-test.subject", "id": "s-1"}
AUTH = {"kind": "core-test.authority", "id": "core-test"}


class Conn:
    def __init__(self, data_dir: str, config: dict | None = None):
        cfg = {"format": "combraton-conformance-config/1", "principal": "probe"}
        cfg.update(config or {})
        self.cfg_path = os.path.join(data_dir, "config.json")
        with open(self.cfg_path, "wb") as fh:
            fh.write(V.canonical(cfg))
        self.proc = subprocess.Popen(
            [sys.executable, PROVIDER, "--data-dir", os.path.join(data_dir, "data"), "--config", self.cfg_path],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        )
        self.next_id = 1

    def raw(self, data: bytes):
        self.proc.stdin.write(data)
        self.proc.stdin.flush()

    def read(self):
        line = self.proc.stdout.readline()
        return V.loads(line.decode("utf-8")) if line else None

    def request(self, method: str, params: dict):
        rid = self.next_id
        self.next_id += 1
        self.raw(V.canonical({"jsonrpc": "2.0", "id": rid, "method": method, "params": params}) + b"\n")
        return self.read()

    def query(self, operation: str, payload: dict, **extra):
        params = {"operation": operation, "message_id": f"m-{self.next_id}", "payload": payload}
        params.update(extra)
        return self.request(operation, params)

    def negotiate(self, profiles=None, core_features=None):
        profiles = profiles or [{"name": "core-test", "majors": [1], "required": True,
                                 "required_features": [], "optional_features": []}]
        if core_features is not None:
            profiles = profiles + [{"name": "core", "majors": [1], "required": True,
                                    "required_features": core_features, "optional_features": []}]
        return self.query("core.negotiate", {"caller": {"name": "probe", "version": "1"},
                                             "receive_limits": {"max_frame_bytes": 1048576},
                                             "profiles": profiles})

    def command(self, operation, command_id, subject, preconditions, payload, algorithm="sha256", **extra):
        env = {"operation": operation, "message_id": f"m-{self.next_id}", "command_id": command_id,
               "dedupe_generation": 0, "subject": subject, "preconditions": preconditions,
               "requires": [], "payload": payload}
        env.update(extra)
        intent = {k: env[k] for k in ("operation", "subject", "preconditions", "requires", "payload")}
        intent["extensions"] = {k: v for k, v in env.get("extensions", {}).items() if k in env["requires"]}
        env.setdefault("command_digest", V.digest(algorithm, V.canonical(intent)))
        return self.request(operation, env)

    def put(self, command_id, value, rev=0, epoch=0, subject=SUBJ, **extra):
        return self.command("core-test.subject.put", command_id, subject,
                            [{"subject": subject, "revision": rev}], {"value": value},
                            authority_epoch=epoch, **extra)

    def close(self) -> int:
        self.proc.stdin.close()
        rest = self.proc.stdout.read()
        code = self.proc.wait(timeout=10)
        assert rest == b"", rest
        return code


def code_of(resp):
    if resp is None:
        return "<closed>"
    if "error" in resp:
        return resp["error"]["data"]["code"]
    return "ok"


RESULTS = []


def probe(tag: str, description: str, got, want):
    ok = got == want
    RESULTS.append(ok)
    print(f"{'ok  ' if ok else 'FAIL'} {tag:<16} {description}: got {got!r}" + ("" if ok else f", want {want!r}"))


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        def fresh(name, config=None):
            d = os.path.join(tmp, name)
            os.makedirs(d)
            return Conn(d, config)

        c = fresh("order")
        probe("D-STEP1", "unknown method before negotiation", code_of(c.query("nope.op", {})), "method_not_found")
        probe("D-STEP1", "deep unknown method is not a limit error",
              code_of(c.request("nope.op", {"operation": "nope.op", "message_id": "m", "payload": {"x": V.loads("[" * 5000 + "]" * 5000)}})),
              "method_not_found")
        probe("D-NEG-RETRY", "failed negotiation can be retried",
              [code_of(c.negotiate([{"name": "core-test", "majors": [9], "required": True, "required_features": [], "optional_features": []}])),
               code_of(c.negotiate())], ["unsupported_version", "ok"])
        probe("D-NEG-AGAIN", "second negotiate with an invalid payload", code_of(c.query("core.negotiate", {"bogus": 1})), "invalid_envelope")
        probe("D-LIMIT-DEPTH", "5000-deep extension is limit_exceeded, not a crash",
              code_of(c.query("core-test.subject.get", {"subject": SUBJ}, extensions={"example.org/x": V.loads("[" * 5000 + "]" * 5000)})),
              "limit_exceeded")
        probe("D-PRE-PRIMARY", "put without a precondition on its primary subject",
              code_of(c.command("core-test.subject.put", "p-1", SUBJ, [{"subject": {"kind": "core-test.subject", "id": "other"}, "revision": 0}],
                                {"value": "v"}, authority_epoch=0)), "invalid_envelope")
        probe("D-CLAIM-PRE", "claim whose precondition names another subject",
              code_of(c.command("core-test.authority.claim", "c-x", AUTH, [{"subject": SUBJ, "revision": 0}], {})), "invalid_envelope")
        probe("D-ORDER-7", "stale epoch and failed precondition together",
              [code_of(c.command("core-test.authority.claim", "c-1", AUTH, [{"subject": AUTH, "revision": 0}], {})),
               code_of(c.put("p-2", "v", rev=7, epoch=0))], ["ok", "stale_authority_epoch"])
        probe("D-SHA512", "sha512 digest when core.digest-sha512 was not negotiated",
              code_of(c.put("p-3", "v", epoch=1, algorithm="sha512")), "unsupported_digest_algorithm")
        # Resolved 2026-09-13 (M2-DIVERGENCES D-PRE-DUP): duplicate subjects are invalid_envelope.
        probe("D-PRE-DUP", "duplicate identical preconditions",
              code_of(c.command("core-test.subject.put", "p-4", SUBJ, [{"subject": SUBJ, "revision": 0}] * 2, {"value": "v"}, authority_epoch=1)),
              "invalid_envelope")
        probe("D-DEDUPE-GEN", "retransmission may carry a different retained generation",
              [code_of(c.put("p-5", "w", rev=0, epoch=1)),
               c.put("p-5", "w", rev=0, epoch=1, dedupe_generation=0).get("result", {}).get("replay")], ["ok", True])
        probe("D-NOTIFY", "id-less object without a method", (c.raw(b'{"jsonrpc":"2.0"}\n'), code_of(c.read()))[1], "invalid_request")
        probe("D-RPC-PARAMS", "request without params",
              (c.raw(b'{"jsonrpc":"2.0","id":5,"method":"core.describe"}\n'), code_of(c.read()))[1], "invalid_request")
        # Resolved 2026-09-13 to code points (M2-DIVERGENCES D-STREAM-ID): 65 code points is valid.
        probe("D-STREAM-ID", "130-byte, 65-code-point string id is accepted and echoed",
              (c.raw(V.canonical({"jsonrpc": "2.0", "id": "é" * 65, "method": "core.describe",
                                  "params": {"operation": "core.describe", "message_id": "m", "payload": {}}}) + b"\n"),
               c.read()["id"])[1], "é" * 65)
        probe("EXIT", "exit status at end of input", c.close(), 0)

        c = fresh("sha512")
        probe("D-SHA512", "sha512 digest after negotiating core.digest-sha512",
              [code_of(c.negotiate(core_features=["core.digest-sha512"])), code_of(c.put("p-1", "v", algorithm="sha512"))], ["ok", "ok"])
        probe("D-DIGEST-ALG", "same intent, same command_id, other algorithm",
              code_of(c.put("p-1", "v")), "idempotency_conflict")
        c.close()

        c = fresh("frames", {"limits": {"max_frame_bytes": 2 * 1048576}})
        c.negotiate()
        big = b'{"jsonrpc":"2.0","id":1,"method":"core.describe","params":{"operation":"core.describe","message_id":"m","payload":{}}' + b" " * (1536 * 1024) + b"}\n"
        c.raw(big)
        probe("D-FRAME-NEG", "frame above 1 MiB accepted after negotiating a 2 MiB limit", code_of(c.read()), "ok")
        c.close()

        c = fresh("preneg", {"limits": {"max_frame_bytes": 2 * 1048576}})
        try:
            c.raw(big)
        except BrokenPipeError:
            pass  # the provider stops reading after limit + 1 bytes and closes
        probe("D-FRAME-NEG", "the same frame before negotiation closes", [code_of(c.read()), c.read()], ["frame_too_large", None])
        c.proc.wait(timeout=10)

    print(f"{sum(RESULTS)}/{len(RESULTS)} probes as documented")
    return 0 if all(RESULTS) else 1


if __name__ == "__main__":
    sys.exit(main())
