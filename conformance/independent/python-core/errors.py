"""The symbolic protocol error raised by operation handlers (CORE 12)."""

from __future__ import annotations


class ProtocolError(Exception):
    def __init__(self, code: str, details: dict | None = None, message: str | None = None):
        super().__init__(message or code)
        self.code = code
        self.details = details if details is not None else {}
        self.message = message or code.replace("_", " ")


def denied(reason: str) -> ProtocolError:
    return ProtocolError("permission_denied", {"reason": reason})
