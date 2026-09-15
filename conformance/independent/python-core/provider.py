#!/usr/bin/env python3
"""combraton-independent-python-core: a spec-only Core provider.

Written from docs/spec/profiles/CORE.md, docs/spec/bindings/STREAM.md,
docs/spec/bindings/ENCODING.md and schemas/** only, as an independent check
of the Protocol 0.1 conformance fixtures. Not a product.

    python3 provider.py --data-dir DIR --config FILE

Speaks the stdio form of stream/1 (STREAM 1-5) and implements core/1 plus the
conformance-only core-test/1 profile (CORE 13), including the M2 Core
features core.grants (CORE 15), core.events (CORE 16) and core.capabilities
(CORE 17), the M3 Core feature core.effects (CORE 19) and execution/1
(docs/spec/profiles/EXECUTION.md) over a scripted executor, with the clock
file and store faults of decision 007, and the M4 profiles evidence/1 over a
scripted store and context/1 over scripted preparation. Standard library only.
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

import clock as C  # noqa: E402
import context as CTX  # noqa: E402
import evidence as EVD  # noqa: E402
import envelope as E  # noqa: E402
import events as EV  # noqa: E402
import execution_envelope as XE  # noqa: E402
import grants as G  # noqa: E402
import valuedomain as V  # noqa: E402
from errors import ProtocolError, denied  # noqa: E402
from executor import Executor, ExecutorConfigError, parse_executor_config  # noqa: E402
from state import Store  # noqa: E402

PROVIDER = {"name": "combraton-independent-python-core", "version": "0.1.0-dev.0"}
MIB = 1048576

CORE_FEATURES = ["core.digest-sha512", "core.grants", "core.events", "core.capabilities", "core.effects"]
EXECUTION_FEATURES = ["execution.steering", "execution.actions", "execution.controller", "execution.workspaces",
                      "execution.usage", "execution.context", "execution.discovery", "execution.continuation",
                      "execution.output", "execution.context_revalidation"]
EVIDENCE_FEATURES = ["evidence.manifests", "evidence.retention_control"]
CONTEXT_FEATURES = ["context.advisory", "context.required_before_start", "context.required_before_transition",
                    "context.shared_jobs", "context.updates", "context.expand"]
# EXECUTION 1: the Core features execution/1 requires. Published only in that
# document; core.describe keeps depends_on: ["core"].
EXECUTION_REQUIRED_CORE_FEATURES = ["core.events", "core.capabilities", "core.effects"]
# EVIDENCE 1 and CONTEXT 1: the Core features those profiles require
# (H-EVD-NEG, H-CTX-NEG).
REQUIRED_CORE_FEATURES = {"execution": EXECUTION_REQUIRED_CORE_FEATURES, "evidence": ["core.events"],
                          "context": ["core.events"]}
SUPPORTED_PROFILES = {
    "core": {"majors": [1], "features": CORE_FEATURES, "depends_on": []},
    "core-test": {"majors": [1], "features": [], "depends_on": ["core"]},
    "execution": {"majors": [1], "features": EXECUTION_FEATURES, "depends_on": ["core"]},
    "evidence": {"majors": [1], "features": EVIDENCE_FEATURES, "depends_on": ["core"]},
    "context": {"majors": [1], "features": CONTEXT_FEATURES, "depends_on": ["core"]},
}
IDLE_RECHECK_SECONDS = 0.1  # real time, never the virtual clock (decision 007)
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
DEFAULT_PROVIDER_ID = "conformance-provider"  # conformance README, launch configuration

# CORE 17.4: the only capability this provider knows, and the operations that
# depend on it.
KNOWN_CAPABILITIES = ("core-test.writes",)
OPERATION_CAPABILITIES = {"core-test.subject.put": ("core-test.writes",)}
AUTHORITY_SCOPE = "core-test"
# CORE 16.6: profile subject kinds and the read right a grant needs to see them.
# EXECUTION 11 "base rights": execution.read covers execution events; the
# controller subject follows the same right (G-CONTROLLER-VISIBILITY).
PROFILE_READ_RIGHTS = {"core-test.subject": "core-test.read", "core-test.authority": "core-test.read",
                       XE.EXECUTION_KIND: "execution.read", XE.CONTROLLER_KIND: "execution.read",
                       EVD.ARTIFACT_KIND: "evidence.read", CTX.REQUEST_KIND: "context.read",
                       CTX.PACKET_KIND: "context.packet.read"}
ALL_EXECUTIONS = {"kind": XE.EXECUTION_KIND, "id": None}  # coverage needs a kind-wide resource
CAPABILITIES_KIND = "core.capabilities"
NOTIFY_CHUNK = 100

# CORE 12 retry classes, plus the binding-level codes of STREAM 2-3.
RETRY = {
    "parse_error": "no", "invalid_utf8": "no", "frame_too_large": "no", "invalid_request": "no",
    "overloaded": "same_command",
    "invalid_envelope": "no", "limit_exceeded": "no", "negotiation_required": "after_renegotiate",
    "already_negotiated": "no", "method_not_found": "no", "profile_not_negotiated": "after_renegotiate",
    "unsupported_version": "no", "unsupported_profile": "no", "unsupported_required_feature": "no",
    "unsupported_digest_algorithm": "no", "digest_mismatch": "no", "idempotency_conflict": "no",
    "dedupe_history_unavailable": "after_reconcile", "effect_history_unavailable": "after_reconcile",
    "capability_unavailable": "after_reconcile",
    "stale_authority_epoch": "after_reconcile",
    "unknown_authority_epoch": "no", "precondition_failed": "after_reconcile", "invalid_cursor": "no",
    "not_found": "no", "permission_denied": "no", "unavailable": "same_command", "internal_error": "after_reconcile",
    "authentication_required": "no", "authentication_failed": "no", "already_authenticated": "no",
    # CORE 12, EVIDENCE 6 (M4).
    "upload_offset_mismatch": "after_reconcile", "upload_size_exceeded": "no", "upload_incomplete": "after_reconcile",
    "content_digest_mismatch": "no", "artifact_digest_mismatch": "no", "hold_active": "after_reconcile",
}
RPC_CODE = {
    "parse_error": -32700, "invalid_utf8": -32700, "frame_too_large": -32010,
    "invalid_request": -32600, "method_not_found": -32601, "overloaded": -32011,
}


def log(*parts) -> None:
    print("[independent-python-core]", *parts, file=sys.stderr, flush=True)


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


CONFIG_KEYS = {"format", "principal", "authority_principals", "provider_id", "limits", "dedupe",
               "events", "capabilities", "clock", "executor", "faults", "test_barriers", "evidence_store", "context"}


def _short_string(value, what: str) -> str:
    if not isinstance(value, str) or not 1 <= len(value) <= 128:
        raise ConfigError(f"{what} must be a string of 1-128 characters")
    return value


def load_config(path: str) -> dict:
    with open(path, "rb") as fh:
        raw = fh.read()
    try:
        cfg = V.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, V.ParseError) as exc:
        raise ConfigError(f"config is not value-domain JSON: {exc}") from None
    if not isinstance(cfg, dict):
        raise ConfigError("config must be an object")
    unknown = sorted(set(cfg) - CONFIG_KEYS)
    if unknown:
        raise ConfigError(f"unknown config keys: {unknown}")
    if cfg.get("format") != "combraton-conformance-config/1":
        raise ConfigError("config format must be combraton-conformance-config/1")
    if "principal" not in cfg:
        raise ConfigError("config must assign a principal (STREAM 5)")
    principal = _short_string(cfg["principal"], "principal")
    authorities = cfg.get("authority_principals", [principal])
    if not isinstance(authorities, list):
        raise ConfigError("authority_principals must be an array")
    authorities = [_short_string(a, "authority principal") for a in authorities]
    provider_id = _short_string(cfg.get("provider_id", DEFAULT_PROVIDER_ID), "provider_id")

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

    events = cfg.get("events", {})
    if not isinstance(events, dict) or set(events) - {"new_epoch_on_start", "unvouched_last", "retain_last"}:
        raise ConfigError("events must be an object with new_epoch_on_start, unvouched_last and/or retain_last")
    new_epoch = events.get("new_epoch_on_start", False)
    unvouched_last = events.get("unvouched_last", 0)
    retain_last = events.get("retain_last")
    if not isinstance(new_epoch, bool):
        raise ConfigError("events.new_epoch_on_start must be a boolean")
    if not E.is_int(unvouched_last) or unvouched_last < 0:
        raise ConfigError("events.unvouched_last must be a non-negative integer")
    # "With new_epoch_on_start": without a new epoch there is no previous epoch
    # to vouch for, so the key has no effect (F-UNVOUCHED-CONFIG).
    if retain_last is not None and (not E.is_int(retain_last) or retain_last < 0):
        raise ConfigError("events.retain_last must be a non-negative integer")

    capabilities = cfg.get("capabilities", {})
    if not isinstance(capabilities, dict):
        raise ConfigError("capabilities must be an object")
    for name, status in capabilities.items():
        if name not in KNOWN_CAPABILITIES:
            # A status for a capability this provider does not have cannot be
            # simulated; refuse rather than ignore (E-CAP-CONFIG).
            raise ConfigError(f"unknown capability {name}")
        if status not in ("supported", "unsupported", "unknown"):
            raise ConfigError(f"capability {name} status must be supported, unsupported or unknown")

    clock = cfg.get("clock", {})
    if not isinstance(clock, dict) or set(clock) - {"fixed", "file"} or len(clock) > 1:
        raise ConfigError("clock must be an object with one of fixed and file")
    fixed = clock.get("fixed")
    if fixed is not None and (not isinstance(fixed, str) or not E.INSTANT.fullmatch(fixed)):
        raise ConfigError("clock.fixed must be an instant YYYY-MM-DDTHH:MM:SSZ")
    clock_file = clock.get("file")
    if clock_file is not None and (not isinstance(clock_file, str) or not clock_file):
        raise ConfigError("clock.file must be a path")

    try:
        executor = parse_executor_config(cfg.get("executor", {}))
    except ExecutorConfigError as exc:
        raise ConfigError(str(exc)) from None
    except (TypeError, AttributeError, ValueError) as exc:
        raise ConfigError(f"executor configuration is malformed: {exc!r}") from None

    try:
        evidence_store = EVD.parse_store_config(cfg.get("evidence_store", {}))
        context = CTX.parse_context_config(cfg.get("context", {}))
    except (EVD.EvidenceConfigError, CTX.ContextConfigError) as exc:
        raise ConfigError(str(exc)) from None
    except (TypeError, AttributeError, ValueError) as exc:
        raise ConfigError(f"evidence or context configuration is malformed: {exc!r}") from None

    faults = cfg.get("faults", {})
    parsed_faults = {"commit_unavailable": {}, "response_internal_error": {}}
    if not isinstance(faults, dict) or set(faults) - set(parsed_faults):
        raise ConfigError("faults must be an object with commit_unavailable and/or response_internal_error")
    for kind, entries in faults.items():
        if not isinstance(entries, list):
            raise ConfigError(f"faults.{kind} must be an array")
        for entry in entries:
            if (not isinstance(entry, dict) or set(entry) != {"operation", "times"}
                    or not isinstance(entry["operation"], str) or not E.is_int(entry["times"]) or entry["times"] < 1):
                raise ConfigError(f"faults.{kind} entries are {{operation, times >= 1}}")
            parsed_faults[kind][entry["operation"]] = parsed_faults[kind].get(entry["operation"], 0) + entry["times"]

    barriers = cfg.get("test_barriers")
    if barriers is not None:
        # This implementation declares no barriers or signals (decision 007
        # section 4); a launch that enables one cannot be honoured.
        if not isinstance(barriers, dict) or barriers.get("enabled"):
            raise ConfigError("test_barriers: this provider implements no barriers")

    return {
        "principal": principal,
        # The deduplication scope is the principal's identity, compared by its
        # canonical form (CORE 6.2).
        "scope": V.canonical_text(principal),
        "authorities": authorities,
        "provider_id": provider_id,
        "limits": limits,
        "advance": advance,
        "retain": retain,
        "new_epoch": new_epoch,
        "unvouched_last": unvouched_last,
        "retain_last": retain_last,
        "capabilities": capabilities,
        "clock": fixed,
        "clock_file": clock_file,
        "executor": executor,
        "evidence_store": evidence_store,
        "context": context,
        "faults": parsed_faults,
    }


# ----------------------------------------------------------------- session

class Session:
    """One stdio connection (CORE 3). Nothing here is durable."""

    def __init__(self):
        self.negotiated = False
        self.selected: dict[str, dict] = {}
        self.recv_limit = MIB  # STREAM 1.5: 1 MiB until negotiation completes
        self.send_limit = MIB
        self.subscriptions: dict[str, dict] = {}  # CORE 16.5: end with the session
        self.next_subscription = 1
        self.request_id = None  # JSON-RPC id of the request being answered

    def feature_selected(self, feature: str) -> bool:
        return any(feature in p["features"] for p in self.selected.values())

    def features(self) -> set:
        return {f for p in self.selected.values() for f in p["features"]}

    def digest_algorithms(self) -> list[str]:
        algorithms = ["sha256"]
        if self.feature_selected("core.digest-sha512"):
            algorithms.append("sha512")
        return algorithms


class Auth:
    """The outcome of CORE 10 step 6 for one operation: acting as an authority
    principal (``grant is None``) or under one grant record. A non-authority
    reaches ``grant is None`` only on operations step 6 does not protect
    (``core.grant.*`` follow their own rules, CORE 15.5)."""

    def __init__(self, provider: "Provider", grant: dict | None):
        self.provider = provider
        self.grant = grant

    def _holder_or_issuer(self, grant_id: str) -> bool:
        row = self.provider.store.grant(grant_id)
        return row is not None and self.provider.principal in (row[1]["holder"], row[1]["issuer"])

    def grant_shows(self, subject: dict) -> bool:
        """CORE 16.6 table: whether this grant shows the subject in events."""
        kind = subject["kind"]
        if kind == E.GRANT_KIND:
            return self._holder_or_issuer(subject["id"])
        if kind == CAPABILITIES_KIND:
            return G.grant_covers(self.grant, subject)
        if kind == XE.EFFECT_KIND:
            # CORE 19.4: an effect subject is visible exactly when its target is.
            row = self.provider.store.get_json(XE.EFFECT_KIND, subject["id"])
            return row is not None and self.grant_shows(row[1]["descriptor"]["target"])
        if kind == EVD.HOLD_KIND:
            row = self.provider.store.get_json(EVD.HOLD_KIND, subject["id"])
            return row is not None and self.provider.evidence.hold_visible(self, subject["id"], row[1])
        if kind == CTX.JOB_KIND:
            return self.provider.context.job_visible(self.grant, subject["id"])
        right = PROFILE_READ_RIGHTS.get(kind)
        return right is not None and right in self.grant["rights"] and G.grant_covers(self.grant, subject)

    def sees(self, subject: dict) -> bool:
        """Event and snapshot visibility (CORE 16.6): authority principals see
        every event; under a grant, only what the principal could read."""
        return self.grant is None or self.grant_shows(subject)

    def may_read(self, subject: dict) -> bool:
        """Whether error details may reveal a subject's current revision (CORE
        7, CORE 12 "if permitted")."""
        if self.grant is not None:
            # "Under a grant, the principal may read the subjects that grant
            # would show it in events (16.6)."
            return self.grant_shows(subject)
        if self.provider.is_authority:
            return True  # "An authority principal acting without a grant may read every subject."
        # Not covered by CORE 7: a non-authority acting without a grant on a
        # core.grant.* command may read what core.grant.get shows it
        # (F-PRE-CURRENT-NO-GRANT).
        return subject["kind"] == E.GRANT_KIND and self._holder_or_issuer(subject["id"])


def _core_validator(validator):
    """M1/M2 validators take whether the grant field is allowed."""
    return lambda env, method, features: validator(env, method, "core.grants" in features)


class Provider:
    def __init__(self, config: dict, store: Store, clock: C.Clock):
        self.principal = config["principal"]
        self.scope = config["scope"]
        self.authorities = config["authorities"]
        self.is_authority = self.principal in self.authorities
        self.provider_id = config["provider_id"]
        self.limits = config["limits"]
        self.clock = clock
        self.store = store
        self.session = Session()
        self.executor = Executor(self, config["executor"])
        self.evidence = EVD.Evidence(self, config["evidence_store"])
        self.context = CTX.Context(self, config["context"])
        self.faults = {kind: dict(entries) for kind, entries in config["faults"].items()}
        # One lock serializes request processing and the idle re-check.
        self.lock = threading.RLock()
        x = self.executor
        ev = self.evidence
        cx = self.context
        cv = _core_validator
        self.ops = {
            # operation: (profile, feature, kind, validator, handler)
            "core.describe": ("core", None, "query", cv(E.describe_params), self.op_describe),
            "core.negotiate": ("core", None, "query", cv(E.negotiate_params), self.op_negotiate),
            "core.authenticate": ("core", None, "query", cv(E.authenticate_params), self.op_authenticate),
            "core.grant.issue": ("core", "core.grants", "command", cv(E.grant_issue_params), self.op_grant_issue),
            "core.grant.revoke": ("core", "core.grants", "command", cv(E.grant_revoke_params), self.op_grant_revoke),
            "core.grant.get": ("core", "core.grants", "query", cv(E.grant_get_params), self.op_grant_get),
            "core.events.read": ("core", "core.events", "query", cv(E.events_read_params), self.op_events_read),
            "core.events.subscribe": ("core", "core.events", "query", cv(E.events_subscribe_params), self.op_events_subscribe),
            "core.events.unsubscribe": ("core", "core.events", "query", cv(E.events_unsubscribe_params), self.op_events_unsubscribe),
            "core.capabilities": ("core", "core.capabilities", "query", cv(E.capabilities_params), self.op_capabilities),
            "core.effects.get": ("core", "core.effects", "query", XE.effects_get_params, x.op_effects_get),
            "core.effects.abort_obligation": ("core", "core.effects", "command", XE.effects_abort_params,
                                              x.op_abort_obligation),
            "core-test.authority.claim": ("core-test", None, "command", cv(E.claim_params), self.op_claim),
            "core-test.subject.put": ("core-test", None, "command", cv(E.put_params), self.op_put),
            "core-test.subject.get": ("core-test", None, "query", cv(E.subject_query_params), self.op_get),
            "core-test.subject.applied_count": ("core-test", None, "query", cv(E.subject_query_params), self.op_applied_count),
            "execution.submit": ("execution", None, "command", XE.submit_params, x.op_submit),
            "execution.inspect": ("execution", None, "query", XE.inspect_params, x.op_inspect),
            "execution.cancel": ("execution", None, "command", XE.cancel_params, x.op_cancel),
            "execution.reconcile": ("execution", None, "query", XE.reconcile_params, x.op_reconcile),
            "execution.steer": ("execution", "execution.steering", "command", XE.steer_params, x.op_steer),
            "execution.respond_action": ("execution", "execution.actions", "command", XE.respond_action_params,
                                         x.op_respond_action),
            "execution.controller.claim": ("execution", "execution.controller", "command", XE.controller_claim_params,
                                           x.op_claim),
            "execution.workspace.checkpoint": ("execution", "execution.workspaces", "command", XE.checkpoint_params,
                                               x.op_checkpoint),
            "execution.discovery.list": ("execution", "execution.discovery", "query", XE.discovery_params,
                                         x.op_discovery),
            "execution.output.read": ("execution", "execution.output", "query", XE.output_read_params,
                                      x.op_output_read),
            # EVIDENCE 4, 5, 9
            "evidence.upload.prepare": ("evidence", None, "command", ev.v_prepare, ev.op_prepare),
            "evidence.upload.append": ("evidence", None, "command", ev.v_append, ev.op_append),
            "evidence.seal": ("evidence", None, "command", ev.v_simple(EVD.ARTIFACT_KIND), ev.op_seal),
            "evidence.upload.abandon": ("evidence", None, "command", ev.v_simple(EVD.ARTIFACT_KIND), ev.op_abandon),
            "evidence.inspect": ("evidence", None, "query", ev.v_inspect, ev.op_inspect),
            "evidence.query": ("evidence", None, "query", ev.v_query, ev.op_query),
            "evidence.fetch": ("evidence", None, "query", ev.v_fetch, ev.op_fetch),
            "evidence.hold": ("evidence", "evidence.retention_control", "command", ev.v_hold, ev.op_hold),
            "evidence.release": ("evidence", "evidence.retention_control", "command", ev.v_simple(EVD.HOLD_KIND),
                                 ev.op_release),
            "evidence.purge": ("evidence", "evidence.retention_control", "command", ev.v_purge, ev.op_purge),
            # CONTEXT 3, 6
            "context.request.submit": ("context", None, "command", cx.v_submit, cx.op_submit),
            "context.request.cancel": ("context", None, "command", cx.v_cancel, cx.op_cancel),
            "context.request.inspect": ("context", None, "query", cx.v_request_inspect, cx.op_request_inspect),
            "context.packet.inspect": ("context", None, "query", cx.v_packet_inspect, cx.op_packet_inspect),
            "context.expand": ("context", "context.expand", "query", cx.v_expand, cx.op_expand),
        }

    # ------------------------------------------------------------- clock
    def now(self) -> str:
        """The provider clock (CORE 15.2): the fixed instant, the clock file's
        instant as read for this unit of work, or the system clock."""
        return self.clock.now()

    def tracked_scopes(self) -> tuple:
        # CORE 15.3: the authority scopes this provider tracks.
        return (AUTHORITY_SCOPE, self.executor.controller_scope)

    def epoch_of(self, scope: str) -> int | None:
        # A binding to an untracked scope is refused at issue (CORE 15.3);
        # None never equals a bound epoch.
        if scope == AUTHORITY_SCOPE:
            return self.store.revision(E.AUTHORITY_SUBJECT["kind"], E.AUTHORITY_SUBJECT["id"])
        if scope == self.executor.controller_scope:
            return self.executor.controller_epoch()
        return None

    def head_cursor(self) -> str:
        return EV.encode_cursor(self.store, self.store.head())

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
        with self.lock:
            self.clock.refresh()
            self._handle_frame(frame)

    def tick(self, allow_crash: bool) -> bool:
        """Executor, evidence store and context preparation work that is due
        now, until nothing more can happen."""
        progressed_any = False
        for _ in range(10000):
            progressed = self.executor.tick(allow_crash=allow_crash)
            progressed = self.evidence.tick() or progressed
            progressed = self.context.tick() or progressed
            if not progressed:
                break
            progressed_any = True
        return progressed_any

    def idle_recheck(self) -> None:
        """Executor observations and deadlines advance without a request
        (EXECUTION 4 "Watching", CORE 19.4). Only a tick that committed
        something is followed by notification delivery; an expiry alone is
        reported at the next request, as CORE 16.5 allows on stdio
        (G-IDLE-RECHECK)."""
        with self.lock:
            self.clock.refresh()
            if self.tick(allow_crash=True):
                self.deliver_notifications()

    def _handle_frame(self, frame: bytes) -> None:
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
            self.write_frame(error_object(None, "invalid_request", {}, "request id must be a 1-128 code point string or a safe integer"))
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
        self.session.request_id = rid
        try:
            # Script steps and timeouts due by now are visible to this request
            # ("no later than the provider's next request"). A scripted crash
            # never happens here, only between requests.
            self.tick(allow_crash=False)
            result = self.dispatch(msg["method"], msg["params"])
            response = {"jsonrpc": "2.0", "id": rid, "result": result}
        except ProtocolError as exc:
            response = error_object(rid, exc.code, exc.details, exc.message)
        except Exception as exc:  # undefined failure: outcome unknown (CORE 12)
            log("internal error:", repr(exc))
            self.store.rollback()
            response = error_object(rid, "internal_error", {}, "internal error")
        self.write_frame(response)
        # CORE 16.5: notifications for events a command caused go out after
        # that command's response; a new subscription's backlog after its own.
        self.deliver_notifications()
        # The executor acts on what the request committed (dispatch after the
        # submit's transaction, forwarding after a cancel's) and then reports.
        try:
            if self.tick(allow_crash=True):
                self.deliver_notifications()
        except Exception as exc:
            log("executor tick failed:", repr(exc))
            self.store.rollback()

    # ---------------------------------------------------------- CORE 10
    def dispatch(self, method: str, params: dict) -> dict:
        s = self.session
        # Step 1: operation known; session negotiated; profile selected.
        op = self.ops.get(method)
        if op is None:
            raise ProtocolError("method_not_found", {"operation": method})
        profile, feature, kind, validator, handler = op
        # CORE 3.1: before negotiation only describe, negotiate and authenticate
        # are answered (F-AUTH-STEP1).
        if method not in ("core.describe", "core.negotiate", "core.authenticate"):
            if not s.negotiated:
                raise ProtocolError("negotiation_required")
            if profile not in s.selected:
                raise ProtocolError("profile_not_negotiated", {"profile": profile})
        # Step 2: limits, closed objects, types. An operation's own payload is
        # validated against its schema even when its feature was not selected,
        # so that step 3 can name the feature (E-FEATURE-OP-STEP).
        self.check_limits(params)
        try:
            validator(params, method, s.features())
        except E.Invalid as exc:
            raise ProtocolError("invalid_envelope", {"path": exc.path, "reason": exc.reason}) from None
        # Step 3: every requires entry negotiated and understood; the
        # operation's feature selected.
        unsatisfied = [
            item for item in params.get("requires", [])
            if "/" in item or not s.feature_selected(item)  # no extension is understood
        ]
        if feature is not None and not s.feature_selected(feature) and feature not in unsatisfied:
            unsatisfied.append(feature)
        if method == "context.request.submit":
            # CONTEXT 3: an item obligation whose feature was not negotiated.
            unsatisfied.extend(f for f in self.context.submit_features(params) if f not in unsatisfied)
        if unsatisfied:
            raise ProtocolError("unsupported_required_feature", {"features": unsatisfied})
        if kind == "query":
            # Queries: steps 1-3, then authorization (step 6), then read rules.
            return handler(params, self.authorize(method, params))
        return self.run_command(method, params, handler)

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

    def run_command(self, method: str, env: dict, handler) -> dict:
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
                # A replay skips steps 6 and 7 (CORE 10, 15.5, 17.2).
                return {"acknowledgment": V.loads(bytes(ack).decode("utf-8")),
                        "outcome": V.loads(bytes(outcome).decode("utf-8")),
                        "replay": True}
            if generation < window["oldest_retained"]:
                raise ProtocolError("dedupe_history_unavailable", {"oldest_retained": window["oldest_retained"]})
            # Step 6: authorization (CORE 15.5).
            auth = self.authorize(method, env)
            # Step 7, first part: capabilities the operation depends on
            # (CORE 17.2). The handler checks the epoch, then preconditions.
            self.check_capabilities(method)
            # Step 8: state change, events, effect records and the command
            # record in one owner transaction. Effect descriptors name the
            # command's operation_ref, so it is allocated first.
            operation_ref = st.next_operation_ref()
            if method.startswith("execution.") or method.startswith("core.effects."):
                revision, outcome, changes, effect_refs = handler(env, auth, operation_ref)
            else:
                revision, outcome, changes = handler(env, auth)
                effect_refs = []  # CORE 11: M1 and M2 operations record no effects
            ack = {
                "command_id": env["command_id"],
                "command_digest": env["command_digest"],
                "operation_ref": operation_ref,
                "subject": env["subject"],
                "revision": revision,
                "effect_refs": effect_refs,
            }
            recorded_at = self.now()
            for event_type, subject, event_revision, payload in changes:
                st.append_event({
                    "type": event_type,
                    "subject": subject,
                    "revision": event_revision,
                    "origin": "command",
                    "operation_ref": ack["operation_ref"],
                    "command_id": env["command_id"],
                    "caused_by": list(env.get("caused_by", [])),
                    "recorded_at": recorded_at,
                    "payload": payload,
                })
            st.bind(self.scope, env["command_id"], generation, env["command_digest"],
                    V.canonical(ack), V.canonical(outcome))
            if self._take_fault("commit_unavailable", method):
                # Injected store fault (decision 007): the owner transaction
                # rolls back, so nothing is bound (CORE 12 unavailable).
                log("injected fault: commit_unavailable for", method)
                raise ProtocolError("unavailable")
            try:
                st.commit()
            except sqlite3.Error as exc:
                log("commit failed:", repr(exc))
                raise ProtocolError("unavailable") from None
            if self._take_fault("response_internal_error", method):
                # Committed; the caller learns nothing about the outcome.
                log("injected fault: response_internal_error for", method)
                raise ProtocolError("internal_error")
            return {"acknowledgment": ack, "outcome": outcome, "replay": False}
        except BaseException:
            st.rollback()
            raise

    def _take_fault(self, kind: str, method: str) -> bool:
        remaining = self.faults[kind].get(method, 0)
        if remaining <= 0:
            return False
        self.faults[kind][method] = remaining - 1
        return True

    # ------------------------------------------------------ CORE 15.5 (step 6)
    def _effect_target(self, effect_id: str) -> dict:
        row = self.store.get_json(XE.EFFECT_KIND, effect_id)
        return row[1]["descriptor"]["target"] if row is not None else ALL_EXECUTIONS

    def authorize(self, method: str, env: dict) -> Auth:
        payload = env["payload"]
        # EXECUTION 11 "base rights" and "Rights": each command's right on the
        # subject it acts on; execution.read for reads and effects.
        if method in ("execution.submit", "execution.cancel", "execution.steer", "execution.respond_action",
                      "execution.workspace.checkpoint", "execution.controller.claim"):
            return self.authorize_under(env, [(method, env["subject"])])
        if method in ("execution.inspect", "execution.output.read"):
            return self.authorize_under(env, [("execution.read", {"kind": XE.EXECUTION_KIND, "id": payload["execution"]})])
        if method == "execution.reconcile":
            # The executions a reconcile finds are unknown before the lookup;
            # the result shows only those the grant covers (G-RECONCILE-AUTH).
            return self.authorize_under(env, [("execution.read", None)])
        if method == "execution.discovery.list":
            return self.authorize_under(env, [("execution.discovery.list",
                                               {"kind": "execution.discovery", "id": "installations"})])
        if method == "core.effects.get":
            # CORE 19.2: read authority on the effect's target; a missing
            # effect is refused exactly like one the grant does not cover.
            return self.authorize_under(env, [("execution.read", self._effect_target(payload["effect"]))])
        if method == "core.effects.abort_obligation":
            return self.authorize_under(env, [("core.effects.abort_obligation",
                                               self._effect_target(env["subject"]["id"]))])
        if method.startswith("evidence."):
            return self.evidence.authorize(method, env)
        if method.startswith("context."):
            return self.context.authorize(method, env)
        if method == "core-test.subject.put":
            needs = [("core-test.write", env["subject"])]
            needs += [("core-test.read", p["subject"]) for p in env["preconditions"] if p["subject"] != env["subject"]]
            return self.authorize_under(env, needs)
        if method == "core-test.authority.claim":
            return self.authorize_under(env, [("core-test.claim", E.AUTHORITY_SUBJECT)])
        if method in ("core-test.subject.get", "core-test.subject.applied_count"):
            return self.authorize_under(env, [("core-test.read", payload["subject"])])
        if method in ("core.events.read", "core.events.subscribe"):
            return self.authorize_under(env, [("core.events.read", None)])
        if method == "core.grant.issue":
            return self.authorize_issue(env)
        if method == "core.grant.revoke":
            return self.authorize_revoke(env)
        # CORE 15.5 "Which operations are protected": core.describe,
        # core.negotiate, core.authenticate, core.capabilities and
        # core.events.unsubscribe are not; core.grant.get has its own
        # visibility rule. A grant field there is validated, not evaluated.
        return Auth(self, None)

    def make_auth(self, grant: dict | None) -> Auth:
        return Auth(self, grant)

    def usable_grant(self, env: dict) -> dict:
        """The grant a request names, checked in CORE 15.5 order up to its
        rights: held by this principal, active, unexpired, epoch current."""
        row = self.store.grant(env["grant"])
        if row is None or row[1]["holder"] != self.principal:
            raise denied("grant_not_found")
        record = row[1]
        problem = G.usable_problem(record, self.now(), self.epoch_of)
        if problem:
            raise denied(problem)
        return record

    def authorize_under(self, env: dict, needs: list) -> Auth:
        grant_id = env.get("grant")
        if grant_id is None:
            if self.is_authority:
                return Auth(self, None)
            raise denied("grant_required")
        record = self.usable_grant(env)
        # All rights first, then all resources (E-DENIAL-ORDER).
        if any(right not in record["rights"] for right, _ in needs):
            raise denied("right_missing")
        if any(subj is not None and not G.grant_covers(record, subj) for _, subj in needs):
            raise denied("out_of_scope")
        return Auth(self, record)

    def authorize_issue(self, env: dict) -> Auth:
        terms = env["payload"]
        # CORE 15.3 lists these as invalid_envelope; CORE 10 step 6 allows
        # invalid_envelope. Checking them here, after the deduplication lookup,
        # keeps a retransmission replayable after the clock passes expires_at
        # or provider_id changes (E-ISSUE-VALIDATION-STEP).
        if terms["audience"] != self.provider_id:
            raise ProtocolError("invalid_envelope", {"path": "/payload/audience",
                                                     "reason": "audience must be this provider's ID"})
        if "expires_at" in terms and terms["expires_at"] <= self.now():
            raise ProtocolError("invalid_envelope", {"path": "/payload/expires_at",
                                                     "reason": "expires_at must be after the provider's current time"})
        # CORE 15.3: a binding to an authority scope the provider does not
        # track, "checked with audience and expires_at at step 6". A later
        # epoch of a tracked scope is accepted (F-ISSUE-CHECK-ORDER).
        if "authority_binding" in terms and terms["authority_binding"]["scope"] not in self.tracked_scopes():
            raise ProtocolError("invalid_envelope", {"path": "/payload/authority_binding/scope",
                                                     "reason": "not an authority scope this provider tracks"})
        if "parent" not in terms:
            if not self.is_authority:
                raise denied("not_authority")
            return Auth(self, None)
        row = self.store.grant(terms["parent"])
        # "only by the parent's holder" -- an authority that is not the holder
        # is refused too (E-ISSUE-PARENT-HOLDER).
        if row is None or row[1]["holder"] != self.principal:
            raise denied("grant_not_found")
        parent = row[1]
        problem = G.usable_problem(parent, self.now(), self.epoch_of)
        if problem:
            raise denied(problem)
        if G.delegation_exceeded(terms, parent):
            raise denied("delegation_exceeded")
        return Auth(self, None)

    def authorize_revoke(self, env: dict) -> Auth:
        row = self.store.grant(env["subject"]["id"])
        issuer = row[1]["issuer"] if row else None
        if not self.is_authority and issuer != self.principal:
            # CORE 15.3: "whether or not the grant exists".
            raise denied("not_authority")
        if row is not None and row[1]["state"] == "revoked":
            # CORE 15.3: re-revocation, "decided after that check".
            raise denied("revoked")
        return Auth(self, None)

    # ------------------------------------------------------ CORE 7, 8, 17
    def check_capabilities(self, method: str) -> None:
        _, predicates = self.store.capabilities()
        by_name = {p["name"]: p for p in predicates}
        for name in OPERATION_CAPABILITIES.get(method, ()):
            status = by_name[name]["status"] if name in by_name else "unknown"
            if status != "supported":
                raise ProtocolError("capability_unavailable", {"capability": name, "status": status})

    def check_preconditions(self, env: dict, auth: Auth) -> None:
        failed = []
        for entry in env["preconditions"]:
            subj = entry["subject"]
            current = self.store.revision(subj["kind"], subj["id"])
            if current != entry["revision"]:
                item = {"subject": subj, "expected": entry["revision"]}
                if auth.may_read(subj):
                    item["current"] = current
                failed.append(item)
        if failed:
            raise ProtocolError("precondition_failed", {"failed": failed})

    def op_claim(self, env: dict, auth: Auth):
        subj = E.AUTHORITY_SUBJECT
        self.check_preconditions(env, auth)
        epoch = self.store.revision(subj["kind"], subj["id"]) + 1
        self.store.write_subject(subj["kind"], subj["id"], epoch, None)
        return epoch, {"epoch": epoch}, [("core-test.authority.claimed", dict(subj), epoch, {"epoch": epoch})]

    def op_put(self, env: dict, auth: Auth):
        current_epoch = self.epoch_of(AUTHORITY_SCOPE)
        claimed = env["authority_epoch"]
        if claimed < current_epoch:
            details = {"current_epoch": current_epoch} if auth.may_read(E.AUTHORITY_SUBJECT) else {}
            raise ProtocolError("stale_authority_epoch", details)
        if claimed > current_epoch:
            raise ProtocolError("unknown_authority_epoch")
        self.check_preconditions(env, auth)
        subj = env["subject"]
        revision = self.store.revision(subj["kind"], subj["id"]) + 1
        value = env["payload"]["value"]
        # labels exist only for canonical-ordering fixtures and are not stored.
        self.store.write_subject(subj["kind"], subj["id"], revision, value)
        return revision, {"value": value}, [("core-test.subject.changed", subj, revision, {"value": value})]

    def op_grant_issue(self, env: dict, auth: Auth):
        self.check_preconditions(env, auth)
        terms = env["payload"]
        record = {"id": env["subject"]["id"], "issuer": self.principal}
        record.update(terms)
        record["state"] = "active"
        self.store.write_grant(1, record)
        return 1, {"grant": record}, [("core.grant.issued", env["subject"], 1, {"grant": record})]

    def op_grant_revoke(self, env: dict, auth: Auth):
        self.check_preconditions(env, auth)
        root = env["subject"]["id"]
        grants = self.store.grants_in_issue_order()
        children: dict[str, list] = {}
        by_id = {}
        for gid, rev, record in grants:
            by_id[gid] = (rev, record)
            if "parent" in record:
                children.setdefault(record["parent"], []).append(gid)
        # The grant itself, then its descendants breadth-first in issue order
        # (CORE 16.3: stable order, primary subject first).
        order, queue, seen = [], [root], set()
        while queue:
            gid = queue.pop(0)
            if gid in seen or gid not in by_id:
                continue
            seen.add(gid)
            order.append(gid)
            queue.extend(children.get(gid, []))
        revoked, changes, primary_revision = [], [], None
        for gid in order:
            rev, record = by_id[gid]
            if record["state"] == "revoked":
                continue
            record = dict(record, state="revoked")
            rev += 1
            self.store.write_grant(rev, record)
            revoked.append(gid)
            changes.append(("core.grant.revoked", {"kind": E.GRANT_KIND, "id": gid}, rev, {"state": "revoked"}))
            if gid == root:
                primary_revision = rev
        return primary_revision, {"revoked": revoked}, changes

    # ----------------------------------------------------------- queries
    def op_describe(self, env: dict, auth: Auth) -> dict:
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

    def op_negotiate(self, env: dict, auth: Auth) -> dict:
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
        feature_refused: set[str] = set()
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
                feature_refused.add(name)
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
        # EXECUTION 1: execution/1 needs Core features core.events,
        # core.capabilities and core.effects selected in this session; one
        # item per missing feature. Its own missing required features, if
        # any, are listed too (G-NEG-EXEC-ITEMS).
        for profile_name, required_core in REQUIRED_CORE_FEATURES.items():
            p = requested.get(profile_name)
            if p is None or not (profile_name in selected or profile_name in feature_refused):
                continue
            core_features = selected.get("core", {"features": []})["features"]
            absent = [f for f in required_core if f not in core_features]
            if absent:
                selected.pop(profile_name, None)
                sink = refusals if p["required"] else unselected
                sink.extend({"profile": profile_name, "feature": f, "reason": "dependency_not_selected"} for f in absent)
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

    def op_authenticate(self, env: dict, auth: Auth) -> dict:
        # CORE 18.2 "Repeats": every stdio session already has the principal
        # its launch configuration assigned (STREAM 5). The credential is never
        # examined, logged or echoed (CORE 18.1 "Secrecy").
        raise ProtocolError("already_authenticated")

    def op_get(self, env: dict, auth: Auth) -> dict:
        subj = env["payload"]["subject"]
        row = self.store.subject(subj["kind"], subj["id"])
        if row is None:
            raise ProtocolError("not_found")
        return {"subject": subj, "revision": row[0], "value": row[1]}

    def op_applied_count(self, env: dict, auth: Auth) -> dict:
        subj = env["payload"]["subject"]
        row = self.store.subject(subj["kind"], subj["id"])
        return {"subject": subj, "applied_count": row[2] if row else 0}

    def op_grant_get(self, env: dict, auth: Auth) -> dict:
        row = self.store.grant(env["payload"]["grant"])
        if row is None or not (self.is_authority or self.principal in (row[1]["holder"], row[1]["issuer"])):
            raise ProtocolError("not_found")
        return {"grant": row[1], "revision": row[0]}

    def stream_ref(self) -> dict:
        return {"id": self.store.stream_id(), "epoch": self.store.head()[0]}

    def start_position(self, payload: dict) -> tuple[int, int]:
        try:
            return EV.start_position(self.store, payload)
        except EV.CursorError as exc:
            raise ProtocolError("invalid_cursor", {"reason": exc.reason}) from None

    def response_fits(self, result: dict) -> bool:
        """Whether the response frame carrying ``result`` fits the caller's
        receive limit (STREAM 1.5, CORE 4.2)."""
        frame = {"jsonrpc": "2.0", "id": self.session.request_id, "result": result}
        return len(V.canonical(frame)) <= self.session.send_limit

    def fit_bytes(self, build, data: bytes) -> dict:
        """The response for the longest prefix of ``data`` that fits the
        caller's receive limit, keeping at least one byte whenever one fits
        (EVIDENCE 4 "Chunk sizing", CONTEXT 6)."""
        result = build(data)
        if self.response_fits(result) or not data:
            return result
        lo, hi, best = 1, len(data) - 1, build(b"")
        while lo <= hi:
            mid = (lo + hi) // 2
            candidate = build(data[:mid])
            if self.response_fits(candidate):
                best, lo = candidate, mid + 1
            else:
                hi = mid - 1
        return best

    def fit_items(self, build, limit: int):
        """CORE 16.4 "Size" and 16.5 "Delivery": the largest item count up to
        ``limit`` whose frame fits. ``build(n)`` reads at most ``n`` items and
        returns ``(frame_fits, items, value)``. Returns the value for the
        largest fitting count, or None when not even one item fits."""
        fits, items, value = build(limit)
        if fits or not items:
            return value
        lo, hi, best = 1, len(items) - 1, None
        while lo <= hi:
            mid = (lo + hi) // 2
            fits, _, candidate = build(mid)
            if fits:
                best, lo = candidate, mid + 1
            else:
                hi = mid - 1
        return best

    def op_events_read(self, env: dict, auth: Auth) -> dict:
        payload = env["payload"]
        start = self.start_position(payload)

        def build(n):
            items, pos, hidden = EV.read_items(self.store, start, n, auth.sees, payload.get("kinds"))
            result = {
                "stream": self.stream_ref(),
                "items": items,
                "next_cursor": EV.encode_cursor(self.store, pos),
                # CORE 16.4: true exactly when something in the covered range
                # was hidden by authorization or kinds.
                "filtered": hidden,
            }
            return self.response_fits(result), items, result

        result = self.fit_items(build, payload["limit"])
        if result is None:
            # CORE 16.4: "If not even one item fits, the read is internal_error."
            raise ProtocolError("internal_error", {}, "the next item does not fit the caller's receive limit")
        return result

    def op_events_subscribe(self, env: dict, auth: Auth) -> dict:
        payload = env["payload"]
        pos = self.start_position(payload)
        s = self.session
        sid = f"sub-{s.next_subscription}"
        s.next_subscription += 1
        s.subscriptions[sid] = {"pos": pos, "kinds": payload.get("kinds"), "env": env}
        return {"subscription": sid, "stream": self.stream_ref()}

    def op_events_unsubscribe(self, env: dict, auth: Auth) -> dict:
        if self.session.subscriptions.pop(env["payload"]["subscription"], None) is None:
            raise ProtocolError("not_found")
        return {}

    def op_capabilities(self, env: dict, auth: Auth) -> dict:
        revision, predicates = self.store.capabilities()
        return {"revision": revision, "predicates": predicates}

    # ------------------------------------------------------ CORE 16.5
    def end_subscription(self, sid: str, reason: str) -> None:
        """One final notification with no items and ``ended``, then nothing
        more (CORE 16.5 "Lifetime"). ``next_cursor`` is where delivery stopped:
        after the last item delivered."""
        sub = self.session.subscriptions.pop(sid)
        log("ending subscription", sid, "reason", reason)
        self.write_frame({"jsonrpc": "2.0", "method": "core.events.notify",
                          "params": {"subscription": sid, "items": [],
                                     "next_cursor": EV.encode_cursor(self.store, sub["pos"]),
                                     "ended": {"reason": reason}}})

    def deliver_notifications(self) -> None:
        for sid in list(self.session.subscriptions):
            sub = self.session.subscriptions.get(sid)
            if sub is None:
                continue
            try:
                # CORE 16.5 "Authorization": checked again before each
                # delivery. This runs after every request, whether or not an
                # item is pending, so the end is reported promptly
                # (F-SUB-END-TIMING).
                auth = self.authorize("core.events.subscribe", sub["env"])
            except ProtocolError:
                self.end_subscription(sid, "authorization_lost")
                continue
            while True:
                start = sub["pos"]

                def build(n, start=start):
                    items, pos, _ = EV.read_items(self.store, start, n, auth.sees, sub["kinds"])
                    frame = {"jsonrpc": "2.0", "method": "core.events.notify",
                             "params": {"subscription": sid, "items": items,
                                        "next_cursor": EV.encode_cursor(self.store, pos)}}
                    return len(V.canonical(frame)) <= self.session.send_limit, items, (frame, pos)

                fitted = self.fit_items(build, NOTIFY_CHUNK)
                if fitted is None:
                    # CORE 16.5: an item that cannot fit alone ends the
                    # subscription; it is never skipped.
                    self.end_subscription(sid, "item_too_large")
                    break
                frame, pos = fitted
                sub["pos"] = pos  # also passes trailing hidden events
                if not frame["params"]["items"]:
                    break  # the schema requires at least one item unless ended
                self.write_frame(frame)


def valid_request_id(rid) -> bool:
    if isinstance(rid, bool):
        return False
    if isinstance(rid, int):
        return -V.MAX_SAFE <= rid <= V.MAX_SAFE
    if isinstance(rid, str):
        return 1 <= len(rid) <= 128  # code points (STREAM §3 as resolved in M2-DIVERGENCES D-STREAM-ID)
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
                with provider.lock:
                    fatal("frame_too_large", "frame exceeds the frame limit")
            provider.handle_frame(frame)
            continue
        scanned = len(buf)
        limit = provider.session.recv_limit
        if len(buf) > limit:
            with provider.lock:
                fatal("frame_too_large", "frame exceeds the frame limit")
        # STREAM 1.5: never buffer more than limit + 1 bytes of an unfinished frame.
        chunk = os.read(0, max(1, min(65536, limit + 1 - len(buf))))
        if not chunk:
            # STREAM 1.6: bytes after the last terminator are discarded.
            if buf:
                log(f"discarded {len(buf)} unterminated bytes at end of input")
            return
        buf += chunk


def capability_predicates(config: dict, adapter_predicates: list) -> list:
    """The snapshot observed at this start (CORE 17.1). Without a configured
    status, core-test.writes is supported on the evidence that this start
    committed a write transaction to the store. No observed_at: an instant
    would change the evidence, and so the revision, on every restart
    (E-CAP-EVIDENCE)."""
    status = config["capabilities"].get("core-test.writes")
    if status is None:
        writes = [{"name": "core-test.writes", "status": "supported",
                   "evidence": {"source": "store write transaction committed at provider start"}}]
    else:
        writes = [{"name": "core-test.writes", "status": status,
                   "evidence": {"source": "conformance launch configuration"}}]
    # EXECUTION 10: adapter predicates reuse CORE 17. Only predicates the
    # scripted adapter reports are listed; an unlisted one is unknown.
    return writes + adapter_predicates


def idle_loop(provider: "Provider") -> None:
    import time
    while True:
        time.sleep(IDLE_RECHECK_SECONDS)
        try:
            provider.idle_recheck()
        except Exception as exc:  # never let the re-check thread die silently
            log("idle re-check failed:", repr(exc))
            with provider.lock:
                provider.store.rollback()


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
        # Decision 007 section 2: a missing or malformed clock file refuses
        # the start with a nonzero exit.
        clock = C.Clock(fixed=config["clock"], path=config["clock_file"])
    except (OSError, ConfigError, C.ClockError) as exc:
        log("configuration error:", exc)
        return 2
    store = Store(args.data_dir)
    provider = Provider(config, store, clock)
    # Start-time environment, in this order (E-START-ORDER): deduplication
    # window, stream epoch, event retention, capability snapshot.
    store.apply_generation_config(config["advance"], config["retain"])
    store.apply_event_config(config["new_epoch"], config["unvouched_last"], config["retain_last"])
    predicates = capability_predicates(config, provider.executor.adapter_predicates())

    def capability_event(revision: int) -> dict:
        # CORE 17.3: provider-origin, so no operation_ref or command_id.
        return {
            "type": "core.capabilities.changed",
            "subject": {"kind": "core.capabilities", "id": config["provider_id"]},
            "revision": revision,
            "origin": "provider",
            "caused_by": [],
            "recorded_at": provider.now(),
            "payload": {"predicates": predicates},
        }

    store.apply_capabilities(predicates, capability_event)
    # EXECUTION 7.1: restart recovery, before any request. A new stream epoch
    # at this start means the journal's continuity cannot be vouched for.
    provider.executor.recover(journal_intact=not config["new_epoch"])
    threading.Thread(target=idle_loop, args=(provider,), daemon=True).start()
    serve(provider)
    # End of input: take the lock so no idle re-check is mid-transaction when
    # the process exits.
    provider.lock.acquire()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
