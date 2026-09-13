"""Author and cross-check conformance/vectors/encoding.json with Python (rfc8785).

    uv run python generate_encoding_vectors.py > ../vectors/encoding.json   # regenerate
    uv run python generate_encoding_vectors.py --check ../vectors/encoding.json

--check exits nonzero unless the committed vectors equal what this independent
implementation produces. The Rust runner and crosscheck_encoding_vectors.mjs
(Node `canonicalize`) check the same file.
"""
import hashlib
import json
import sys

from combraton_crosscheck import canonical, strictjson

CANONICAL = [
    ("key-order-ascii", '{"b":1,"a":2}'),
    ("key-order-utf16-rfc8785", '{"\\u20ac":"Euro Sign","\\r":"Carriage Return","\\ufb33":"Hebrew Letter Dalet With Dagesh","1":"One","\\ud83d\\ude00":"Emoji: Grinning Face","\\u0080":"Control","\\u00f6":"Latin Small Letter O With Diaeresis"}'),
    ("nested-order-and-arrays", '{"z":[3,1,2],"a":{"y":true,"x":null},"m":[]}'),
    ("string-escapes", '{"s":"\\u0000\\u001f\\b\\f\\n\\r\\t\\"\\\\/\\u007f\\u00e9"}'),
    ("escaped-and-raw-same-value-escaped", '{"a":"\\u0041"}'),
    ("escaped-and-raw-same-value-raw", '{"a":"A"}'),
    ("whitespace-insignificant", ' { "a" : [ 1 , 2 ] ,\t"b" : { } } '),
    ("nfc-e-acute", '{"v":"\\u00e9"}'),
    ("nfd-e-combining", '{"v":"e\\u0301"}'),
    ("integer-bounds", '[0,-1,9007199254740991,-9007199254740991]'),
    ("scalars", '[true,false,null,"",{},[]]'),
    ("astral-raw", '{"k":"\U0001F600"}'),
]

REJECTED = [
    ("duplicate-member", b'{"a":1,"a":2}', "duplicate member name"),
    ("nested-duplicate-member", b'{"o":{"x":1,"x":1}}', "duplicate member name"),
    ("fraction", b'{"n":1.0}', "non-integer number"),
    ("exponent", b'{"n":1e2}', "non-integer number"),
    ("negative-zero", b'{"n":-0}', "negative zero"),
    ("above-safe-integer", b'{"n":9007199254740992}', "integer outside safe range"),
    ("below-safe-integer", b'{"n":-9007199254740992}', "integer outside safe range"),
    ("lone-high-surrogate-escape", b'{"s":"\\ud800"}', "unpaired surrogate"),
    ("lone-low-surrogate-escape", b'{"s":"\\udc00"}', "unpaired surrogate"),
    ("noncharacter-fdd0", b'{"s":"\\ufdd0"}', "noncharacter"),
    ("noncharacter-ffff", b'{"s":"\\uffff"}', "noncharacter"),
    ("noncharacter-10ffff", b'{"s":"\\udbff\\udfff"}', "noncharacter"),
    ("noncharacter-key", b'{"\\ufffe":1}', "noncharacter"),
    ("invalid-utf8", b'{"s":"\xc3\x28"}', "invalid UTF-8"),
    ("overlong-utf8", b'{"s":"\xc0\xaf"}', "invalid UTF-8"),
    ("byte-order-mark", b'\xef\xbb\xbf{}', "byte-order mark"),
    ("nan-token", b'{"n":NaN}', "not JSON"),
    ("trailing-garbage", b'{} x', "not JSON"),
]

INTENT_ENVELOPE = {
    "operation": "core-test.subject.put",
    "message_id": "msg-1",
    "command_id": "cmd-1",
    "dedupe_generation": 1,
    "subject": {"kind": "core-test.subject", "id": "s-1"},
    "preconditions": [{"subject": {"kind": "core-test.subject", "id": "s-1"}, "revision": 0}],
    "authority_epoch": 0,
    "requires": ["example.org/audit"],
    "correlation": {"trace": "t-9"},
    "extensions": {"example.org/audit": {"level": 2}, "example.org/note": "not part of intent"},
    "payload": {"value": "café \U0001F600"},
}


def build():
    out = {
        "format": "combraton-encoding-vectors/1",
        "spec": "docs/spec/bindings/ENCODING.md",
        "note": "Inputs are exact bytes in hex. Canonical outputs are exact bytes in hex. Authored by conformance/crosscheck/generate_encoding_vectors.py; checked by that script, by crosscheck_encoding_vectors.mjs and by the Rust runner.",
        "canonical": [],
        "rejected": [],
        "intent": [],
    }
    for vid, text in CANONICAL:
        raw = text.encode("utf-8")
        value = strictjson.loads(raw)
        canon = canonical.canonical_bytes(value)
        out["canonical"].append({
            "id": vid,
            "input_hex": raw.hex(),
            "canonical_hex": canon.hex(),
            "digest": canonical.digest_bytes(canon),
        })
    for vid, raw, reason in REJECTED:
        try:
            strictjson.loads(raw)
        except strictjson.DomainError:
            pass
        else:
            sys.exit(f"vector {vid} was not rejected by the runner")
        out["rejected"].append({"id": vid, "input_hex": raw.hex(), "reason": reason})
    intent = canonical.intent(INTENT_ENVELOPE)
    canon = canonical.canonical_bytes(intent)
    out["intent"].append({
        "id": "put-with-required-and-optional-extension",
        "envelope": INTENT_ENVELOPE,
        "intent_canonical_hex": canon.hex(),
        "command_digest": canonical.digest_bytes(canon),
    })
    return out


def main():
    rendered = json.dumps(build(), indent=2, ensure_ascii=False) + "\n"
    if len(sys.argv) == 3 and sys.argv[1] == "--check":
        with open(sys.argv[2], encoding="utf-8") as handle:
            committed = handle.read()
        if committed != rendered:
            sys.exit("encoding vectors differ from the Python cross-check implementation")
        print("encoding vectors match the Python (rfc8785) cross-check")
        return
    sys.stdout.write(rendered)


if __name__ == "__main__":
    main()
