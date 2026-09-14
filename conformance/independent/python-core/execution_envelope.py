"""Closed-object validation for the M3 operations: Core effects (CORE 19) and
execution/1 (EXECUTION 4, 11-14).

Hand-written from schemas/core/1/core.effects.*.json and
schemas/execution/1/*.json. Validators take the set of features the session
selected: a submit member that belongs to an unselected feature is an unknown
member, ``invalid_envelope`` at that member's path (EXECUTION 11
"Negotiation"), and the envelope's ``grant`` field needs ``core.grants``.
"""

from __future__ import annotations

import envelope as E
from envelope import Invalid, ptr

EXECUTION_KIND = "execution.execution"
CONTROLLER_KIND = "execution.controller"
EFFECT_KIND = "core.effect"
MAX_SAFE = 2**53 - 1

ENFORCEMENT = ("enforced", "mediated", "cooperative")
TIMEOUTS = ("queue", "delivery", "execution_deadline", "inactivity", "reconciliation")
OBLIGATIONS = ("advisory", "required_before_start", "required_before_transition")

# EXECUTION 11 "Negotiation": submit members owned by optional features.
FEATURE_MEMBERS = {
    "workspace": "execution.workspaces",
    "budget": "execution.usage",
    "context_bindings": "execution.context",
    "continuation": "execution.continuation",
}
BASE_SUBMIT_MEMBERS = ("brief", "adapter", "restrictions", "timeouts", "predecessor", "correlation")


def _command(env, method, features) -> dict:
    return E.command_envelope(env, method, "core.grants" in features)


def _query(env, method, features) -> dict:
    return E.query_envelope(env, method, "core.grants" in features)


def _only_primary_precondition(env: dict) -> dict:
    # Schemas: exactly one precondition. EXECUTION 4 and 11.3 name it on the
    # command's own subject; a precondition on anything else has no meaning
    # here (G-PRIMARY-PRECONDITION).
    if len(env["preconditions"]) != 1:
        raise Invalid("/preconditions", "exactly one precondition is required")
    if env["preconditions"][0]["subject"] != env["subject"]:
        raise Invalid("/preconditions/0/subject", "the precondition must name the command's subject")
    return env["preconditions"][0]


def _content_ref(v, path: str) -> dict:
    E.closed(v, path, ("digest", "media_type"))
    E.digest_string(v["digest"], ptr(path, "digest"))
    E.string(v["media_type"], ptr(path, "media_type"), 1, 128)
    return v


def _string_array(v, path: str, max_items: int, max_len: int) -> list:
    E.array(v, path, max_items=max_items)
    for idx, item in enumerate(v):
        E.string(item, ptr(path, idx), 1, max_len)
    return v


def _dotted_array(v, path: str, max_items: int, unique: bool = False) -> list:
    E.array(v, path, max_items=max_items, unique=unique)
    for idx, item in enumerate(v):
        E.dotted_name(item, ptr(path, idx))
    return v


# ---------------------------------------------------------- CORE 19 effects

def effects_get_params(env, method, features) -> None:
    _query(env, method, features)
    payload = E.closed(env["payload"], "/payload", ("effect",))
    E.identifier(payload["effect"], "/payload/effect")


def effects_abort_params(env, method, features) -> None:
    _command(env, method, features)
    E.subject(env["subject"], "/subject", EFFECT_KIND)
    payload = E.closed(env["payload"], "/payload", ("obligation",))
    E.identifier(payload["obligation"], "/payload/obligation")
    _only_primary_precondition(env)


# ------------------------------------------------------- EXECUTION 4 base

def submit_params(env, method, features) -> None:
    _command(env, method, features)
    E.subject(env["subject"], "/subject", EXECUTION_KIND)
    payload = env["payload"]
    allowed = list(BASE_SUBMIT_MEMBERS) + [m for m, f in FEATURE_MEMBERS.items() if f in features]
    E.closed(payload, "/payload", ("brief",), tuple(m for m in allowed if m != "brief"))
    _content_ref(payload["brief"], "/payload/brief")
    if "adapter" in payload:
        adapter = E.closed(payload["adapter"], "/payload/adapter", ("requires",))
        _dotted_array(adapter["requires"], "/payload/adapter/requires", 64, unique=True)
    if "restrictions" in payload:
        E.array(payload["restrictions"], "/payload/restrictions", max_items=64)
        for idx, r in enumerate(payload["restrictions"]):
            p = ptr("/payload/restrictions", idx)
            E.closed(r, p, ("scope", "enforcement"))
            E.string(r["scope"], ptr(p, "scope"), 1, 256)
            if r["enforcement"] not in ENFORCEMENT:
                raise Invalid(ptr(p, "enforcement"), "must be enforced, mediated or cooperative")
    if "timeouts" in payload:
        t = E.closed(payload["timeouts"], "/payload/timeouts", (), TIMEOUTS)
        for name, value in t.items():
            E.integer(value, ptr("/payload/timeouts", name), 1, 31536000)
    if "predecessor" in payload:
        E.identifier(payload["predecessor"], "/payload/predecessor")
    if "correlation" in payload:
        if not isinstance(payload["correlation"], dict) or len(payload["correlation"]) > 64:
            raise Invalid("/payload/correlation", "must be an object with at most 64 members")
    if "workspace" in payload:
        workspace_request(payload["workspace"], "/payload/workspace")
    if "budget" in payload:
        b = E.closed(payload["budget"], "/payload/budget", ("pool", "ceiling"), ("amount",))
        E.identifier(b["pool"], "/payload/budget/pool")
        if b["ceiling"] not in ("hard", "soft"):
            raise Invalid("/payload/budget/ceiling", "must be hard or soft")
        if "amount" in b:
            E.integer(b["amount"], "/payload/budget/amount", 1)
    if "context_bindings" in payload:
        E.array(payload["context_bindings"], "/payload/context_bindings", max_items=64)
        for idx, binding in enumerate(payload["context_bindings"]):
            context_binding_request(binding, ptr("/payload/context_bindings", idx))
    if "continuation" in payload:
        c = E.closed(payload["continuation"], "/payload/continuation", ("of", "mode"))
        E.identifier(c["of"], "/payload/continuation/of")
        if c["mode"] not in ("resume", "fork"):
            raise Invalid("/payload/continuation/mode", "must be resume or fork")
    # EXECUTION 4: "Creates the execution (precondition revision 0)"
    # (G-SUBMIT-PRECONDITION).
    if _only_primary_precondition(env)["revision"] != 0:
        raise Invalid("/preconditions/0/revision", "execution.submit creates the execution: revision must be 0")


