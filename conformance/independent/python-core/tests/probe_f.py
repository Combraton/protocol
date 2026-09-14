"""Probes of the section F decisions (documents at 6c64ae4) the fixtures do not exercise.

Drives provider.py over its stdio binding. Each probe asserts the behavior
this implementation chose or read from the documents; the entry and its basis
are in DIVERGENCES.md section F under the probe's tag.

Usage: python3 conformance/independent/python-core/tests/probe_f.py
"""

from __future__ import annotations

import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from probe_m2 import M2, details, result  # noqa: E402
from probe_provider import AUTH, SUBJ, Conn, code_of  # noqa: E402

RESULTS = []
S2 = {"kind": "core-test.subject", "id": "s-2"}


def probe(tag: str, description: str, got, want):
    ok = got == want
    RESULTS.append(ok)
    print(f"{'ok  ' if ok else 'FAIL'} {tag:<26} {description}: got {got!r}" + ("" if ok else f", want {want!r}"))


def ids(read):
    """(kind of item, subject id or epoch marker) for each item of a read result."""
    out = []
    for item in read["items"]:
        if "event" in item:
            out.append(item["event"]["subject"]["id"])
        elif "gap" in item:
            out.append(("gap", [s["subject"]["id"] for s in item["gap"]["snapshot"]["subjects"]]))
        else:
            out.append(("epoch_change", item["epoch_change"]["vouched_through"]))
    return out


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        def data(name):
            d = os.path.join(tmp, name)
            os.makedirs(d, exist_ok=True)
            return d

        # ------------------------------------------------ CORE 18 authenticate
        c = Conn(data("auth"), {"principal": "agent-1"})
        cred = "ccred1.agent-1." + "A" * 43
        r = c.query("core.authenticate", {"credential": cred})
        probe("F-AUTH-STEP1", "core.authenticate before negotiation", code_of(r), "already_authenticated")
        probe("F-AUTH-SECRET", "the refusal does not echo the credential", cred in repr(r), False)
        probe("F-AUTH-ORDER", "invalid payload is invalid_envelope before already_authenticated",
              code_of(c.query("core.authenticate", {})), "invalid_envelope")
        probe("F-AUTH-ORDER", "a malformed credential string is still already_authenticated",
              code_of(c.query("core.authenticate", {"credential": "not-a-credential"})), "already_authenticated")
        probe("F-AUTH-ORDER", "requires naming an unnegotiated feature",
              code_of(c.query("core.authenticate", {"credential": cred}, requires=["core.grants"])),
              "unsupported_required_feature")
        c.negotiate()
        probe("F-AUTH-STEP1", "core.authenticate after negotiation",
              code_of(c.query("core.authenticate", {"credential": cred})), "already_authenticated")
        c.close()

        # ------------------------------------------------- CORE 15.3 binding scope
        d = data("binding")
        c = M2(d, "owner")
        terms = {"holder": "agent-1", "audience": "conformance-provider", "rights": ["core-test.read"],
                 "resources": [{"kind": "core-test.subject"}], "delegation": {"allowed": False, "max_depth": 0}}
        subj = {"kind": "core.grant", "id": "gb"}
        pre = [{"subject": subj, "revision": 0}]
        r = c.command("core.grant.issue", "gb-1", subj, pre, dict(terms, authority_binding={"scope": "nope", "epoch": 0}))
        probe("F-ISSUE-CHECK-ORDER", "unknown scope names its path", details(r).get("path"), "/payload/authority_binding/scope")
        r = c.command("core.grant.issue", "gb-2", subj, pre, dict(terms, audience="elsewhere",
                                                                   authority_binding={"scope": "nope", "epoch": 0}))
        probe("F-ISSUE-CHECK-ORDER", "wrong audience and unknown scope: audience reported", details(r).get("path"), "/payload/audience")
        c.close()
        c = M2(d, "agent-1")
        r = c.command("core.grant.issue", "gb-3", subj, pre, dict(terms, authority_binding={"scope": "nope", "epoch": 0}))
        probe("F-ISSUE-CHECK-ORDER", "non-authority root grant with unknown scope: invalid_envelope before not_authority",
              code_of(r), "invalid_envelope")
        c.close()

        # ------------------------------------------------- CORE 16.6 visibility
        d = data("visibility")
        c = M2(d, "owner")
        c.put("v-1", "a")                                                     # 1 s-1
        c.put("v-2", "b", subject=S2)                                          # 2 s-2
        c.command("core-test.authority.claim", "claim-1", AUTH, [{"subject": AUTH, "revision": 0}], {})  # 3 authority
        c.issue("g-ag", "agent-1", ["core.events.read", "core-test.read"],     # 4 g-ag
                [{"kind": "core-test.subject", "id": "s-1"}], delegation={"allowed": True, "max_depth": 1})
        c.issue("g-cap", "agent-2", ["core.events.read"], [{"kind": "core.capabilities"}])  # 5 g-cap
        c.issue("g-self", "owner", ["core.events.read"], [{"kind": "core-test.subject"}])   # 6 g-self
        c.issue("g-auth", "agent-2", ["core.events.read", "core-test.read"], [{"kind": "core-test.authority"}])  # 7
        c.close()
        c = M2(d, "agent-1")
        c.issue("c-1", "agent-2", ["core-test.read"], [{"kind": "core-test.subject", "id": "s-1"}], parent="g-ag")  # 8 c-1
        c.close()
        # 9: a status configured as supported changes the evidence source, so
        # the revision rises and one event is recorded (E-CAP-EVIDENCE). Every
        # later start on this store keeps the same configuration.
        caps = {"capabilities": {"core-test.writes": "supported"}}
        c = M2(d, "owner", **caps)
        probe("F-VISIBILITY", "authority without a grant sees every event",
              ids(result(c.read_events())), ["s-1", "s-2", "core-test", "g-ag", "g-cap", "g-self", "g-auth", "c-1",
                                              "conformance-provider"])
        probe("F-GRANT-VIS-AUTHORITY", "authority under its own grant: only grants it holds or issued, not a delegated child",
              ids(result(c.read_events(grant="g-self"))), ["g-ag", "g-cap", "g-self", "g-auth"])
        probe("F-GRANT-VIS-AUTHORITY", "yet core.grant.get as the same authority shows the child",
              code_of(c.query("core.grant.get", {"grant": "c-1"}, grant="g-self")), "ok")
        c.close()
        c = M2(d, "agent-1", **caps)
        probe("F-VISIBILITY", "issuer of a delegated grant sees its issue event; holder sees its own grant",
              ids(result(c.read_events(grant="g-ag"))), ["s-1", "g-ag", "c-1"])
        c.close()
        c = M2(d, "agent-2", **caps)
        r = result(c.read_events(grant="g-cap"))
        probe("F-VISIBILITY", "core.capabilities needs only resource coverage; held grants visible; no read right needed",
              ids(r), ["g-cap", "g-auth", "c-1", "conformance-provider"])
        probe("F-VISIBILITY", "core-test.authority needs core-test.read covering it",
              ids(result(c.read_events(grant="g-auth"))), ["core-test", "g-cap", "g-auth", "c-1"])
        c.close()

        # ------------------------------------------------- CORE 7 current
        foreign = {"kind": "other.kind", "id": "x"}
        c = M2(d, "owner", **caps)
        # Step 6 needs core-test.read on every other precondition subject, so
        # the grant also covers the foreign kind.
        c.issue("g-w", "agent-1", ["core-test.write", "core-test.read"], [{"kind": "core-test.subject"}, {"kind": "other.kind"}])
        c.close()
        c = M2(d, "agent-1", **caps)
        r = c.command("core-test.subject.put", "pc-2", SUBJ,
                      [{"subject": SUBJ, "revision": 9}, {"subject": foreign, "revision": 3}],
                      {"value": "z"}, authority_epoch=1, grant="g-w")
        probe("F-PRE-CURRENT-GRANT", "under a grant: readable subject has current, a foreign kind does not",
              details(r), {"failed": [{"subject": SUBJ, "expected": 9, "current": 1},
                                      {"subject": foreign, "expected": 3}]})
        c.close()
        c = M2(d, "owner", **caps)
        r = c.command("core-test.subject.put", "pc-3", SUBJ,
                      [{"subject": SUBJ, "revision": 9}, {"subject": foreign, "revision": 3}], {"value": "z"}, authority_epoch=1)
        probe("F-PRE-CURRENT-GRANT", "authority without a grant: current for every subject, 0 for a foreign kind",
              details(r), {"failed": [{"subject": SUBJ, "expected": 9, "current": 1},
                                      {"subject": foreign, "expected": 3, "current": 0}]})
        c.close()
        c = M2(d, "agent-1", **caps)
        cs = {"kind": "core.grant", "id": "c-1"}
        r = c.command("core.grant.issue", "pc-4", cs, [{"subject": cs, "revision": 0}],
                      {"holder": "agent-2", "audience": "conformance-provider", "rights": ["core-test.read"],
                       "resources": [{"kind": "core-test.subject", "id": "s-1"}],
                       "delegation": {"allowed": False, "max_depth": 0}, "parent": "g-ag"})
        probe("F-PRE-CURRENT-NO-GRANT", "non-authority without a grant delegating: current for a grant it issued",
              details(r), {"failed": [{"subject": cs, "expected": 0, "current": 1}]})
        c.close()

        # ------------------------------------------------- CORE 16.4 filtered
        d = data("filtered")
        c = M2(d, "owner")
        c.put("f-1", "a")
        c.put("f-2", "b", subject=S2)
        c.issue("g-f", "agent-1", ["core.events.read", "core-test.read"], [{"kind": "core-test.subject", "id": "s-1"}])
        c.close()
        c = M2(d, "agent-1")
        r1 = result(c.read_events(grant="g-f", limit=1))
        probe("F-FILTERED-RANGE", "limit reached before a hidden event: not filtered", [ids(r1), r1["filtered"]], [["s-1"], False])
        r2 = result(c.read_events(grant="g-f", cursor=r1["next_cursor"], limit=1))
        probe("F-FILTERED-RANGE", "the next read covers the hidden event and the grant event", [ids(r2), r2["filtered"]],
              [["g-f"], True])
        r3 = result(c.read_events(grant="g-f", cursor=r2["next_cursor"], limit=1))
        probe("F-FILTERED-RANGE", "at the end nothing is left to cover", [ids(r3), r3["filtered"], r3["next_cursor"] == r2["next_cursor"]],
              [[], False, True])
        c.close()
        c = M2(d, "owner")
        r4 = result(c.read_events(kinds=["core-test.subject"]))
        probe("F-FILTERED-RANGE", "an authority's read hiding only by kinds is filtered", [ids(r4), r4["filtered"]], [["s-1", "s-2"], True])
        c.close()

        # --------------------------------------- CORE 16.5 subscription end
        d = data("sub-end")
        c = M2(d, "owner")
        c.put("se-1", "a")
        c.issue("g-sub", "owner", ["core.events.read", "core-test.read"], [{"kind": "core-test.subject"}])
        sub = result(c.query("core.events.subscribe", {"from": "start", "kinds": ["core-test.subject"]}, grant="g-sub"))
        c.drain()
        delivered = [n["params"] for n in c.notifications()]
        mark = len(c.frames)
        c.revoke("g-sub", 1)
        c.drain()
        ended = [f["params"] for f in c.frames[mark:] if "method" in f]
        probe("F-SUB-END-TIMING", "ended is sent right after the revocation although no item passes the kinds filter",
              [(e["items"], e.get("ended")) for e in ended], [([], {"reason": "authorization_lost"})])
        probe("F-SUB-END-CURSOR", "the final notification's next_cursor is where delivery stopped (covering hidden events)",
              [len(delivered), ended[0]["next_cursor"] == delivered[-1]["next_cursor"], sub["subscription"] == ended[0]["subscription"]],
              [1, True, True])
        head = result(c.read_events(**{"from": "now"}))["next_cursor"]
        probe("F-SUB-END-CURSOR", "and it is not past the revocation event", ended[0]["next_cursor"] != head, True)
        c.close()

        # --------------------------------------- unvouched_last (README)
        d = data("unvouched")
        c = M2(d, "owner")
        c.put("u-1", "a")                      # (1,1)
        c.put("u-2", "x", subject=S2)          # (1,2)
        c.close()
        c = M2(d, "owner", events={"unvouched_last": 1})
        probe("F-UNVOUCHED-CONFIG", "unvouched_last without new_epoch_on_start has no effect",
              ids(result(c.read_events())), ["s-1", "s-2"])
        c.close()
        c = M2(d, "owner", events={"new_epoch_on_start": True, "unvouched_last": 1, "retain_last": 1})
        probe("F-UNVOUCHED-RETENTION", "unvouched events leave the stream and do not count toward retain_last",
              ids(result(c.read_events())), ["s-1", ("epoch_change", 1)])
        probe("F-UNVOUCHED", "subject state is unchanged", result(c.query("core-test.subject.get", {"subject": S2}))["value"], "x")
        c.put("u-3", "b", rev=1)               # (2,1)
        c.close()
        c = M2(d, "owner", events={"retain_last": 0})
        r = result(c.read_events())
        probe("F-GAP-EPOCHS", "a discarded range spanning epochs is one gap; the snapshot includes the unvouched change",
              [ids(r), r["items"][0]["gap"]["to"]], [[("gap", ["s-1", "s-2"])], {"epoch": 2, "sequence": 1}])
        c.close()
        d = data("unvouched-clamp")
        c = M2(d, "owner")
        c.put("uc-1", "a")
        c.close()
        c = M2(d, "owner", events={"new_epoch_on_start": True, "unvouched_last": 5})
        probe("F-UNVOUCHED-CONFIG", "unvouched_last above the epoch's length vouches through 0",
              ids(result(c.read_events())), [("epoch_change", 0)])
        c.close()

        c = Conn(data("creds"), {"principal": "owner", "credentials": [{"credential": "ccred1.owner." + "B" * 43}]})
        probe("F-CREDENTIALS-CONFIG", "a stdio launch with credentials refuses to start", c.proc.wait(timeout=10), 2)

    print(f"{sum(RESULTS)}/{len(RESULTS)} probes as documented")
    return 0 if all(RESULTS) else 1


if __name__ == "__main__":
    sys.exit(main())
