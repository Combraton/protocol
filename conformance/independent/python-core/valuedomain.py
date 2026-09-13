"""The protocol JSON value domain, canonical form and digests.

Written from docs/spec/bindings/ENCODING.md and STREAM.md section 1 only.

- ``loads`` is a strict, iterative JSON parser for the value domain
  (ENCODING section 1): no duplicate member names, strings of Unicode scalar
  values without noncharacters, integer-only numbers within +-(2^53 - 1),
  no ``-0``. It never recurses, so arbitrarily deep input cannot crash it.
- ``canonical`` is RFC 8785 restricted to that domain (ENCODING section 2).
- ``digest`` formats ``<algorithm>:<lowercase hex>`` (ENCODING section 3).
"""

from __future__ import annotations

import hashlib
import re

MAX_SAFE = 2**53 - 1


class ParseError(Exception):
    """The text is not JSON or is outside the value domain."""


_WS = re.compile(r"[ \t\r\n]*")
_NUM = re.compile(r"-?(?:0|[1-9][0-9]*)")
# A string body: runs of ordinary characters, each backslash followed by one
# character. Raw control characters U+0000-U+001F are not allowed (RFC 8259 7).
_STR = re.compile(r'"([^"\\\x00-\x1f]*(?:\\.[^"\\\x00-\x1f]*)*)"', re.S)
_ESC = re.compile(r'\\(?:(["\\/bfnrt])|u([0-9a-fA-F]{4})|(.))', re.S)
_SIMPLE_ESC = {'"': '"', "\\": "\\", "/": "/", "b": "\b", "f": "\f", "n": "\n", "r": "\r", "t": "\t"}
_SURROGATE = re.compile("[\ud800-\udfff]")
_NONCHAR = re.compile(
    "[\ufdd0-\ufdef"
    + "".join(chr((plane << 16) | 0xFFFE) + chr((plane << 16) | 0xFFFF) for plane in range(17))
    + "]"
)


def _esc_sub(m: re.Match) -> str:
    if m.group(1) is not None:
        return _SIMPLE_ESC[m.group(1)]
    if m.group(2) is not None:
        return chr(int(m.group(2), 16))
    raise ParseError("invalid escape sequence")


def _parse_string(text: str, i: int) -> tuple[str, int]:
    m = _STR.match(text, i)
    if m is None:
        raise ParseError("invalid or unterminated string")
    s = m.group(1)
    if "\\" in s:
        s = _ESC.sub(_esc_sub, s)
        if _SURROGATE.search(s):
            # Combine escaped surrogate pairs; a lone surrogate fails strictly.
            try:
                s = s.encode("utf-16-le", "surrogatepass").decode("utf-16-le")
            except UnicodeDecodeError:
                raise ParseError("unpaired surrogate") from None
    if _NONCHAR.search(s):
        raise ParseError("noncharacter in string")
    return s, m.end()


def _ws(text: str, i: int) -> int:
    return _WS.match(text, i).end()


def _parse_key(text: str, i: int) -> tuple[str, int]:
    if i >= len(text) or text[i] != '"':
        raise ParseError("expected member name")
    key, i = _parse_string(text, i)
    i = _ws(text, i)
    if i >= len(text) or text[i] != ":":
        raise ParseError("expected ':'")
    return key, _ws(text, i + 1)


def loads(text: str):
    """Parse one JSON text in the value domain. Raises ParseError."""
    n = len(text)
    i = _ws(text, 0)
    stack: list[list] = []  # [container, pending member name]
    while True:
        if i >= n:
            raise ParseError("unexpected end of text")
        ch = text[i]
        if ch == "{":
            i = _ws(text, i + 1)
            if i < n and text[i] == "}":
                val = {}
                i += 1
            else:
                key, i = _parse_key(text, i)
                stack.append([{}, key])
                continue
        elif ch == "[":
            i = _ws(text, i + 1)
            if i < n and text[i] == "]":
                val = []
                i += 1
            else:
                stack.append([[], None])
                continue
        elif ch == '"':
            val, i = _parse_string(text, i)
        elif text.startswith("true", i):
            val, i = True, i + 4
        elif text.startswith("false", i):
            val, i = False, i + 5
        elif text.startswith("null", i):
            val, i = None, i + 4
        else:
            m = _NUM.match(text, i)
            if m is None:
                raise ParseError("unexpected character")
            j = m.end()
            if j < n and text[j] in ".eE":
                raise ParseError("number is not an integer")
            tok = m.group(0)
            if tok == "-0":
                raise ParseError("negative zero")
            if len(tok.lstrip("-")) > 16:
                raise ParseError("integer outside safe range")
            val = int(tok)
            if val > MAX_SAFE or val < -MAX_SAFE:
                raise ParseError("integer outside safe range")
            i = j
        # Attach the value to its container, closing containers as needed.
        while True:
            if not stack:
                i = _ws(text, i)
                if i != n:
                    raise ParseError("trailing data after JSON text")
                return val
            top = stack[-1]
            cont = top[0]
            if isinstance(cont, dict):
                if top[1] in cont:
                    raise ParseError("duplicate member name")
                cont[top[1]] = val
            else:
                cont.append(val)
            i = _ws(text, i)
            if i >= n:
                raise ParseError("unexpected end of text")
            ch = text[i]
            if ch == ",":
                i = _ws(text, i + 1)
                if isinstance(cont, dict):
                    top[1], i = _parse_key(text, i)
                break
            if ch == ("}" if isinstance(cont, dict) else "]"):
                i += 1
                stack.pop()
                val = cont
                continue
            raise ParseError("expected ',' or closing bracket")


