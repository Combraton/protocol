"""Check valuedomain.py against conformance/vectors/encoding.json.

Usage: python3 conformance/independent/python-core/tests/check_vectors.py [vectors.json]
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
import valuedomain as vd  # noqa: E402

path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "..", "..", "..", "vectors", "encoding.json")
vectors = json.load(open(path, encoding="utf-8"))
failures = 0


def parse_bytes(raw):
    return vd.loads(raw.decode("utf-8"))


for v in vectors["canonical"]:
    value = parse_bytes(bytes.fromhex(v["input_hex"]))
    got = vd.canonical(value)
    ok = got.hex() == v["canonical_hex"] and vd.digest("sha256", got) == v["digest"]
    failures += not ok
    print(("ok  " if ok else "FAIL"), "canonical", v["id"])

for v in vectors["rejected"]:
    try:
        parse_bytes(bytes.fromhex(v["input_hex"]))
        ok = False
    except (vd.ParseError, UnicodeDecodeError):
        ok = True
    failures += not ok
    print(("ok  " if ok else "FAIL"), "rejected", v["id"])

for v in vectors["intent"]:
    env = v["envelope"]
    req = env.get("requires", [])
    intent = {
        "operation": env["operation"],
        "subject": env["subject"],
        "preconditions": env["preconditions"],
        "requires": req,
        "payload": env["payload"],
        "extensions": {k: x for k, x in env.get("extensions", {}).items() if k in req},
    }
    got = vd.canonical(intent)
    ok = got.hex() == v["intent_canonical_hex"] and vd.digest("sha256", got) == v["command_digest"]
    failures += not ok
    print(("ok  " if ok else "FAIL"), "intent", v["id"])

print("failures:", failures)
sys.exit(1 if failures else 0)
