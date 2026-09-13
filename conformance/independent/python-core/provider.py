#!/usr/bin/env python3
"""combraton-independent-python-core: a spec-only Core provider.

Written from docs/spec/profiles/CORE.md, docs/spec/bindings/STREAM.md,
docs/spec/bindings/ENCODING.md and schemas/** only, as an independent check
of the Protocol 0.1 conformance fixtures. Not a product.

    python3 provider.py --data-dir DIR --config FILE

Speaks the stdio form of stream/1 (STREAM 1-5) and implements core/1 plus the
conformance-only core-test/1 profile (CORE 13). Standard library only.
"""

from __future__ import annotations

import argparse
import os
import sqlite3
import sys
import threading

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

import envelope as E  # noqa: E402
import valuedomain as V  # noqa: E402
from state import Store  # noqa: E402

PROVIDER = {"name": "combraton-independent-python-core", "version": "0.1.0-dev.0"}
MIB = 1048576

SUPPORTED_PROFILES = {
    "core": {"majors": [1], "features": ["core.digest-sha512"], "depends_on": []},
    "core-test": {"majors": [1], "features": [], "depends_on": ["core"]},
}
DECLARED_UNSUPPORTED = [
    {"name": "coordination", "reason": "not_in_release"},
    {"name": "remote-trust", "reason": "not_in_release"},
]
DEFAULT_LIMITS = {
    "max_frame_bytes": MIB,
    "max_payload_bytes": 262144,
    "max_string_bytes": 65536,
    "max_array_items": 1024,
    "max_depth": 32,
}
LIMIT_RANGES = {
    "max_frame_bytes": (MIB, V.MAX_SAFE),
    "max_payload_bytes": (1, V.MAX_SAFE),
    "max_string_bytes": (1, V.MAX_SAFE),
    "max_array_items": (1, V.MAX_SAFE),
    "max_depth": (1, 1024),
}

# CORE 12 retry classes, plus the binding-level codes of STREAM 2-3 (whose
# retry class the documents leave open; see DIVERGENCES.md D-ERR-RETRY).
RETRY = {
    "parse_error": "no", "invalid_utf8": "no", "frame_too_large": "no", "invalid_request": "no",
    "overloaded": "same_command",
    "invalid_envelope": "no", "limit_exceeded": "no", "negotiation_required": "after_renegotiate",
    "already_negotiated": "no", "method_not_found": "no", "profile_not_negotiated": "after_renegotiate",
    "unsupported_version": "no", "unsupported_profile": "no", "unsupported_required_feature": "no",
    "unsupported_digest_algorithm": "no", "digest_mismatch": "no", "idempotency_conflict": "no",
    "dedupe_history_unavailable": "after_reconcile", "stale_authority_epoch": "after_reconcile",
    "unknown_authority_epoch": "no", "precondition_failed": "after_reconcile", "not_found": "no",
    "permission_denied": "no", "unavailable": "same_command", "internal_error": "after_reconcile",
}
RPC_CODE = {
    "parse_error": -32700, "invalid_utf8": -32700, "frame_too_large": -32010,
    "invalid_request": -32600, "method_not_found": -32601, "overloaded": -32011,
}


def log(*parts) -> None:
    print("[independent-python-core]", *parts, file=sys.stderr, flush=True)


class ProtocolError(Exception):
    def __init__(self, code: str, details: dict | None = None, message: str | None = None):
        super().__init__(message or code)
        self.code = code
        self.details = details if details is not None else {}
        self.message = message or code.replace("_", " ")


def error_object(rid, code: str, details: dict | None = None, message: str | None = None) -> dict:
    return {
        "jsonrpc": "2.0",
        "id": rid,
        "error": {
            "code": RPC_CODE.get(code, 1),
            "message": (message or code.replace("_", " "))[:1024],
            "data": {"code": code, "retry": RETRY[code], "details": details or {}},
        },
    }