# ---------------------------------------------------------------- canonical

_CANON_ESC = re.compile(r'[\x00-\x1f"\\]')
_CANON_SHORT = {'"': '\\"', "\\": "\\\\", "\b": "\\b", "\t": "\\t", "\n": "\\n", "\f": "\\f", "\r": "\\r"}


def _canon_str(s: str) -> str:
    return '"' + _CANON_ESC.sub(lambda m: _CANON_SHORT.get(m.group(0)) or "\\u%04x" % ord(m.group(0)), s) + '"'


def _utf16_key(name: str) -> bytes:
    # Big-endian UTF-16 bytes compare exactly like sequences of unsigned
    # 16-bit code units (RFC 8785 section 3.2.3).
    return name.encode("utf-16-be")


def canonical_text(value) -> str:
    """Canonical form as a str (iterative; safe for any depth)."""
    out: list[str] = []
    # Work stack of either literal strings to emit or values to encode.
    work: list = [("v", value)]
    while work:
        tag, item = work.pop()
        if tag == "s":
            out.append(item)
            continue
        v = item
        if v is None:
            out.append("null")
        elif v is True:
            out.append("true")
        elif v is False:
            out.append("false")
        elif isinstance(v, int):
            out.append(str(v))
        elif isinstance(v, str):
            out.append(_canon_str(v))
        elif isinstance(v, list):
            out.append("[")
            work.append(("s", "]"))
            for idx in range(len(v) - 1, -1, -1):
                work.append(("v", v[idx]))
                if idx:
                    work.append(("s", ","))
        elif isinstance(v, dict):
            out.append("{")
            work.append(("s", "}"))
            keys = sorted(v, key=_utf16_key)
            for idx in range(len(keys) - 1, -1, -1):
                k = keys[idx]
                work.append(("v", v[k]))
                work.append(("s", _canon_str(k) + ":"))
                if idx:
                    work.append(("s", ","))
        else:
            raise TypeError("value outside the JSON value domain: %r" % type(v))
    return "".join(out)


def canonical(value) -> bytes:
    return canonical_text(value).encode("utf-8")


# ------------------------------------------------------------------ digests

DIGEST_GRAMMAR = re.compile(r"[a-z0-9]+(?:[+._-][a-z0-9]+)*:[0-9a-f]+")
KNOWN_LENGTHS = {"sha256": 64, "sha512": 128}


def digest(algorithm: str, data: bytes) -> str:
    return algorithm + ":" + hashlib.new(algorithm, data).hexdigest()


# ------------------------------------------------------------------ metrics

def measure(value, string_cap: int) -> tuple[int, int, int]:
    """Return (depth, longest array, longest string in UTF-8 bytes).

    Depth counts nested objects and arrays: a scalar is 0, ``{}`` is 1,
    ``{"a": []}`` is 2. Strings include member names. The string length
    is exact up to ``string_cap + 1`` and saturates above that.
    """
    depth = 0
    longest_array = 0
    longest_string = 0
    stack = [(value, 1)]
    while stack:
        v, d = stack.pop()
        if isinstance(v, str):
            if len(v) > string_cap:
                b = string_cap + 1
            else:
                b = len(v.encode("utf-8"))
            if b > longest_string:
                longest_string = b
        elif isinstance(v, list):
            if d > depth:
                depth = d
            if len(v) > longest_array:
                longest_array = len(v)
            stack.extend((x, d + 1) for x in v)
        elif isinstance(v, dict):
            if d > depth:
                depth = d
            for k, x in v.items():
                stack.append((k, d + 1))
                stack.append((x, d + 1))
    return depth, longest_array, longest_string
