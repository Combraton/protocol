"""Strict JSON value domain for the runner (ENCODING section 1).

Independent of the reference provider's implementation: shared behaviour is
established only through the published documents and vectors.
"""
from __future__ import annotations

import json
import re

MAX_SAFE = 2**53 - 1
_INT_TOKEN = re.compile(r"-?(0|[1-9][0-9]*)")


class DomainError(ValueError):
    """The input is valid JSON but outside the protocol value domain, or not JSON."""


def _pairs(pairs):
    obj = {}
    for key, value in pairs:
        if key in obj:
            raise DomainError(f"duplicate member name {key!r}")
        obj[key] = value
    return obj


def _float(token: str):
    raise DomainError(f"non-integer number {token!r}")


def _int(token: str):
    if not _INT_TOKEN.fullmatch(token) or token == "-0":
        raise DomainError(f"invalid integer token {token!r}")
    value = int(token)
    if abs(value) > MAX_SAFE:
        raise DomainError(f"integer outside safe range {token!r}")
    return value


def _constant(token: str):
    raise DomainError(f"non-finite number {token!r}")


def _is_noncharacter(cp: int) -> bool:
    return 0xFDD0 <= cp <= 0xFDEF or (cp & 0xFFFE) == 0xFFFE


def check_string(value: str) -> None:
    for ch in value:
        cp = ord(ch)
        if 0xD800 <= cp <= 0xDFFF:
            raise DomainError("unpaired surrogate in string")
        if _is_noncharacter(cp):
            raise DomainError(f"noncharacter U+{cp:04X} in string")


def check_domain(value) -> None:
    """Validate an already-constructed Python value against the value domain."""
    if isinstance(value, bool) or value is None:
        return
    if isinstance(value, int):
        if abs(value) > MAX_SAFE:
            raise DomainError("integer outside safe range")
        return
    if isinstance(value, float):
        raise DomainError("non-integer number")
    if isinstance(value, str):
        check_string(value)
        return
    if isinstance(value, list):
        for item in value:
            check_domain(item)
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                raise DomainError("non-string member name")
            check_string(key)
            check_domain(item)
        return
    raise DomainError(f"unsupported value type {type(value).__name__}")


def loads(data: bytes):
    """Parse one frame's bytes. Raises DomainError for any violation."""
    try:
        text = data.decode("utf-8", errors="strict")
    except UnicodeDecodeError as exc:
        raise DomainError(f"invalid UTF-8: {exc}") from None
    if text.startswith("\ufeff"):
        raise DomainError("byte-order mark")
    try:
        value = json.loads(
            text,
            object_pairs_hook=_pairs,
            parse_float=_float,
            parse_int=_int,
            parse_constant=_constant,
        )
    except DomainError:
        raise
    except (json.JSONDecodeError, RecursionError) as exc:
        raise DomainError(f"not JSON: {exc}") from None
    check_domain(value)
    return value


def dumps(value) -> bytes:
    """Compact serialization for frames the runner sends (not canonical)."""
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