# ------------------------------------------------------------------ config

class ConfigError(Exception):
    pass


def load_config(path: str) -> dict:
    with open(path, "rb") as fh:
        raw = fh.read()
    try:
        cfg = V.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, V.ParseError) as exc:
        raise ConfigError(f"config is not value-domain JSON: {exc}") from None
    if not isinstance(cfg, dict):
        raise ConfigError("config must be an object")
    unknown = sorted(set(cfg) - {"format", "principal", "limits", "dedupe"})
    if unknown:
        raise ConfigError(f"unknown config keys: {unknown}")
    if cfg.get("format") != "combraton-conformance-config/1":
        raise ConfigError("config format must be combraton-conformance-config/1")
    if "principal" not in cfg:
        raise ConfigError("config must assign a principal (STREAM 5)")
    limits = dict(DEFAULT_LIMITS)
    overrides = cfg.get("limits", {})
    if not isinstance(overrides, dict):
        raise ConfigError("limits must be an object")
    for key, value in overrides.items():
        if key not in LIMIT_RANGES:
            raise ConfigError(f"unknown limit {key}")
        lo, hi = LIMIT_RANGES[key]
        if not E.is_int(value) or not lo <= value <= hi:
            raise ConfigError(f"limit {key} out of range")
        limits[key] = value
    dedupe = cfg.get("dedupe", {})
    if not isinstance(dedupe, dict) or set(dedupe) - {"advance_on_start", "retain_generations"}:
        raise ConfigError("dedupe must be an object with advance_on_start and/or retain_generations")
    advance = dedupe.get("advance_on_start", 0)
    retain = dedupe.get("retain_generations")
    if not E.is_int(advance) or advance < 0:
        raise ConfigError("dedupe.advance_on_start must be a non-negative integer")
    if retain is not None and (not E.is_int(retain) or retain < 1):
        raise ConfigError("dedupe.retain_generations must be a positive integer")
    return {
        # The deduplication scope is the principal's identity; any JSON value
        # is accepted and compared by its canonical form.
        "scope": V.canonical_text(cfg["principal"]),
        "limits": limits,
        "advance": advance,
        "retain": retain,
    }


# ----------------------------------------------------------------- session

class Session:
    """One stdio connection (CORE 3). Nothing here is durable."""

    def __init__(self):
        self.negotiated = False
        self.selected: dict[str, dict] = {}
        self.recv_limit = MIB  # STREAM 1.5: 1 MiB until negotiation completes
        self.send_limit = MIB

    def feature_selected(self, feature: str) -> bool:
        return any(feature in p["features"] for p in self.selected.values())

    def digest_algorithms(self) -> list[str]:
        algorithms = ["sha256"]
        if self.feature_selected("core.digest-sha512"):
            algorithms.append("sha512")
        return algorithms


