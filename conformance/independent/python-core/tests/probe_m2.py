"""Probes of M2 behavior (grants, events, capabilities) the fixtures do not exercise.

Drives provider.py over its stdio binding. Each probe asserts the behavior
this implementation chose; the choice and its basis are recorded in
DIVERGENCES.md section E under the probe's tag. Probes whose decision was
later resolved the other way (M2-DIVERGENCES section E) now assert the
resolved behavior and say so; section F decisions are in probe_f.py.

Usage: python3 conformance/independent/python-core/tests/probe_m2.py
"""

from __future__ import annotations

import os
import select
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from probe_provider import AUTH, SUBJ, Conn, code_of  # noqa: E402
import valuedomain as V  # noqa: E402

FEATURES = ["core.grants", "core.events", "core.capabilities"]
T0 = "2030-01-01T00:00:00Z"
RESULTS = []


def probe(tag: str, description: str, got, want):
    ok = got == want
    RESULTS.append(ok)
    print(f"{'ok  ' if ok else 'FAIL'} {tag:<24} {description}: got {got!r}" + ("" if ok else f", want {want!r}"))


def reason(resp):
    if resp and "error" in resp:
        return resp["error"]["data"]["details"].get("reason", resp["error"]["data"]["code"])
    return code_of(resp)


def details(resp):
    return resp["error"]["data"]["details"] if resp and "error" in resp else resp


def result(resp):
    return resp.get("result") if resp else None


