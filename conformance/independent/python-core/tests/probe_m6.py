"""Probes of the M6 pass: core.feature_dependencies and feature-triggered
dependencies (DIVERGENCES H.M6).

On this participant the CMP-5 fixture is unsupported, because
execution.claim_revalidation is not implemented (HM6-UNSUPPORTED-TRIGGER). The
second half therefore runs a scratch copy of the provider in which that feature
is added to the supported list, to exercise the feature-triggered rule and the
query's feature entry. The committed provider is never modified.

Usage: python3 conformance/independent/python-core/tests/probe_m6.py
"""

from __future__ import annotations

import os
import shutil
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import probe_provider as P  # noqa: E402

CORE_EXEC = ["core.events", "core.capabilities", "core.effects"]


def core(features):
    return {"name": "core", "majors": [1], "required": True, "required_features": features, "optional_features": []}


def execution(required=True, req=(), opt=()):
    return {"name": "execution", "majors": [1], "required": required, "required_features": list(req),
            "optional_features": list(opt)}


def entry(profile, requires_features, trigger=None, requires_profile="core"):
    return {"profile": profile, "major": 1, "trigger": trigger or {"kind": "profile"},
            "requires": [{"profile": requires_profile, "major": 1, "features": requires_features}]}


def item(feature, profile="execution"):
    return {"profile": profile, "feature": feature, "reason": "dependency_not_selected"}


BASE = [
    entry("core-test", []),
    entry("execution", CORE_EXEC),
    entry("evidence", ["core.events"]),
    entry("context", ["core.events"]),
    entry("knowledge", ["core.events"]),
    entry("verification", ["core.events", "core.capabilities"]),
]
CLAIM_ENTRY = entry("execution", ["execution.context_revalidation"],
                    {"kind": "feature", "feature": "execution.claim_revalidation"}, "execution")


def outcome(resp):
    if "error" in resp:
        return resp["error"]["data"]["code"], resp["error"]["data"]["details"].get("unsatisfied")
    r = resp["result"]
    return "ok", {s["name"]: s["features"] for s in r["selected"]}, r["unselected"]


def main() -> int:
    probe = P.probe
    code_of = P.code_of
    with tempfile.TemporaryDirectory() as tmp:
        def fresh(name):
            d = os.path.join(tmp, name)
            os.makedirs(d)
            return P.Conn(d)

        c = fresh("query")
        resp = c.query("core.feature_dependencies", {})
        probe("HM6-QUERY", "before negotiation, exact result", resp.get("result"), {"dependencies": BASE})
        probe("HM6-QUERY", "session still unnegotiated",
              code_of(c.query("core-test.subject.get", {"subject": P.SUBJ})), "negotiation_required")
        probe("HM6-QUERY-ENVELOPE", "payload member", code_of(c.query("core.feature_dependencies", {"x": 1})),
              "invalid_envelope")
        probe("HM6-QUERY-ENVELOPE", "requires before negotiation",
              code_of(c.query("core.feature_dependencies", {}, requires=["core.events"])),
              "unsupported_required_feature")
        probe("HM6-QUERY", "negotiation still succeeds", code_of(c.negotiate()), "ok")
        probe("HM6-QUERY", "after negotiation, same result",
              c.query("core.feature_dependencies", {}).get("result"), {"dependencies": BASE})
        probe("HM6-QUERY", "not already_negotiated, and core-test still usable",
              code_of(c.query("core-test.subject.get", {"subject": P.SUBJ})), "not_found")
        c.close()

        c = fresh("unsupported-trigger")
        probe("HM6-UNSUPPORTED-TRIGGER", "optional execution.claim_revalidation is unknown_feature here",
              outcome(c.negotiate([core(CORE_EXEC), execution(opt=["execution.claim_revalidation"])])),
              ("ok", {"core": CORE_EXEC, "execution": []},
               [{"profile": "execution", "feature": "execution.claim_revalidation", "reason": "unknown_feature"}]))
        c.close()

        # A scratch copy that supports execution.claim_revalidation.
        impl = os.path.join(tmp, "impl")
        shutil.copytree(P.IMPL, impl, ignore=shutil.ignore_patterns("tests", "__pycache__", "*.md"))
        path = os.path.join(impl, "provider.py")
        with open(path, encoding="utf-8") as fh:
            src = fh.read()
        marker = '"execution.output", "execution.context_revalidation"]'
        assert src.count(marker) == 1
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(src.replace(marker, '"execution.output", "execution.context_revalidation", '
                                         '"execution.claim_revalidation"]'))
        P.PROVIDER = path

        c = fresh("feature-entry")
        probe("HM6-QUERY", "feature entry follows the execution profile entry",
              c.query("core.feature_dependencies", {}).get("result"),
              {"dependencies": BASE[:2] + [CLAIM_ENTRY] + BASE[2:]})
        c.close()

        cases = [
            ("required feature, required profile",
             [core(CORE_EXEC), execution(req=["execution.claim_revalidation"])],
             ("unsupported_required_feature", [item("execution.claim_revalidation")])),
            ("required feature, optional profile",
             [core(CORE_EXEC), execution(required=False, req=["execution.claim_revalidation"],
                                         opt=["execution.steering", "nope.feature"])],
             ("ok", {"core": CORE_EXEC}, [item("execution.claim_revalidation")])),
            ("optional feature",
             [core(CORE_EXEC), execution(req=["execution.steering"], opt=["execution.claim_revalidation"])],
             ("ok", {"core": CORE_EXEC, "execution": ["execution.steering"]}, [item("execution.claim_revalidation")])),
            ("dependency requested too",
             [core(CORE_EXEC), execution(req=["execution.claim_revalidation"], opt=["execution.context_revalidation"])],
             ("ok", {"core": CORE_EXEC, "execution": ["execution.claim_revalidation", "execution.context_revalidation"]},
              [])),
            ("neither requested", [core(CORE_EXEC), execution()], ("ok", {"core": CORE_EXEC, "execution": []}, [])),
            ("missing Core feature and feature dependency (HM6-REFUSAL-CODE)",
             [core(["core.events", "core.capabilities"]), execution(req=["execution.claim_revalidation"])],
             ("unsupported_profile", [item("core.effects"), item("execution.claim_revalidation")])),
            ("missing Core feature, both features requested (HM6-DEPENDENCY-STATE)",
             [core(["core.events", "core.capabilities"]),
              execution(req=["execution.claim_revalidation", "execution.context_revalidation"])],
             ("unsupported_profile", [item("core.effects")])),
            ("optional profile missing Core feature, optional feature (HM6-DEPENDENCY-STATE)",
             [core(["core.events"]), execution(required=False, opt=["execution.claim_revalidation"])],
             ("ok", {"core": ["core.events"]}, [item("core.capabilities"), item("core.effects")])),
        ]
        for n, (label, profiles, want) in enumerate(cases):
            c = fresh(f"case-{n}")
            probe("HM6-FEATURE-DEP", label, outcome(c.negotiate(profiles)), want)
            c.close()

    print(f"{sum(P.RESULTS)}/{len(P.RESULTS)} probes as documented")
    return 0 if all(P.RESULTS) else 1


if __name__ == "__main__":
    sys.exit(main())
