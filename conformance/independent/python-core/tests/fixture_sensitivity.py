"""Fixture sensitivity: which deliberate deviations does the suite notice?

Copies this implementation to a temporary directory, applies one small
source edit at a time (each a behavior the documents require, or a choice
recorded in DIVERGENCES.md), runs the black-box conformance runner against
the copy and reports how many fixtures fail. A deviation with 0 failing
fixtures is a behavior no fixture checks.

Run from the repository root after `cargo build -p combraton-conformance --locked`:

    python3 conformance/independent/python-core/tests/fixture_sensitivity.py
"""
import json, os, shutil, subprocess, sys, tempfile
REPO = os.getcwd()
SCR = tempfile.mkdtemp(prefix="indep-py-sensitivity-")
IMPL = os.path.join(REPO, "conformance/independent/python-core")
V = [
 ("binding-level errors use retry same_command", "provider.py", '"parse_error": "no", "invalid_utf8": "no", "frame_too_large": "no", "invalid_request": "no"', '"parse_error": "same_command", "invalid_utf8": "same_command", "frame_too_large": "same_command", "invalid_request": "same_command"'),
 ("exit status 3 at end of input", "provider.py", "    serve(Provider(config, store))\n    return 0", "    serve(Provider(config, store))\n    return 3"),
 ("limits off-by-one (>= instead of >)", "provider.py", "            if observed > lim[name]:", "            if observed >= lim[name]:"),
 ("max_payload_bytes ignored", "provider.py", 'if "payload" in params and len(V.canonical(params["payload"])) > lim["max_payload_bytes"]:', 'if False:'),
 ("max_array_items ignored", "provider.py", '            ("max_array_items", longest_array),\n', ''),
 ("frame limit never raised by negotiation", "provider.py", '        s.recv_limit = self.limits["max_frame_bytes"]\n', ''),
 ("precondition_failed lists only first failure", "provider.py", '                failed.append({"subject": subj, "expected": entry["revision"], "current": current})', '                failed.append({"subject": subj, "expected": entry["revision"], "current": current}); break'),
 ("epoch checked after preconditions", "provider.py", '        current_epoch = self.store.revision(auth["kind"], auth["id"])\n', '        self.check_preconditions(env)\n        current_epoch = self.store.revision(auth["kind"], auth["id"])\n'),
 ("noncharacters accepted", "valuedomain.py", '    if _NONCHAR.search(s):', '    if False:'),
 ("lone surrogate escapes accepted", "valuedomain.py", '                s = s.encode("utf-16-le", "surrogatepass").decode("utf-16-le")', '                s = s.encode("utf-16-le", "surrogatepass").decode("utf-16-le", "surrogatepass")'),
 ("-0 accepted", "valuedomain.py", '            if tok == "-0":', '            if False:'),
 ("failed negotiation blocks retry (already_negotiated)", "provider.py", '        if s.negotiated:\n            raise ProtocolError("already_negotiated")\n', '        if getattr(s, "tried", False):\n            raise ProtocolError("already_negotiated")\n        s.tried = True\n'),
 ("requires uniqueness not enforced", "envelope.py", 'req = array(env["requires"], "/requires", max_items=64, unique=True)', 'req = array(env["requires"], "/requires", max_items=64)'),
 ("query requires not checked", "provider.py", '        if unsatisfied:\n', '        if unsatisfied and kind == "command":\n'),
 ("primary-subject precondition not mandatory", "envelope.py", '    if not any(e["subject"] == env["subject"] for e in env["preconditions"]):', '    if False:'),
 ("unknown method before negotiation -> negotiation_required", "provider.py", '        if op is None:\n            raise ProtocolError("method_not_found", {"operation": method})', '        if op is None:\n            raise ProtocolError("method_not_found" if self.session.negotiated else "negotiation_required", {"operation": method})'),
 ("unknown request members accepted", "provider.py", '            or set(msg) - {"jsonrpc", "id", "method", "params"}\n', ''),
 ("non-null invalid ids accepted (bool/empty)", "provider.py", '    if isinstance(rid, bool):\n        return False', '    if isinstance(rid, bool):\n        return True'),
 ("negotiation code precedence reversed", "provider.py", '            if reasons & {"unknown_profile", "declared_unsupported", "dependency_not_selected"}:\n                code = "unsupported_profile"\n            elif "no_common_major" in reasons:\n                code = "unsupported_version"', '            if "no_common_major" in reasons:\n                code = "unsupported_version"\n            elif reasons & {"unknown_profile", "declared_unsupported", "dependency_not_selected"}:\n                code = "unsupported_profile"'),
 ("correlation included in digest intent", "provider.py", '            "extensions": {k: v for k, v in extensions.items() if k in requires},\n        }', '            "extensions": {k: v for k, v in extensions.items() if k in requires},\n        }\n        if "correlation" in env: intent["correlation"] = env["correlation"]'),
 ("optional feature selected even if unsupported", "provider.py", '                if f in support["features"]:', '                if True:'),
 ("unknown_extensions field says preserve but drops", "provider.py", '"unknown_extensions": "drop",', '"unknown_extensions": "preserve",'),
 ("records discarded below current regardless of retain", "state.py", '            self.db.execute("DELETE FROM commands WHERE generation < ?", (oldest,))', '            self.db.execute("DELETE FROM commands WHERE generation < ?", (current,))'),
]
desc = json.load(open("conformance/participants/independent-python-core.json"))
for name, fname, old, new in V:
    vdir = os.path.join(SCR, "variant")
    shutil.rmtree(vdir, ignore_errors=True)
    shutil.copytree(IMPL, vdir, ignore=shutil.ignore_patterns("__pycache__"))
    p = os.path.join(vdir, fname)
    src = open(p).read()
    assert old in src, name
    open(p, "w").write(src.replace(old, new, 1))
    d = json.loads(json.dumps(desc)); d["launch"]["argv"][1] = os.path.join(vdir, "provider.py")
    json.dump(d, open(os.path.join(SCR, "variant.json"), "w"))
    out = subprocess.run(["./target/debug/combraton-conformance", "run", "--participant", os.path.join(SCR, "variant.json"), "--out", os.path.join(SCR, "variant-results")], capture_output=True, text=True)
    fails = [l.split()[1] for l in out.stdout.splitlines() if l.startswith("fail")]
    print(f"{len(fails):2d} failing | {name}" + (f" | {', '.join(fails)}" if fails else ""))
shutil.rmtree(SCR, ignore_errors=True)