class M2(Conn):
    """A session that negotiates the M2 features and records every frame in
    arrival order, so notification ordering can be observed."""

    def __init__(self, data_dir, principal, authorities=("owner",), negotiate=True, **config):
        cfg = {"principal": principal, "authority_principals": list(authorities)}
        cfg.update(config)
        super().__init__(data_dir, cfg)
        self.buf = b""
        self.frames = []  # every frame received, in order
        if negotiate:
            self.negotiate(core_features=FEATURES)

    def _frame(self, timeout):
        fd = self.proc.stdout.fileno()
        while b"\n" not in self.buf:
            if timeout is not None and not select.select([fd], [], [], timeout)[0]:
                return None
            chunk = os.read(fd, 65536)
            if not chunk:
                return None
            self.buf += chunk
        line, self.buf = self.buf.split(b"\n", 1)
        msg = V.loads(line.decode("utf-8"))
        self.frames.append(msg)
        return msg

    def read(self):
        while True:
            msg = self._frame(None)
            if msg is None or "id" in msg:
                return msg

    def drain(self, seconds=0.5):
        """Frames arriving within the window."""
        got = []
        while (msg := self._frame(seconds)) is not None:
            got.append(msg)
        return got

    def notifications(self):
        return [f for f in self.frames if f.get("method") == "core.events.notify"]

    def issue(self, gid, holder, rights, resources, command_id=None, **terms):
        payload = {"holder": holder, "audience": "conformance-provider", "rights": rights,
                   "resources": resources, "delegation": terms.pop("delegation", {"allowed": False, "max_depth": 0})}
        payload.update(terms)
        subj = {"kind": "core.grant", "id": gid}
        return self.command("core.grant.issue", command_id or f"issue-{gid}", subj, [{"subject": subj, "revision": 0}], payload)

    def revoke(self, gid, revision):
        subj = {"kind": "core.grant", "id": gid}
        return self.command("core.grant.revoke", f"revoke-{gid}-{revision}", subj, [{"subject": subj, "revision": revision}], {})

    def read_events(self, grant=None, **payload):
        payload.setdefault("limit", 100)
        if "cursor" not in payload:
            payload.setdefault("from", "start")
        return self.query("core.events.read", payload, **({"grant": grant} if grant else {}))


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        def data(name):
            d = os.path.join(tmp, name)
            os.makedirs(d, exist_ok=True)
            return d

        # ----------------------------------------------------------- grants
        d = data("grants")
        c = M2(d, "owner", clock={"fixed": T0})
        probe("E-GRANT-PRE", "issue with precondition revision 1",
              code_of(c.command("core.grant.issue", "bad-1", {"kind": "core.grant", "id": "gx"},
                                [{"subject": {"kind": "core.grant", "id": "gx"}, "revision": 1}],
                                {"holder": "a", "audience": "conformance-provider", "rights": ["core-test.read"],
                                 "resources": [{"kind": "core-test.subject"}], "delegation": {"allowed": False, "max_depth": 0}})),
              "invalid_envelope")
        probe("E-GRANT-PRE", "revoke with precondition revision 0", code_of(c.revoke("gx", 0)), "invalid_envelope")
        c.issue("g-1", "agent-1", ["core-test.read", "core-test.write"], [{"kind": "core-test.subject", "id_prefix": "s-"}],
                delegation={"allowed": True, "max_depth": 2}, expires_at="2030-01-10T00:00:00Z",
                authority_binding={"scope": "core-test", "epoch": 0})
        c.issue("g-nodelegate", "agent-1", ["core-test.read"], [{"kind": "core-test.subject"}])
        c.issue("g-exp", "agent-1", ["core-test.read"], [{"kind": "core-test.subject"}], expires_at="2030-01-02T00:00:00Z")
        c.issue("g-a2", "agent-2", ["core-test.read"], [{"kind": "core-test.subject"}])
        c.issue("g-idparent", "agent-1", ["core-test.read"], [{"kind": "core-test.subject", "id": "s-1"}],
                delegation={"allowed": True, "max_depth": 1})
        probe("E-ISSUE-PARENT-HOLDER", "authority issuing a child of a grant it does not hold",
              reason(c.issue("g-owner-child", "agent-2", ["core-test.read"], [{"kind": "core-test.subject", "id": "s-1"}],
                             parent="g-1", expires_at="2030-01-05T00:00:00Z", authority_binding={"scope": "core-test", "epoch": 0})),
              "grant_not_found")
        c.close()

        c = M2(d, "agent-2", clock={"fixed": T0})
        probe("E-REVOKE-DENIAL", "stranger revokes an existing grant", reason(c.revoke("g-1", 1)), "not_authority")
        probe("E-REVOKE-DENIAL", "stranger revokes a nonexistent grant", reason(c.revoke("g-none", 1)), "not_authority")
        probe("E-GRANT-GET", "stranger reads a grant", code_of(c.query("core.grant.get", {"grant": "g-1"})), "not_found")
        probe("E-UNPROTECTED", "non-authority without a grant reads the capability snapshot",
              code_of(c.query("core.capabilities", {})), "ok")
        c.close()

        c = M2(d, "agent-1", clock={"fixed": T0})
        base = {"parent": "g-1", "expires_at": "2030-01-05T00:00:00Z", "authority_binding": {"scope": "core-test", "epoch": 0}}
        s1 = [{"kind": "core-test.subject", "id": "s-1"}]
        probe("E-DELEGATION", "child expiring after its parent",
              reason(c.issue("c-1", "agent-2", ["core-test.read"], s1, **dict(base, expires_at="2030-02-01T00:00:00Z"))),
              "delegation_exceeded")
        probe("E-DELEGATION", "child without expiry under an expiring parent",
              reason(c.issue("c-2", "agent-2", ["core-test.read"], s1, **{k: v for k, v in base.items() if k != "expires_at"})),
              "delegation_exceeded")
        probe("E-DELEGATION", "child with a different authority binding",
              reason(c.issue("c-3", "agent-2", ["core-test.read"], s1, **dict(base, authority_binding={"scope": "core-test", "epoch": 1}))),
              "delegation_exceeded")
        probe("E-DELEGATION", "child of a parent that does not allow delegation",
              reason(c.issue("c-4", "agent-2", ["core-test.read"], s1, parent="g-nodelegate")), "delegation_exceeded")
        probe("E-DELEGATION", "id_prefix child under a parent narrowed by id",
              reason(c.issue("c-6", "agent-2", ["core-test.read"], [{"kind": "core-test.subject", "id_prefix": "s-1"}],
                             parent="g-idparent")), "delegation_exceeded")
        probe("E-DELEGATION", "id_prefix child within the parent's id_prefix",
              code_of(c.issue("c-5", "agent-2", ["core-test.read"], [{"kind": "core-test.subject", "id_prefix": "s-10"}], **base)),
              "ok")
        probe("E-DELEGATION", "child within every bound",
              code_of(c.issue("c-ok", "agent-2", ["core-test.read"], s1, delegation={"allowed": True, "max_depth": 1}, **base)),
              "ok")
        probe("E-GRANT-GET", "issuer (not holder, not authority) reads its child grant",
              code_of(c.query("core.grant.get", {"grant": "c-ok"})), "ok")
        probe("E-PRE-CURRENT", "precondition_failed omits current for a grant ID the principal cannot see",
              details(c.issue("g-a2", "agent-2", ["core-test.read"], s1, command_id="issue-dup", **base)),
              {"failed": [{"subject": {"kind": "core.grant", "id": "g-a2"}, "expected": 0}]})
        probe("E-PRE-CURRENT", "and includes it for a grant the principal holds",
              details(c.issue("g-exp", "agent-2", ["core-test.read"], s1, command_id="issue-dup2", **base)),
              {"failed": [{"subject": {"kind": "core.grant", "id": "g-exp"}, "expected": 0, "current": 1}]})
        c.close()

        c = M2(d, "agent-1", clock={"fixed": "2030-01-02T00:00:00Z"})
        probe("E-EXPIRY-BOUNDARY", "grant used at exactly its expires_at",
              reason(c.query("core-test.subject.get", {"subject": SUBJ}, grant="g-exp")), "expired")
        c.close()

        c = M2(d, "owner", clock={"fixed": "2030-03-01T00:00:00Z"})
        probe("E-ISSUE-VALIDATION-STEP", "retransmitted issue after the clock passed its expires_at replays",
              result(c.issue("g-exp", "agent-1", ["core-test.read"], [{"kind": "core-test.subject"}],
                             expires_at="2030-01-02T00:00:00Z"))["replay"], True)
        probe("E-ISSUE-VALIDATION-STEP", "the same terms under a new command_id are invalid",
              code_of(c.issue("g-exp2", "agent-1", ["core-test.read"], [{"kind": "core-test.subject"}],
                              expires_at="2030-01-02T00:00:00Z")), "invalid_envelope")
        probe("E-REVOKE-CASCADE", "revocation lists the grant, then descendants in issue order",
              result(c.revoke("g-1", 1))["outcome"], {"revoked": ["g-1", "c-5", "c-ok"]})
        probe("E-REVOKE-TWICE", "revoking an already revoked grant", reason(c.revoke("g-1", 2)), "revoked")
        c.close()

        c = M2(d, "owner", provider_id="renamed-provider", clock={"fixed": T0})
        probe("E-ISSUE-VALIDATION-STEP", "retransmitted issue replays after provider_id changes",
              result(c.issue("g-nodelegate", "agent-1", ["core-test.read"], [{"kind": "core-test.subject"}]))["replay"], True)
        c.close()

        c = Conn(data("auth-always"), {"principal": "agent-1", "authority_principals": ["owner"]})
        c.negotiate()
        probe("E-AUTH-WITHOUT-FEATURE", "non-authority in a session without core.grants",
              reason(c.query("core-test.subject.get", {"subject": SUBJ})), "grant_required")
        c.close()

        # ------------------------------------------------------ step order
        d = data("order")
        c = M2(d, "owner", capabilities={"core-test.writes": "unsupported"})
        c.command("core-test.authority.claim", "claim-1", AUTH, [{"subject": AUTH, "revision": 0}], {})
        probe("E-STEP7-ORDER", "capability before stale epoch and failed precondition",
              code_of(c.put("p-1", "v", rev=9, epoch=0)), "capability_unavailable")
        c.close()
        c = M2(d, "agent-1", capabilities={"core-test.writes": "unsupported"})
        probe("E-STEP7-ORDER", "authorization before capability", reason(c.put("p-2", "v", epoch=1)), "grant_required")
        c.close()

        # ----------------------------------------------------------- events
        d = data("events")
        c = Conn(d, {"principal": "owner"})
        c.negotiate()
        c.put("e-1", "a")
        c.close()
        c = M2(d, "owner")
        probe("E-EVENTS-WITHOUT-FEATURE", "commands in a session without core.events still record events",
              [i["event"]["command_id"] for i in result(c.read_events())["items"]], ["e-1"])
        # CORE 16.6 as resolved: profile subjects need the profile's read right too.
        c.issue("g-self", "owner", ["core.events.read", "core-test.read"], [{"kind": "core-test.subject"}])
        head = result(c.read_events(**{"from": "now"}))["next_cursor"]
        probe("E-FILTERED", "resolved: authority acting under a grant, nothing hidden in range",
              result(c.read_events(grant="g-self", cursor=head))["filtered"], False)
        o = M2(data("other-stream"), "owner")
        foreign = result(o.read_events(**{"from": "now"}))["next_cursor"]
        o.close()
        probe("E-CURSOR", "cursor from another store's stream", details(c.read_events(cursor=foreign)), {"reason": "other_stream"})
        stream, epoch, seq = head.split(".")
        probe("E-CURSOR", "cursor one past the head",
              details(c.read_events(cursor=f"{stream}.{epoch}.{int(seq) + 1}")), {"reason": "beyond_end"})
        probe("E-UNSUBSCRIBE", "unsubscribe an unknown subscription",
              code_of(c.query("core.events.unsubscribe", {"subscription": "sub-99"})), "not_found")
        c.query("core.events.subscribe", {"from": "now", "kinds": ["core-test.subject"]})
        c.put("e-2", "b", rev=1)
        c.issue("g-later", "agent-1", ["core-test.read"], [{"kind": "core-test.subject"}])
        c.drain()
        probe("E-SUB-KINDS", "subscription with a kinds filter delivers only matching events",
              [i["event"]["type"] for n in c.notifications() for i in n["params"]["items"]], ["core-test.subject.changed"])
        c.close()

        c = M2(d, "owner")
        c.query("core.events.subscribe", {"from": "now"}, grant="g-self")
        mark = len(c.frames)
        response = c.put("e-3", "c", rev=2)
        c.drain()
        kinds = ["notification" if "method" in f else "response" for f in c.frames[mark:]]
        probe("E-SUB-ORDER", "a command's notification arrives after its response", [code_of(response)] + kinds,
              ["ok", "response", "notification"])
        mark = len(c.frames)
        c.revoke("g-self", 1)
        c.drain()
        ended = [f["params"] for f in c.frames[mark:] if "method" in f]
        probe("E-SUB-REAUTH", "resolved: revoking the subscription's grant sends one final ended notification",
              [(n["items"], n.get("ended")) for n in ended], [([], {"reason": "authorization_lost"})])
        mark = len(c.frames)
        c.put("e-4", "d", rev=3)
        c.drain()
        probe("E-SUB-REAUTH", "and nothing further",
              [f for f in c.frames[mark:] if "method" in f], [])
        c.close()

        # --------------------------------------------- retention and epochs
        d = data("retention")
        c = M2(d, "owner")
        c.put("r-1", "a")
        c.put("r-2", "a", subject={"kind": "core-test.subject", "id": "t-1"})
        c.issue("g-s", "agent-1", ["core.events.read", "core-test.read"], [{"kind": "core-test.subject", "id_prefix": "s-"}])
        c.close()
        c = M2(d, "owner", events={"new_epoch_on_start": True, "retain_last": 0})
        items = result(c.read_events())["items"]
        probe("E-START-ORDER", "new epoch and retain_last 0 at one start",
              [next(iter(i)) for i in items] + [items[0]["gap"]["to"], items[1]["epoch_change"]],
              ["gap", "epoch_change", {"epoch": 1, "sequence": 3}, {"from_epoch": 1, "to_epoch": 2, "vouched_through": 3}])
        c.put("r-3", "b", rev=1)
        c.close()
        c = M2(d, "agent-1")
        r = result(c.read_events(grant="g-s"))
        probe("E-GAP-FILTER", "resolved: gap snapshot under a grant lists readable subjects and grants the principal holds",
              [s["subject"]["id"] for s in r["items"][0]["gap"]["snapshot"]["subjects"]] + [r["filtered"]], ["s-1", "g-s", True])
        probe("E-GAP-FILTER", "then the epoch change, then the new epoch's events",
              [next(iter(i)) for i in r["items"][1:]] + [r["items"][2]["event"]["epoch"]], ["epoch_change", "event", 2])
        c.close()

        # ---------------------------------------------- frame-size fitting
        d = data("sizes")
        c = M2(d, "owner", limits={"max_string_bytes": 262144})
        big = "x" * 200000
        for n in range(10):
            assert code_of(c.put(f"big-{n}", big, subject={"kind": "core-test.subject", "id": f"b-{n}"})) == "ok"
        r1 = result(c.read_events())
        r2 = result(c.read_events(cursor=r1["next_cursor"]))
        probe("E-NOTIFY-SIZE", "a read whose full result exceeds 1 MiB returns fewer items; resuming covers the rest",
              [0 < len(r1["items"]) < 10, [i["event"]["sequence"] for i in r1["items"] + r2["items"]][:10]],
              [True, list(range(1, 11))])
        mark = len(c.frames)
        c.query("core.events.subscribe", {"from": "start"})
        c.drain(1.0)
        notes = [f for f in c.frames[mark:] if "method" in f]
        probe("E-NOTIFY-SIZE", "backlog notifications stay within the caller's frame limit and deliver every event",
              [len(notes) > 1, all(len(V.canonical(f)) <= 1048576 for f in notes),
               [i["event"]["sequence"] for f in notes for i in f["params"]["items"]]],
              [True, True, list(range(1, 11))])
        c.close()

        # ---------------------------------------------------- capabilities
        d = data("caps")
        c = M2(d, "owner")
        rev1 = result(c.query("core.capabilities", {}))["revision"]
        c.close()
        c = M2(d, "owner", capabilities={"core-test.writes": "supported"}, provider_id="p-42")
        snap = result(c.query("core.capabilities", {}))
        ev = result(c.read_events(kinds=["core.capabilities"]))["items"]
        probe("E-CAP-EVIDENCE", "same status on different evidence raises the revision", [rev1, snap["revision"]], [1, 2])
        probe("E-CAP-EVENT", "event revision equals the snapshot revision; subject id is provider_id",
              [ev[0]["event"]["revision"], ev[0]["event"]["subject"]], [2, {"kind": "core.capabilities", "id": "p-42"}])
        c.close()
        c = Conn(data("caps-bad"), {"principal": "owner", "capabilities": {"core-test.teleport": "supported"}})
        probe("E-CAP-CONFIG", "unknown capability name in the launch configuration", c.proc.wait(timeout=10), 2)

    print(f"{sum(RESULTS)}/{len(RESULTS)} probes as documented")
    return 0 if all(RESULTS) else 1


if __name__ == "__main__":
    sys.exit(main())
