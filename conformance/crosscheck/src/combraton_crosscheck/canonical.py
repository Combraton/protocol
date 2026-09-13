"""Canonical form and digests (ENCODING sections 2-4), using the vetted rfc8785 package."""
from __future__ import annotations

import hashlib
import re

import rfc8785

from .strictjson import check_domain

DIGEST_GRAMMAR = re.compile(r"^[a-z0-9]+([+._-][a-z0-9]+)*:[0-9a-f]+$")
ALGORITHMS = {"sha256": (hashlib.sha256, 64), "sha512": (hashlib.sha512, 128)}

INTENT_MEMBERS = ("operation", "subject", "preconditions", "requires", "payload")


def canonical_bytes(value) -> bytes:
    check_domain(value)
    return rfc8785.dumps(value)


def digest_bytes(data: bytes, algorithm: str = "sha256") -> str:
    fn, _ = ALGORITHMS[algorithm]
    return f"{algorithm}:{fn(data).hexdigest()}"


def intent(envelope: dict) -> dict:
    """Command intent object (CORE section 6.1)."""
    obj = {name: envelope[name] for name in INTENT_MEMBERS if name in envelope}
    required_ext = {
        key: value
        for key, value in (envelope.get("extensions") or {}).items()
        if key in (envelope.get("requires") or [])
    }
    obj["extensions"] = required_ext
    return obj


def intent_digest(envelope: dict, algorithm: str = "sha256") -> str:
    return digest_bytes(canonical_bytes(intent(envelope)), algorithm)