class Provider:
    def __init__(self, config: dict, store: Store):
        self.scope = config["scope"]
        self.limits = config["limits"]
        self.store = store
        self.session = Session()
        self.ops = {
            # operation: (profile, kind, validator, handler)
            "core.describe": ("core", "query", E.describe_params, self.op_describe),
            "core.negotiate": ("core", "query", E.negotiate_params, self.op_negotiate),
            "core-test.authority.claim": ("core-test", "command", E.claim_params, self.op_claim),
            "core-test.subject.put": ("core-test", "command", E.put_params, self.op_put),
            "core-test.subject.get": ("core-test", "query", E.subject_query_params, self.op_get),
            "core-test.subject.applied_count": ("core-test", "query", E.subject_query_params, self.op_applied_count),
        }

    # ------------------------------------------------------------ output
    def write_frame(self, obj: dict) -> None:
        data = V.canonical(obj)
        if len(data) > self.session.send_limit:
            # The caller could not accept this frame (STREAM 1.5). A bound
            # command's outcome is then unknown to the caller.
            data = V.canonical(error_object(obj.get("id"), "internal_error", {},
                                            "response exceeds the caller's receive limit"))
        write_all(data + b"\n")

    # ------------------------------------------------------------- frames
    def handle_frame(self, frame: bytes) -> None:
        try:
            text = frame.decode("utf-8")
        except UnicodeDecodeError:
            fatal("invalid_utf8", "frame is not valid UTF-8")
        if text.strip(" \t\r") == "":
            return  # STREAM 1.4: blank frames are ignored
        try:
            msg = V.loads(text)
        except V.ParseError as exc:
            fatal("parse_error", f"frame is not a value-domain JSON text: {exc}")

        if not isinstance(msg, dict):
            self.write_frame(error_object(None, "invalid_request", {}, "a frame must be one JSON-RPC request object"))
            return
        if "id" not in msg:
            if msg.get("jsonrpc") == "2.0" and isinstance(msg.get("method"), str):
                log("ignored a notification for method", repr(msg["method"])[:140])
                return  # STREAM 3: not processed, not answered
            self.write_frame(error_object(None, "invalid_request", {}, "malformed JSON-RPC object without id"))
            return
        rid = msg["id"]
        if not valid_request_id(rid):
            self.write_frame(error_object(None, "invalid_request", {}, "request id must be a 1-128 byte string or a safe integer"))
            return
        if (
            msg.get("jsonrpc") != "2.0"
            or set(msg) - {"jsonrpc", "id", "method", "params"}
            or not isinstance(msg.get("method"), str)
            or not 1 <= len(msg["method"]) <= 128
            or not isinstance(msg.get("params"), dict)
        ):
            self.write_frame(error_object(rid, "invalid_request", {}, "malformed JSON-RPC request"))
            return
        try:
            result = self.dispatch(msg["method"], msg["params"])
            response = {"jsonrpc": "2.0", "id": rid, "result": result}
        except ProtocolError as exc:
            response = error_object(rid, exc.code, exc.details, exc.message)
        except Exception as exc:  # undefined failure: outcome unknown (CORE 12)
            log("internal error:", repr(exc))
            self.store.rollback()
            response = error_object(rid, "internal_error", {}, "internal error")
        self.write_frame(response)

    # ---------------------------------------------------------- CORE 10
    def dispatch(self, method: str, params: dict) -> dict:
        s = self.session
        # Step 1: operation known; session negotiated; profile selected.
        op = self.ops.get(method)
        if op is None:
            raise ProtocolError("method_not_found", {"operation": method})
        profile, kind, validator, handler = op
        if method not in ("core.describe", "core.negotiate"):
            if not s.negotiated:
                raise ProtocolError("negotiation_required")
            if profile not in s.selected:
                raise ProtocolError("profile_not_negotiated", {"profile": profile})
        # Step 2: limits, closed objects, types.
        self.check_limits(params)
        try:
            validator(params, method)
        except E.Invalid as exc:
            raise ProtocolError("invalid_envelope", {"path": exc.path, "reason": exc.reason}) from None
        # Step 3: every requires entry negotiated and understood.
        unsatisfied = [
            item for item in params.get("requires", [])
            if "/" in item or not s.feature_selected(item)  # no extension is understood
        ]
        if unsatisfied:
            raise ProtocolError("unsupported_required_feature", {"features": unsatisfied})
        if kind == "query":
            return handler(params)
        return self.run_command(params, handler)

    def check_limits(self, params: dict) -> None:
        lim = self.limits
        depth, longest_array, longest_string = V.measure(params, lim["max_string_bytes"])
        checks = [
            ("max_depth", depth),
            ("max_array_items", longest_array),
            ("max_string_bytes", longest_string),
        ]
        for name, observed in checks:
            if observed > lim[name]:
                raise ProtocolError("limit_exceeded", {"limit": name, "maximum": lim[name]})
        if "payload" in params and len(V.canonical(params["payload"])) > lim["max_payload_bytes"]:
            raise ProtocolError("limit_exceeded", {"limit": "max_payload_bytes", "maximum": lim["max_payload_bytes"]})

    def run_command(self, env: dict, handler) -> dict:
        # Step 4: digest algorithm supported; digest matches recomputed intent.
        algorithm = env["command_digest"].split(":", 1)[0]
        supported = self.session.digest_algorithms()
        if algorithm not in supported:
            raise ProtocolError("unsupported_digest_algorithm", {"algorithm": algorithm, "supported": supported})
        requires = env["requires"]
        extensions = env.get("extensions", {})
        intent = {
            "operation": env["operation"],
            "subject": env["subject"],
            "preconditions": env["preconditions"],
            "requires": requires,
            "payload": env["payload"],
            "extensions": {k: v for k, v in extensions.items() if k in requires},
        }
        recomputed = V.digest(algorithm, V.canonical(intent))
        if recomputed != env["command_digest"]:
            raise ProtocolError("digest_mismatch", {"expected": recomputed})

        st = self.store
        try:
            st.begin()
        except sqlite3.Error as exc:
            log("cannot begin transaction:", repr(exc))
            raise ProtocolError("unavailable") from None
        try:
            # Step 5: deduplication lookup (CORE 6.3 table, in order).
            window = st.window()
            generation = env["dedupe_generation"]
            if generation > window["current"]:
                raise ProtocolError("invalid_envelope", {
                    "path": "/dedupe_generation",
                    "reason": "generation was never issued by this provider",
                })
            record = st.command_record(self.scope, env["command_id"])
            if record is not None:
                _, stored_digest, ack, outcome = record
                if stored_digest != env["command_digest"]:
                    raise ProtocolError("idempotency_conflict", {"command_id": env["command_id"]})
                st.rollback()
                return {"acknowledgment": V.loads(bytes(ack).decode("utf-8")),
                        "outcome": V.loads(bytes(outcome).decode("utf-8")),
                        "replay": True}
            if generation < window["oldest_retained"]:
                raise ProtocolError("dedupe_history_unavailable", {"oldest_retained": window["oldest_retained"]})
            # Step 6 (authorization) is reserved for M2.
            # Step 7 and the state change of step 8.
            revision, outcome = handler(env)
            ack = {
                "command_id": env["command_id"],
                "command_digest": env["command_digest"],
                "operation_ref": st.next_operation_ref(),
                "subject": env["subject"],
                "revision": revision,
                "effect_refs": [],
            }
            st.bind(self.scope, env["command_id"], generation, env["command_digest"],
                    V.canonical(ack), V.canonical(outcome))
            try:
                st.commit()
            except sqlite3.Error as exc:
                log("commit failed:", repr(exc))
                raise ProtocolError("unavailable") from None
            return {"acknowledgment": ack, "outcome": outcome, "replay": False}
        except BaseException:
            st.rollback()
            raise

    # -------------------------------------------------------- CORE 7, 8
    def check_preconditions(self, env: dict) -> None:
        failed = []
        for entry in env["preconditions"]:
            subj = entry["subject"]
            current = self.store.revision(subj["kind"], subj["id"])
            if current != entry["revision"]:
                failed.append({"subject": subj, "expected": entry["revision"], "current": current})
        if failed:
            raise ProtocolError("precondition_failed", {"failed": failed})

    def op_claim(self, env: dict):
        auth = E.AUTHORITY_SUBJECT
        self.check_preconditions(env)
        epoch = self.store.revision(auth["kind"], auth["id"]) + 1
        self.store.write_subject(auth["kind"], auth["id"], epoch, None)
        return epoch, {"epoch": epoch}

    def op_put(self, env: dict):
        auth = E.AUTHORITY_SUBJECT
        current_epoch = self.store.revision(auth["kind"], auth["id"])
        claimed = env["authority_epoch"]
        if claimed < current_epoch:
            raise ProtocolError("stale_authority_epoch", {"current_epoch": current_epoch})
        if claimed > current_epoch:
            raise ProtocolError("unknown_authority_epoch")
        self.check_preconditions(env)
        subj = env["subject"]
        revision = self.store.revision(subj["kind"], subj["id"]) + 1
        value = env["payload"]["value"]
        # labels exist only for canonical-ordering fixtures and are not stored.
        self.store.write_subject(subj["kind"], subj["id"], revision, value)
        return revision, {"value": value}

    # ----------------------------------------------------------- queries
    def op_describe(self, env: dict) -> dict:
        return {
            "provider": dict(PROVIDER),
            "profiles": [
                {"name": name, "majors": list(p["majors"]), "features": list(p["features"]),
                 "depends_on": list(p["depends_on"])}
                for name, p in SUPPORTED_PROFILES.items()
            ],
            "unsupported_profiles": [dict(u) for u in DECLARED_UNSUPPORTED],
            "limits": dict(self.limits),
            "dedupe_window": self.store.window(),
            # Unknown optional extensions are not stored with subject values.
            "unknown_extensions": "drop",
        }

    def op_negotiate(self, env: dict) -> dict:
        s = self.session
        if s.negotiated:
            raise ProtocolError("already_negotiated")
        payload = env["payload"]
        requested = {p["name"]: p for p in payload["profiles"]}
        order = ["core"] + [p["name"] for p in payload["profiles"] if p["name"] != "core"]
        declared = {u["name"] for u in DECLARED_UNSUPPORTED}
        refusals: list[dict] = []
        unselected: list[dict] = []
        selected: dict[str, dict] = {}
        optional_feature_misses: dict[str, list[dict]] = {}
        for name in order:
            p = requested.get(name)
            if p is None:  # Core is included implicitly (CORE 4.2)
                selected["core"] = {"major": 1, "features": []}
                continue
            # A caller cannot deselect Core, so a Core entry is always treated
            # as required (DIVERGENCES.md D-NEG-CORE).
            sink = refusals if (p["required"] or name == "core") else unselected
            support = SUPPORTED_PROFILES.get(name)
            if support is None:
                sink.append({"profile": name, "reason": "declared_unsupported" if name in declared else "unknown_profile"})
                continue
            common = set(p["majors"]) & set(support["majors"])
            if not common:
                sink.append({"profile": name, "reason": "no_common_major"})
                continue
            missing = [f for f in p["required_features"] if f not in support["features"]]
            if missing:
                sink.extend({"profile": name, "feature": f, "reason": "unknown_feature"} for f in missing)
                continue
            features = list(p["required_features"])
            misses = []
            for f in p["optional_features"]:
                if f in support["features"]:
                    if f not in features:
                        features.append(f)
                else:
                    misses.append({"profile": name, "feature": f, "reason": "unknown_feature"})
            selected[name] = {"major": max(common), "features": features}
            optional_feature_misses[name] = misses
        # Dependencies (REL-2, CORE 4.2 reason dependency_not_selected).
        changed = True
        while changed:
            changed = False
            for name in list(selected):
                deps = SUPPORTED_PROFILES[name]["depends_on"]
                if any(d not in selected for d in deps):
                    del selected[name]
                    p = requested.get(name)
                    sink = refusals if (p is None or p["required"] or name == "core") else unselected
                    sink.append({"profile": name, "reason": "dependency_not_selected"})
                    changed = True
        for name in selected:
            unselected.extend(optional_feature_misses.get(name, []))
        if refusals:
            reasons = {r["reason"] for r in refusals}
            if reasons & {"unknown_profile", "declared_unsupported", "dependency_not_selected"}:
                code = "unsupported_profile"
            elif "no_common_major" in reasons:
                code = "unsupported_version"
            else:
                code = "unsupported_required_feature"
            raise ProtocolError(code, {"unsatisfied": refusals})
        s.negotiated = True
        s.selected = selected
        s.recv_limit = self.limits["max_frame_bytes"]
        s.send_limit = payload["receive_limits"]["max_frame_bytes"]
        return {
            "selected": [{"name": n, "major": v["major"], "features": v["features"]} for n, v in selected.items()],
            "unselected": unselected,
            "limits": dict(self.limits),
            "dedupe_window": self.store.window(),
        }

    def op_get(self, env: dict) -> dict:
        subj = env["payload"]["subject"]
        row = self.store.subject(subj["kind"], subj["id"])
        if row is None:
            raise ProtocolError("not_found")
        return {"subject": subj, "revision": row[0], "value": row[1]}

    def op_applied_count(self, env: dict) -> dict:
        subj = env["payload"]["subject"]
        row = self.store.subject(subj["kind"], subj["id"])
        return {"subject": subj, "applied_count": row[2] if row else 0}