def workspace_request(v, path: str) -> None:
    E.closed(v, path, ("repository", "base", "cleanup"), ("permitted_paths", "permitted_effects"))
    E.string(v["repository"], ptr(path, "repository"), 1, 512)
    E.string(v["base"], ptr(path, "base"), 1, 128)
    if "permitted_paths" in v:
        _string_array(v["permitted_paths"], ptr(path, "permitted_paths"), 64, 512)
    if "permitted_effects" in v:
        _dotted_array(v["permitted_effects"], ptr(path, "permitted_effects"), 64)
    if v["cleanup"] not in ("retain", "remove"):
        raise Invalid(ptr(path, "cleanup"), "must be retain or remove")


def context_binding_request(v, path: str) -> None:
    E.closed(v, path, ("binding_id", "packet", "obligation", "selected_by"), ("transition",))
    E.identifier(v["binding_id"], ptr(path, "binding_id"))
    packet = E.closed(v["packet"], ptr(path, "packet"), ("ref", "digest"))
    E.identifier(packet["ref"], ptr(ptr(path, "packet"), "ref"))
    E.digest_string(packet["digest"], ptr(ptr(path, "packet"), "digest"))
    if v["obligation"] not in OBLIGATIONS:
        raise Invalid(ptr(path, "obligation"), "unknown obligation")
    if "transition" in v:
        E.string(v["transition"], ptr(path, "transition"), 1, 128)
    elif v["obligation"] == "required_before_transition":
        raise Invalid(ptr(path, "transition"), "required for required_before_transition")
    E.identifier(v["selected_by"], ptr(path, "selected_by"))


def inspect_params(env, method, features) -> None:
    _query(env, method, features)
    payload = E.closed(env["payload"], "/payload", ("execution",))
    E.identifier(payload["execution"], "/payload/execution")


def cancel_params(env, method, features) -> None:
    _command(env, method, features)
    E.subject(env["subject"], "/subject", EXECUTION_KIND)
    payload = E.closed(env["payload"], "/payload", (), ("reason",))
    if "reason" in payload:
        E.string(payload["reason"], "/payload/reason", 1, 256)
    _only_primary_precondition(env)


def reconcile_params(env, method, features) -> None:
    _query(env, method, features)
    payload = E.closed(env["payload"], "/payload", (), ("command_id", "delivery_id"))
    if len(payload) != 1:
        raise Invalid("/payload", "exactly one of command_id and delivery_id is required")
    for name, value in payload.items():
        E.identifier(value, ptr("/payload", name))


# ---------------------------------------------------- EXECUTION 11 features

def steer_params(env, method, features) -> None:
    _command(env, method, features)
    E.subject(env["subject"], "/subject", EXECUTION_KIND)
    payload = E.closed(env["payload"], "/payload", ("message",))
    _content_ref(payload["message"], "/payload/message")
    _only_primary_precondition(env)


def respond_action_params(env, method, features) -> None:
    _command(env, method, features)
    E.subject(env["subject"], "/subject", EXECUTION_KIND)
    payload = E.closed(env["payload"], "/payload", ("action_id", "response"))
    E.identifier(payload["action_id"], "/payload/action_id")
    _content_ref(payload["response"], "/payload/response")
    _only_primary_precondition(env)


def controller_claim_params(env, method, features) -> None:
    _command(env, method, features)
    E.subject(env["subject"], "/subject", CONTROLLER_KIND)
    E.closed(env["payload"], "/payload", ())
    _only_primary_precondition(env)


def checkpoint_params(env, method, features) -> None:
    _command(env, method, features)
    E.subject(env["subject"], "/subject", EXECUTION_KIND)
    E.closed(env["payload"], "/payload", ())
    _only_primary_precondition(env)


def discovery_params(env, method, features) -> None:
    _query(env, method, features)
    E.closed(env["payload"], "/payload", ())


def output_read_params(env, method, features) -> None:
    _query(env, method, features)
    payload = E.closed(env["payload"], "/payload", ("execution",), ("offset", "max_bytes"))
    E.identifier(payload["execution"], "/payload/execution")
    if "offset" in payload:
        E.integer(payload["offset"], "/payload/offset", 0, MAX_SAFE)
    if "max_bytes" in payload:
        E.integer(payload["max_bytes"], "/payload/max_bytes", 1, 1048576)