def valid_request_id(rid) -> bool:
    if isinstance(rid, bool):
        return False
    if isinstance(rid, int):
        return -V.MAX_SAFE <= rid <= V.MAX_SAFE
    if isinstance(rid, str):
        return 1 <= len(rid.encode("utf-8")) <= 128
    return False


# -------------------------------------------------------------- transport

OUT_FD = 1


def write_all(data: bytes) -> None:
    view = memoryview(data)
    try:
        while view:
            n = os.write(OUT_FD, view)
            view = view[n:]
    except (BrokenPipeError, OSError) as exc:
        # STREAM 4: the peer is gone; pending responses are undelivered.
        log("output closed by peer:", repr(exc))
        os._exit(0)


def fatal(code: str, message: str) -> None:
    """STREAM 2: one error with id null, flush for at most one second, close."""
    frame = V.canonical(error_object(None, code, {}, message)) + b"\n"
    writer = threading.Thread(target=write_all, args=(frame,), daemon=True)
    writer.start()
    writer.join(1.0)
    log("closing connection after frame-level failure:", code)
    try:
        os.close(OUT_FD)
    except OSError:
        pass
    os._exit(0)


def serve(provider: Provider) -> None:
    buf = bytearray()
    scanned = 0  # bytes of buf already known to hold no line feed
    while True:
        nl = buf.find(b"\n", scanned)
        if nl >= 0:
            frame = bytes(buf[:nl])
            del buf[: nl + 1]
            scanned = 0
            if len(frame) > provider.session.recv_limit:
                fatal("frame_too_large", "frame exceeds the frame limit")
            provider.handle_frame(frame)
            continue
        scanned = len(buf)
        limit = provider.session.recv_limit
        if len(buf) > limit:
            fatal("frame_too_large", "frame exceeds the frame limit")
        # STREAM 1.5: never buffer more than limit + 1 bytes of an unfinished frame.
        chunk = os.read(0, max(1, min(65536, limit + 1 - len(buf))))
        if not chunk:
            # STREAM 1.6: bytes after the last terminator are discarded.
            if buf:
                log(f"discarded {len(buf)} unterminated bytes at end of input")
            return
        buf += chunk


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--data-dir", required=True)
    parser.add_argument("--config", required=True)
    args = parser.parse_args(argv)

    # Standard output carries only frames (STREAM 5): route stray prints to
    # stderr, and keep the protocol descriptors out of any child process.
    sys.stdout = sys.stderr
    for fd in (0, 1):
        try:
            os.set_inheritable(fd, False)
        except OSError:
            pass
    sys.setrecursionlimit(max(sys.getrecursionlimit(), 5000))

    try:
        config = load_config(args.config)
    except (OSError, ConfigError) as exc:
        log("configuration error:", exc)
        return 2
    store = Store(args.data_dir)
    store.apply_generation_config(config["advance"], config["retain"])
    serve(Provider(config, store))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
