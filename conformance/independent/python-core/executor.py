"""Core effects (CORE 19) and the execution/1 profile over a scripted executor.

Written from EXECUTION.md, CORE.md section 19, decision 007, the conformance
README's launch configuration table and schemas/execution/1 only.

Durable records live in the provider's SQLite store as subjects:

- ``execution.execution``: one record per execution. Its revision rises by one
  with every execution event (G-EXEC-REVISION); executor bookkeeping (script
  position, dispatch marker, counters) is stored in the same record without
  raising the revision.
- ``core.effect``: one record per effect: descriptor, observations, attempts
  and obligations. Its revision rises with every visible change.
- ``execution.controller``: the controller lease of the one scripted host; its
  revision is the authority epoch of scope ``execution.controller:<host id>``.

The scripted harness (EXECUTION 15, owner decision Q4) is the launch
configuration's ``executor`` object. Its steps advance in "ticks": before and
after every request and on an idle re-check, never inside a command's owner
transaction. A step that stands for harness interaction first dispatches the
prompt: a write-ahead dispatch marker is committed, then the attempt is made
and recorded in a later transaction (EXECUTION 7.1).
"""

from __future__ import annotations

import base64
import hashlib
import os
import sys

import clock as C
import grants as G
import valuedomain as V
from errors import ProtocolError
from execution_envelope import CONTROLLER_KIND, EFFECT_KIND, EXECUTION_KIND

LEVEL = {"cooperative": 1, "mediated": 2, "enforced": 3}
PROOF_CLASSES = ("provider_ack_id", "echo", "transport_only", "bytes_written")
RUNTIMES = ("preparing", "active", "requires_action", "quiescent", "exited", "unknown")
TERMINAL = ("acknowledged", "delivered", "not_delivered", "failed_before_delivery")
# EXECUTION 3.1 "Effect status".
STATUS_OF = {"pending": "pending", "acknowledged": "succeeded", "delivered": "succeeded",
             "not_delivered": "failed", "failed_before_delivery": "failed", "ambiguous": "unknown"}
# CORE 19.3: how many tries a retryable effect gets before its outcome is
# declared unknown (G-MAX-ATTEMPTS).
MAX_ATTEMPTS = 3
SOURCE = "scripted-harness"
DEFAULT_HOST = "scripted-host"
DEFAULT_SPOOL = 65536
DEFAULT_READ_BYTES = 65536
CRASH_EXIT_STATUS = 1  # G-CRASH-EXIT

# Script steps that do not stand for harness interaction, so they do not
# dispatch the prompt first (G-DISPATCH-POINT).
CONTROL_STEPS = ("wait_until", "wait_for", "transport_errors", "on_cancel", "crash", "stale_dispatch",
                 "probe_status", "reconcile_finds")
EXIT_OUTCOMES = ("cancelled", "refused", "not_supported", "unknown")
STEER_ALTERNATIVE = "live steering is not supported by this harness; cancel and submit a new execution with the revised brief"


def log(*parts) -> None:
    print("[independent-python-core] executor:", *parts, file=sys.stderr, flush=True)


def sha256_of(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


# ------------------------------------------------------------ configuration

class ExecutorConfigError(Exception):
    pass


def _fail(msg: str):
    raise ExecutorConfigError(msg)


def _obj(v, what: str, required=(), optional=()) -> dict:
    if not isinstance(v, dict):
        _fail(f"{what} must be an object")
    unknown = set(v) - set(required) - set(optional)
    if unknown:
        _fail(f"{what}: unknown members {sorted(unknown)}")
    missing = [k for k in required if k not in v]
    if missing:
        _fail(f"{what}: missing members {missing}")
    return v


def _int(v, what: str, minimum: int = 0) -> int:
    if not isinstance(v, int) or isinstance(v, bool) or v < minimum:
        _fail(f"{what} must be an integer >= {minimum}")
    return v


def _str(v, what: str, min_len: int = 0) -> str:
    if not isinstance(v, str) or len(v) < min_len:
        _fail(f"{what} must be a string")
    return v


def _enum(v, values, what: str):
    if v not in values:
        _fail(f"{what} must be one of {list(values)}")
    return v


def _str_list(v, what: str) -> list:
    if not isinstance(v, list):
        _fail(f"{what} must be an array")
    for item in v:
        _str(item, what)
    return v


def validate_step(step, what: str) -> None:
    if not isinstance(step, dict) or len(step) != 1:
        _fail(f"{what} must be an object with exactly one member")
    (key, val), = step.items()
    w = f"{what}.{key}"
    if key in ("deliver", "steer_deliver"):
        _enum(val, PROOF_CLASSES, w)
    elif key == "crash":
        _enum(val, ("before_dispatch", "after_write"), w)
    elif key == "reconcile_finds":
        _enum(val, ("delivered", "not_delivered", "unknown"), w)
    elif key == "runtime":
        _enum(val, RUNTIMES, w)
    elif key in ("host_restart", "stall", "probe_status"):
        if val is not True:
            _fail(f"{w} must be true")
    elif key == "complete":
        _obj(val, w, ("completion_id", "content"), ("generation",))
        _str(val["completion_id"], w)
        _str(val["content"], w)
        if "generation" in val:
            _int(val["generation"], w, 1)
    elif key == "exit":
        # Schema: an object. The inspect schema can carry only {code} or
        # {signal}, so nothing else can be reported (G-EXIT-STEP).
        if isinstance(val, dict) and set(val) == {"code"} and isinstance(val["code"], int) \
                and not isinstance(val["code"], bool) and -2**31 <= val["code"] < 2**31:
            return
        if isinstance(val, dict) and set(val) == {"signal"} and isinstance(val["signal"], str) \
                and 1 <= len(val["signal"]) <= 32:
            return
        _fail(f"{w} must be {{code: integer}} or {{signal: string}}")
    elif key == "on_cancel":
        _enum(val, EXIT_OUTCOMES, w)
    elif key == "wait_until":
        if not C.valid_instant(val):
            _fail(f"{w} must be an instant YYYY-MM-DDTHH:MM:SSZ")
    elif key == "wait_for":
        _enum(val, ("cancel", "steer", "action"), w)
    elif key == "stale_dispatch":
        _obj(val, w, ("generation",))
        _int(val["generation"], w, 1)
    elif key == "steer_behavior":
        if val != "observed":
            _fail(f"{w} must be observed")
    elif key == "request_action":
        _obj(val, w, ("action_id", "owner"))
        _str(val["action_id"], w, 1)
        _str(val["owner"], w, 1)
    elif key == "workspace":
        _obj(val, w, ("probes",), ("head", "dirty_paths", "untracked_paths"))
        if not isinstance(val["probes"], list) or any(p not in ("tracked", "dirty", "untracked") for p in val["probes"]):
            _fail(f"{w}.probes must list tracked, dirty or untracked")
        if "head" in val:
            _str(val["head"], w, 1)
        for member in ("dirty_paths", "untracked_paths"):
            if member in val:
                _str_list(val[member], f"{w}.{member}")
    elif key in ("agent_reports_commit", "transition"):
        _str(val, w, 1)
    elif key == "usage":
        _obj(val, w, ("invocation_id", "basis"), ("measure", "amount"))
        _str(val["invocation_id"], w, 1)
        _enum(val["basis"], ("observed", "estimated", "unknown", "enforced_bound"), w)
        if "measure" in val:
            _str(val["measure"], w, 1)
        if "amount" in val:
            _int(val["amount"], w)
    elif key == "context_delivery":
        _obj(val, w, ("binding_id", "boundary", "harness"))
        _str(val["binding_id"], w, 1)
        _enum(val["boundary"], ("initial_prompt", "turn_steering", "context_request"), w)
        _enum(val["harness"], ("acknowledged", "accepted", "queued", "lost"), w)
    elif key == "transport_errors":
        _int(val, w)
    elif key == "output":
        _obj(val, w, (), ("text", "repeat", "bytes"))
        if "text" in val:
            _str(val["text"], w)
            if "repeat" in val or "bytes" in val:
                _fail(f"{w}: text excludes repeat and bytes")
        elif "repeat" in val:
            _str(val["repeat"], w, 1)
            _int(val.get("bytes", 0), w)
        else:
            _fail(f"{w} needs text, or repeat and bytes")
    elif key == "output_lost":
        _obj(val, w, ("reason",), ("bytes",))
        _enum(val["reason"], ("harness_dropped", "capture_unavailable"), w)
        if "bytes" in val:
            _int(val["bytes"], w)
    elif key == "runtime_burst":
        _int(val, w, 1)
    else:
        _fail(f"{what}: unknown script step {key!r}")


def parse_executor_config(raw) -> dict:
    cfg = _obj(raw, "executor", (), ("adapter", "scripts", "default_script", "recovery_policy", "host_id", "capacity",
                                     "budget_pools", "context_packets", "installations", "output_spool_bytes"))
    adapter = _obj(cfg.get("adapter", {}), "executor.adapter", (),
                   ("enforcement", "echo_proves_delivery", "predicates", "steering", "enforced_bounds", "context_boundaries"))
    if "enforcement" in adapter:
        _enum(adapter["enforcement"], tuple(LEVEL), "executor.adapter.enforcement")
    if "echo_proves_delivery" in adapter and not isinstance(adapter["echo_proves_delivery"], bool):
        _fail("executor.adapter.echo_proves_delivery must be a boolean")
    for idx, pred in enumerate(adapter.get("predicates", [])):
        _obj(pred, f"executor.adapter.predicates[{idx}]", ("name", "status"))
        _str(pred["name"], "predicate name", 1)
        _enum(pred["status"], ("supported", "unsupported", "unknown"), "predicate status")
    if "steering" in adapter:
        _enum(adapter["steering"], ("live", "unsupported"), "executor.adapter.steering")
    _str_list(adapter.get("enforced_bounds", []), "executor.adapter.enforced_bounds")
    for b in adapter.get("context_boundaries", []):
        _enum(b, ("initial_prompt", "turn_steering", "context_request"), "executor.adapter.context_boundaries")
    scripts = _obj(cfg.get("scripts", {}), "executor.scripts", (), tuple(cfg.get("scripts", {}) or ()))
    for xid, script in scripts.items():
        if not isinstance(script, list):
            _fail(f"executor.scripts.{xid} must be an array")
        for idx, step in enumerate(script):
            validate_step(step, f"executor.scripts.{xid}[{idx}]")
    default = cfg.get("default_script", [])
    if not isinstance(default, list):
        _fail("executor.default_script must be an array")
    for idx, step in enumerate(default):
        validate_step(step, f"executor.default_script[{idx}]")
    if "recovery_policy" in cfg:
        _enum(cfg["recovery_policy"], ("resume", "terminate"), "executor.recovery_policy")
    if "host_id" in cfg:
        _str(cfg["host_id"], "executor.host_id", 1)
    if "capacity" in cfg:
        _int(cfg["capacity"], "executor.capacity")
    for name, pool in cfg.get("budget_pools", {}).items():
        _obj(pool, f"executor.budget_pools.{name}", ("measure", "limit"))
        _str(pool["measure"], "pool measure", 1)
        _int(pool["limit"], "pool limit")
    for idx, packet in enumerate(cfg.get("context_packets", [])):
        _obj(packet, f"executor.context_packets[{idx}]", ("ref", "digest"))
        _str(packet["ref"], "packet ref", 1)
        _str(packet["digest"], "packet digest", 1)
    for idx, inst in enumerate(cfg.get("installations", [])):
        w = f"executor.installations[{idx}]"
        _obj(inst, w, ("installation_id", "harness", "detected"),
             ("version", "adapter_recognized", "version_supported", "authentication", "reachable", "last_verified"))
        _str(inst["installation_id"], w, 1)
        _str(inst["harness"], w, 1)
        if not isinstance(inst["detected"], bool):
            _fail(f"{w}.detected must be a boolean")
        for member in ("adapter_recognized", "version_supported", "reachable"):
            if member in inst:
                _enum(inst[member], ("yes", "no", "unknown"), f"{w}.{member}")
        if "authentication" in inst:
            _enum(inst["authentication"], ("authenticated", "unauthenticated", "unknown"), f"{w}.authentication")
        if "last_verified" in inst and not C.valid_instant(inst["last_verified"]):
            _fail(f"{w}.last_verified must be an instant")
    if "output_spool_bytes" in cfg:
        _int(cfg["output_spool_bytes"], "executor.output_spool_bytes", 1)
    return cfg


# ------------------------------------------------------------------ executor

class Executor:
    def __init__(self, provider, cfg: dict):
        self.p = provider
        adapter = cfg.get("adapter", {})
        self.enforcement = adapter.get("enforcement")
        self.echo_proves = adapter.get("echo_proves_delivery", False)
        self.predicates = {pred["name"]: pred["status"] for pred in adapter.get("predicates", [])}
        self.steering = adapter.get("steering", "unsupported")
        self.enforced_bounds = set(adapter.get("enforced_bounds", []))
        self.context_boundaries = set(adapter.get("context_boundaries", []))
        self.scripts = cfg.get("scripts", {})
        self.default_script = cfg.get("default_script", [])
        self.policy = cfg.get("recovery_policy", "resume")
        self.host_id = cfg.get("host_id", DEFAULT_HOST)
        self.capacity = cfg.get("capacity")
        self.pools = cfg.get("budget_pools", {})
        self.packets = {(pk["ref"], pk["digest"]) for pk in cfg.get("context_packets", [])}
        self.installations = cfg.get("installations", [])
        self.spool_bytes = cfg.get("output_spool_bytes", DEFAULT_SPOOL)

    # ------------------------------------------------------------ basics
    @property
    def store(self):
        return self.p.store

    def now(self) -> str:
        return self.p.clock.now()

    @property
    def controller_scope(self) -> str:
        return f"execution.controller:{self.host_id}"

    def controller_subject(self) -> dict:
        return {"kind": CONTROLLER_KIND, "id": self.host_id}

    def controller_epoch(self) -> int:
        return self.store.revision(CONTROLLER_KIND, self.host_id)

    def load(self, xid: str):
        return self.store.get_json(EXECUTION_KIND, xid)

    def load_effect(self, eid: str):
        return self.store.get_json(EFFECT_KIND, eid)

    def script_for(self, xid: str) -> list:
        return self.scripts.get(xid, self.default_script)

    def adapter_predicates(self) -> list:
        """Adapter predicates reported in the Core capability snapshot
        (EXECUTION 10: they reuse CORE 17)."""
        return [{"name": name, "status": status, "evidence": {"source": "conformance launch configuration (scripted adapter)"}}
                for name, status in sorted(self.predicates.items())]

    @staticmethod
    def subject_of(xid: str) -> dict:
        return {"kind": EXECUTION_KIND, "id": xid}

    def _emit(self, changes, subject: dict, revision: int, etype: str, payload: dict) -> None:
        if changes is not None:
            changes.append((etype, subject, revision, payload))
        else:
            self.store.append_event({"type": etype, "subject": subject, "revision": revision, "origin": "provider",
                                     "caused_by": [], "recorded_at": self.now(), "payload": payload})

    def _xevent(self, st: dict, etype: str, payload: dict) -> None:
        st["rev"] += 1
        self._emit(st["changes"], self.subject_of(st["id"]), st["rev"], etype, payload)

    def _observed(self, st: dict) -> None:
        """An executor observation: restarts the inactivity count (EXECUTION 8)."""
        st["x"]["last_observation_at"] = self.now()

    # ------------------------------------------------------------ effects
    def _effect_new(self, st, eid, kind, payload_digest, retry_class, operation_ref, source,
                    obligation=None, idempotency_key=None) -> None:
        now = self.now()
        descriptor = {"id": eid, "kind": kind, "target": self.subject_of(st["id"]), "payload_digest": payload_digest,
                      "authorization": dict(st["x"]["authorization"]), "retry_class": retry_class,
                      "operation_ref": operation_ref}
        if idempotency_key is not None:
            descriptor["idempotency_key"] = idempotency_key
        record = {"descriptor": descriptor,
                  "observations": [{"status": "pending", "evidence": {"class": "recorded", "source": source},
                                    "recorded_at": now}],
                  "attempts": [], "obligations": []}
        if obligation is not None:
            expects, deadline = obligation
            record["obligations"].append({"id": f"{eid}.obligation", "expects": expects, "deadline": deadline,
                                          "state": "open"})
        self.store.put_json(EFFECT_KIND, eid, 1, record)
        st["x"]["effects"].append(eid)

    def _effect_update(self, eid: str, fn) -> dict:
        row = self.load_effect(eid)
        if row is None:
            raise RuntimeError(f"effect {eid} vanished")
        revision, record = row
        if fn(record):
            revision += 1
        self.store.put_json(EFFECT_KIND, eid, revision, record)
        return record

    @staticmethod
    def effect_status(record: dict) -> str:
        return record["observations"][-1]["status"]

    def _observe(self, record: dict, status: str, evidence: dict) -> bool:
        if self.effect_status(record) == status:
            return False
        record["observations"].append({"status": status, "evidence": dict(evidence), "recorded_at": self.now()})
        return True

    @staticmethod
    def _close_obligations(record: dict, state: str) -> bool:
        changed = False
        for ob in record["obligations"]:
            if ob["state"] in ("open", "overdue"):
                ob["state"] = state
                changed = True
        return changed

    def _attempt(self, st, eid: str) -> str:
        """One try at an effect's external action (CORE 19.3 "Attempts").
        ``transport_errors`` makes the next tries end with unknown outcomes."""
        x = st["x"]
        outcome = "completed"
        if x["transport_errors"] > 0:
            x["transport_errors"] -= 1
            outcome = "unknown"

        def fn(record):
            attempt = {"attempt": len(record["attempts"]) + 1, "outcome": outcome, "recorded_at": self.now()}
            if "idempotency_key" in record["descriptor"]:
                attempt["idempotency_key"] = record["descriptor"]["idempotency_key"]  # the same key every time
            record["attempts"].append(attempt)
            return True

        self._effect_update(eid, fn)
        return outcome

    # --------------------------------------------------------- deliveries
    def _determine(self, st, value: str, evidence: dict, proof_class: str | None = None,
                   reconciled: str | None = None) -> None:
        """Set the prompt delivery's current determination (EXECUTION 3.1). A
        changed determination moves the previous one, with its evidence, to
        the history; evidence recorded while the determination stays the same
        replaces the current evidence (G-HISTORY)."""
        x = st["x"]
        rec = x["delivery_record"]
        now = self.now()
        if rec["delivery"] != value:
            previous = {"delivery": rec["delivery"], "recorded_at": rec["determined_at"]}
            if "evidence" in rec:
                previous["evidence"] = rec["evidence"]
            rec["history"].append(previous)
            rec["delivery"] = value
            rec["determined_at"] = now
            if value == "ambiguous":
                x["ambiguous_since"] = now
        rec["evidence"] = dict(evidence)
        if proof_class is not None:
            rec["proof_class"] = proof_class
        else:
            rec.pop("proof_class", None)
        x["delivery"] = value
        if reconciled is None:
            payload = {"delivery_id": rec["delivery_id"], "delivery": value}
            if proof_class is not None:
                payload["proof_class"] = proof_class
            payload["evidence"] = dict(evidence)
            self._xevent(st, "execution.delivery.observed", payload)
        else:
            self._xevent(st, "execution.delivery.reconciled",
                         {"delivery_id": rec["delivery_id"], "outcome": reconciled, "delivery": value,
                          "evidence": dict(evidence)})

        def fn(record):
            changed = self._observe(record, STATUS_OF[value], evidence)
            if value in TERMINAL:
                changed = self._close_obligations(record, "satisfied") or changed
            return changed

        self._effect_update(rec["delivery_id"], fn)
        if value == "failed_before_delivery":
            self._release_budget(x)

    def _overdue(self, st, eid: str) -> bool:
        """Mark an effect's open obligations overdue, one provider-origin event
        each on the execution subject (CORE 19.4, EXECUTION 9)."""
        marked = []

        def fn(record):
            for ob in record["obligations"]:
                if ob["state"] == "open":
                    ob["state"] = "overdue"
                    marked.append(ob["id"])
            return bool(marked)

        self._effect_update(eid, fn)
        for ob_id in marked:
            self._xevent(st, "core.effect.obligation.overdue", {"effect": eid, "obligation": ob_id})
        return bool(marked)

    # ------------------------------------------------------------ budgets
    def _pool_in_use(self, pool: str) -> int:
        used = 0
        for xid in self.store.ids_of(EXECUTION_KIND):
            _, x = self.load(xid)
            b = x.get("budget")
            if b and b["pool"] == pool and b["reservation"] in ("reserved", "settled"):
                used += b["amount"]
        return used

    @staticmethod
    def _release_budget(x: dict) -> None:
        b = x.get("budget")
        if b and b["reservation"] == "reserved":
            b["reservation"] = "released"

    @staticmethod
    def _liability(x: dict) -> str:
        latest = {}
        for obs in x["usage_observations"]:
            latest[obs["invocation_id"]] = obs["basis"]
        if not latest:
            return "none"
        return "resolved" if all(b in ("observed", "enforced_bound") for b in latest.values()) else "unresolved"

    # ------------------------------------------------------------ context
    def binding_state(self, binding: dict) -> str:
        if (binding["packet"]["ref"], binding["packet"]["digest"]) in self.packets:
            return "satisfied"
        return "gap" if binding["obligation"] == "advisory" else "unsatisfied"

    # ---------------------------------------------------------- admission
    def _refusal(self, payload: dict):
        """EXECUTION 5 "Order": restriction enforcement, adapter predicates,
        then the budget. Returns (reason, alternative or None) or None."""
        for restriction in payload.get("restrictions", []):
            if self.enforcement is None:
                return "enforcement_unavailable", None  # unknown is not supported
            if LEVEL[restriction["enforcement"]] > LEVEL[self.enforcement]:
                return ("enforcement_unavailable",
                        f"request enforcement {self.enforcement}, the strongest level this adapter enforces")
        for name in payload.get("adapter", {}).get("requires", []):
            if self.predicates.get(name, "unknown") != "supported":
                return "capability_unavailable", None  # G-PREDICATE-REASON
        budget = payload.get("budget")
        if budget is not None:
            pool = self.pools.get(budget["pool"])
            if pool is None:
                return "budget_unavailable", None
            if budget["ceiling"] == "hard" and pool["measure"] not in self.enforced_bounds:
                return ("enforcement_unavailable",
                        f"request a soft ceiling; this adapter cannot enforce a hard bound on {pool['measure']}")
            if self._pool_in_use(budget["pool"]) + budget.get("amount", 1) > pool["limit"]:
                return "budget_exhausted", None
        return None

    @staticmethod
    def _finished(x: dict) -> bool:
        """An admitted execution stops occupying capacity once its process is
        observed to have ended or its delivery provably failed (G-CAPACITY)."""
        return (x["runtime"] == "exited" or x["exit"] != "unavailable"
                or x["delivery"] in ("failed_before_delivery", "not_delivered")
                or x.get("cancellation", {}).get("outcome") == "cancelled")

    def _running(self) -> int:
        count = 0
        for xid in self.store.ids_of(EXECUTION_KIND):
            _, x = self.load(xid)
            if x["admission"] == "admitted" and not self._finished(x):
                count += 1
        return count

    def _queue_reason(self, x: dict):
        if any(b["obligation"] == "required_before_start" and self.binding_state(b) != "satisfied"
               for b in x.get("context_bindings", [])):
            return "context_binding_unsatisfied"
        if self.capacity is not None and self._running() >= self.capacity:
            return "capacity"
        return None

    def _admit(self, st) -> str:
        x = st["x"]
        now = self.now()
        delivery_id = f"{st['id']}.delivery-1"
        x["admission"] = "admitted"
        x["admitted_at"] = now
        x["last_observation_at"] = now
        x["delivery_id"] = delivery_id
        x["delivery"] = "pending"
        x.pop("queue_reason", None)
        x["delivery_record"] = {"delivery_id": delivery_id, "delivery": "pending", "determined_at": now, "history": []}
        timeouts = x["timeouts"]
        deadline = C.add_seconds(now, timeouts["delivery"]) if "delivery" in timeouts else None
        # The effect is recorded before any external I/O, in the transaction
        # that admits the execution (CORE 19.1).
        self._effect_new(st, delivery_id, "execution.prompt_submission", x["brief"]["digest"], "non_repeatable",
                         x["operation_ref"], "execution.submit", obligation=("delivery_evidence", deadline))
        return delivery_id

    # ------------------------------------------------- command handlers
    def check_exists(self, xid: str):
        row = self.load(xid)
        if row is None:
            raise ProtocolError("not_found")
        return row

    def check_controller_epoch(self, env: dict, auth) -> None:
        """EXECUTION 11.3: once the host has a controller epoch, mutating
        commands carry it."""
        current = self.controller_epoch()
        claimed = env.get("authority_epoch")
        if current == 0:
            if claimed is not None and claimed > 0:
                raise ProtocolError("unknown_authority_epoch")
            return
        if claimed is None or claimed < current:
            details = {"current_epoch": current} if auth.may_read(self.controller_subject()) else {}
            raise ProtocolError("stale_authority_epoch", details)
        if claimed > current:
            raise ProtocolError("unknown_authority_epoch")

    def _authorization(self, auth) -> dict:
        authorization = {"principal": self.p.principal}
        if auth.grant is not None:
            authorization["grant"] = auth.grant["id"]
        return authorization

    def op_submit(self, env: dict, auth, operation_ref: str):
        xid = env["subject"]["id"]
        self.check_controller_epoch(env, auth)
        self.p.check_preconditions(env, auth)
        payload = env["payload"]
        now = self.now()
        features = self.p.session.features()
        x = {
            "brief": payload["brief"], "authorization": self._authorization(auth), "operation_ref": operation_ref,
            "command_id": env["command_id"], "submitted_at": now, "timeouts": dict(payload.get("timeouts", {})),
            "runtime": "preparing", "result": "absent", "exit": "unavailable", "evaluation": "not_requested",
            "effects": [], "completions": [], "recovery": [], "passed": [], "generation": 1,
            "script_pos": 0, "dispatched": False, "transport_errors": 0, "stalled": False,
            "counters": {"cancels": 0, "steers": 0, "responses": 0, "probes": 0, "answered": 0,
                         "wait_cancel": 0, "wait_steer": 0, "wait_action": 0},
            "cancel_forwards": [], "cancel_awaiting": [], "attempts_due": [],
            "steering": [], "actions": [], "annotations": [], "checkpoints": [], "usage_observations": [],
            "context_deliveries": [], "transitions": [],
        }
        for member in ("predecessor", "correlation"):
            if member in payload:
                x[member] = payload[member]
        if "context_bindings" in payload:
            x["context_bindings"] = [dict(b) for b in payload["context_bindings"]]
        if "continuation" in payload:
            cont = payload["continuation"]
            predicate = "adapter.resume" if cont["mode"] == "resume" else "adapter.fork"
            native = self.predicates.get(predicate, "unknown") == "supported"
            label = ("resumed" if cont["mode"] == "resume" else "forked") if native else "fresh_continuation"
            x["continuation"] = {"of": cont["of"], "requested": cont["mode"], "label": label}

        st = {"id": xid, "rev": 0, "x": x, "changes": []}
        refusal = self._refusal(payload)
        event = {}
        if refusal is not None:
            reason, alternative = refusal
            x["admission"] = "refused"
            x["reason"] = reason
            if alternative is not None:
                x["alternative"] = alternative
            # No delivery exists; nothing can ever be dispatched (G-REFUSED-AXES).
            x["delivery"] = "failed_before_delivery"
            x["runtime"] = "unknown"
            event = {"admission": "refused", "reason": reason}
        else:
            queue_reason = self._queue_reason(x)
            if queue_reason is not None:
                x["admission"] = "queued"
                x["queue_reason"] = queue_reason
                x["delivery"] = "pending"
                event = {"admission": "queued", "queue_reason": queue_reason}
            else:
                delivery_id = self._admit(st)
                event = {"admission": "admitted", "delivery_id": delivery_id}
            if "workspace" in payload:
                ws = payload["workspace"]
                lease = {"lease_id": f"{xid}.lease-1", "repository": ws["repository"], "base": ws["base"],
                         "writer": self.p.principal, "lease_epoch": 1, "cleanup": ws["cleanup"]}
                for member in ("permitted_paths", "permitted_effects"):
                    if member in ws:
                        lease[member] = list(ws[member])
                x["workspace_lease"] = lease
        if "budget" in payload:
            b = payload["budget"]
            x["budget"] = {"pool": b["pool"], "ceiling": b["ceiling"], "amount": b.get("amount", 1),
                           "reservation": "not_reserved" if refusal is not None else "reserved"}
            if b["pool"] in self.pools:
                x["budget"]["measure"] = self.pools[b["pool"]]["measure"]
        self._xevent(st, "execution.admission.changed", event)
        self.store.put_json(EXECUTION_KIND, xid, st["rev"], x)

        outcome = {"execution": self.subject_of(xid), "admission": x["admission"]}
        for member in ("reason", "alternative", "delivery_id", "queue_reason"):
            if member in x and (member != "delivery_id" or x["admission"] == "admitted"):
                outcome[member] = x[member]
        if "continuation" in x and "execution.continuation" in features:
            outcome["continuation"] = dict(x["continuation"])
        effect_refs = [x["delivery_id"]] if x["admission"] == "admitted" else []
        return st["rev"], outcome, st["changes"], effect_refs

    def op_cancel(self, env: dict, auth, operation_ref: str):
        xid = env["subject"]["id"]
        revision, x = self.check_exists(xid)
        self.check_controller_epoch(env, auth)
        self.p.check_preconditions(env, auth)
        st = {"id": xid, "rev": revision, "x": x, "changes": []}
        n = x["counters"]["cancels"] + 1
        eid = f"{xid}.cancel-{n}"
        x["counters"]["cancels"] = n
        # EXECUTION 7 "Forwarding effect": idempotent_key with the effect ID
        # as its key, and an open obligation for the outcome.
        self._effect_new(st, eid, "execution.cancel_forwarding", sha256_of(V.canonical(env["payload"])),
                         "idempotent_key", operation_ref, "execution.cancel",
                         obligation=("cancellation_outcome", None), idempotency_key=eid)
        receipt = {"state": "cancel_requested", "operation_ref": operation_ref}
        x["cancellation"] = {"receipt": receipt, "effect": eid}
        x["cancel_forwards"].append(eid)
        self._xevent(st, "execution.cancel.requested", {"receipt": receipt})
        self.store.put_json(EXECUTION_KIND, xid, st["rev"], x)
        return st["rev"], {"receipt": receipt}, st["changes"], [eid]

    def op_steer(self, env: dict, auth, operation_ref: str):
        xid = env["subject"]["id"]
        revision, x = self.check_exists(xid)
        self.check_controller_epoch(env, auth)
        self.p.check_preconditions(env, auth)
        st = {"id": xid, "rev": revision, "x": x, "changes": []}
        n = x["counters"]["steers"] + 1
        x["counters"]["steers"] = n
        steer_id = f"{xid}.steer-{n}"
        entry = {"steer_id": steer_id, "recorded_at": self.now(), "behavior": "not_observed"}
        if self.steering == "live":
            delivery_id = f"{steer_id}.delivery"
            self._effect_new(st, delivery_id, "execution.steering_delivery", env["payload"]["message"]["digest"],
                             "non_repeatable", operation_ref, "execution.steer", obligation=("steering_evidence", None))
            entry.update(request="recorded", delivery_id=delivery_id, delivery="pending")
            x["attempts_due"].append(delivery_id)
            outcome = {"steer_id": steer_id, "request": "recorded", "delivery_id": delivery_id}
            refs = [delivery_id]
        else:
            entry.update(request="not_supported", alternative=STEER_ALTERNATIVE)
            outcome = {"steer_id": steer_id, "request": "not_supported", "alternative": STEER_ALTERNATIVE}
            refs = []
        x["steering"].append(entry)
        self._xevent(st, "execution.steer.requested", {"steer_id": steer_id, "request": entry["request"]})
        self.store.put_json(EXECUTION_KIND, xid, st["rev"], x)
        return st["rev"], outcome, st["changes"], refs

    def op_respond_action(self, env: dict, auth, operation_ref: str):
        xid = env["subject"]["id"]
        revision, x = self.check_exists(xid)
        self.check_controller_epoch(env, auth)
        self.p.check_preconditions(env, auth)
        action_id = env["payload"]["action_id"]
        action = next((a for a in x["actions"] if a["action_id"] == action_id and a["state"] == "pending"), None)
        if action is None:
            # EXECUTION 11.2: not a pending action of this execution.
            raise ProtocolError("not_found")
        st = {"id": xid, "rev": revision, "x": x, "changes": []}
        n = x["counters"]["responses"] + 1
        x["counters"]["responses"] = n
        eid = f"{xid}.response-{n}"
        self._effect_new(st, eid, "execution.action_response", env["payload"]["response"]["digest"],
                         "non_repeatable", operation_ref, "execution.respond_action")
        action.update(state="answered", answered_at=self.now(), response_effect=eid)
        x["counters"]["answered"] += 1
        x["attempts_due"].append(eid)
        self._xevent(st, "execution.action.answered", {"action_id": action_id, "response_effect": eid})
        self.store.put_json(EXECUTION_KIND, xid, st["rev"], x)
        return st["rev"], {"action_id": action_id, "state": "answered", "response_effect": eid}, st["changes"], [eid]

    def op_checkpoint(self, env: dict, auth, operation_ref: str):
        xid = env["subject"]["id"]
        revision, x = self.check_exists(xid)
        lease = x.get("workspace_lease")
        if lease is None:
            raise ProtocolError("not_found")  # EXECUTION 11.4
        self.check_controller_epoch(env, auth)
        self.p.check_preconditions(env, auth)
        st = {"id": xid, "rev": revision, "x": x, "changes": []}
        probe = x.get("probe")
        probes = set(probe["probes"]) if probe else set()
        coverage = {area: ("probed" if area in probes else "not_probed") for area in ("tracked", "dirty", "untracked")}
        checkpoint = {"checkpoint_id": f"{xid}.checkpoint-{len(x['checkpoints']) + 1}",
                      "lease_epoch": lease["lease_epoch"], "recorded_at": self.now()}
        if probe:
            checkpoint["probed_at"] = probe["probed_at"]
        if "tracked" in probes and "head" in probe:
            checkpoint["head"] = probe["head"]
        checkpoint["coverage"] = coverage
        checkpoint["complete"] = len(probes) == 3
        if "dirty" in probes:
            checkpoint["dirty_paths"] = list(probe.get("dirty_paths", []))
        if "untracked" in probes:
            checkpoint["untracked_paths"] = list(probe.get("untracked_paths", []))
        checkpoint["annotations"] = [dict(a) for a in x["annotations"]]
        x["checkpoints"].append(checkpoint)
        self._xevent(st, "execution.workspace.checkpointed",
                     {"checkpoint_id": checkpoint["checkpoint_id"], "complete": checkpoint["complete"]})
        self.store.put_json(EXECUTION_KIND, xid, st["rev"], x)
        return st["rev"], checkpoint, st["changes"], []

    def op_claim(self, env: dict, auth, operation_ref: str):
        if env["subject"]["id"] != self.host_id:
            raise ProtocolError("not_found")  # a host this executor does not own (EXECUTION 11.3)
        self.p.check_preconditions(env, auth)
        epoch = self.controller_epoch() + 1
        self.store.write_subject(CONTROLLER_KIND, self.host_id, epoch, None)
        changes = [("execution.controller.claimed", self.controller_subject(), epoch,
                    {"epoch": epoch, "controller": self.p.principal})]
        return epoch, {"epoch": epoch}, changes, []

    def op_abort_obligation(self, env: dict, auth, operation_ref: str):
        eid = env["subject"]["id"]
        row = self.load_effect(eid)
        if row is None:
            raise ProtocolError("not_found")
        self.p.check_preconditions(env, auth)
        revision, record = row
        ob_id = env["payload"]["obligation"]
        ob = next((o for o in record["obligations"] if o["id"] == ob_id and o["state"] in ("open", "overdue")), None)
        if ob is None:
            raise ProtocolError("not_found")  # CORE 19.4
        ob["state"] = "aborted"  # the effect's status is left exactly as observed (EFF-4)
        revision += 1
        self.store.put_json(EFFECT_KIND, eid, revision, record)
        target = record["descriptor"]["target"]
        changes = [("core.effect.obligation.aborted", {"kind": EFFECT_KIND, "id": eid}, revision,
                    {"effect": eid, "obligation": ob_id, "target": dict(target)})]
        outcome = {"effect": eid, "obligation": {"id": ob_id, "state": "aborted"}, "status": self.effect_status(record)}
        return revision, outcome, changes, []

    # ---------------------------------------------------------- queries
    def op_effects_get(self, env: dict, auth) -> dict:
        row = self.load_effect(env["payload"]["effect"])
        if row is None:
            raise ProtocolError("not_found")
        revision, record = row
        return {"effect": record["descriptor"], "revision": revision, "status": self.effect_status(record),
                "observations": record["observations"], "attempts": record["attempts"],
                "obligations": record["obligations"]}

    def _open_obligations(self, effect_ids) -> list:
        out = []
        for eid in effect_ids:
            row = self.load_effect(eid)
            if row is not None:
                out.extend(dict(o) for o in row[1]["obligations"] if o["state"] in ("open", "overdue"))
        return out

    def op_inspect(self, env: dict, auth) -> dict:
        xid = env["payload"]["execution"]
        revision, x = self.check_exists(xid)
        features = self.p.session.features()
        view = {"execution": self.subject_of(xid), "revision": revision, "admission": x["admission"],
                "delivery": x["delivery"], "runtime": x["runtime"], "result": x["result"], "exit": x["exit"],
                "evaluation": x["evaluation"]}
        for member in ("predecessor", "correlation", "finalized_by", "reason", "alternative", "queue_reason"):
            if member in x:
                view[member] = x[member]
        view["deliveries"] = [x["delivery_record"]] if "delivery_record" in x else []
        view["completions"] = x["completions"]
        view["effects"] = list(x["effects"])
        view["obligations"] = self._open_obligations(x["effects"])
        if "cancellation" in x:
            view["cancellation"] = {k: v for k, v in x["cancellation"].items() if k in ("receipt", "outcome")}
        view["host"] = {"id": self.host_id, "generation": x["generation"]}
        view["next_cursor"] = self.p.head_cursor()
        view["recovery"] = x["recovery"]
        if x["runtime"] == "requires_action" and "runtime_detail" in x:
            view["runtime_detail"] = x["runtime_detail"]
        # Members of optional features appear only in sessions that selected
        # the feature (G-INSPECT-GATING).
        if "execution.steering" in features:
            view["steering"] = x["steering"]
        if "execution.actions" in features:
            view["actions"] = x["actions"]
        if "execution.workspaces" in features and "workspace_lease" in x:
            view["workspace"] = {"lease": x["workspace_lease"], "checkpoints": x["checkpoints"]}
        if "execution.usage" in features:
            usage = {"observations": x["usage_observations"], "liability": self._liability(x)}
            if "budget" in x:
                usage["budget"] = x["budget"]
            view["usage"] = usage
        if "execution.context" in features:
            bindings = []
            for b in x.get("context_bindings", []):
                bindings.append(dict(b, state=self.binding_state(b)))
            view["context"] = {"bindings": bindings, "deliveries": x["context_deliveries"]}
        if "execution.continuation" in features and "continuation" in x:
            view["continuation"] = x["continuation"]
        return view

    def op_reconcile(self, env: dict, auth) -> dict:
        """EXECUTION 4: the scoped observations the executor holds. Reads
        only; never submits, resubmits or restarts (EXE-10)."""
        payload = env["payload"]
        empty = {"executions": [], "deliveries": [], "obligations": []}
        effect_ids: list[str] = []
        xid = None
        if "command_id" in payload:
            record = self.store.command_record(self.p.scope, payload["command_id"])
            if record is None:
                return empty
            ack = V.loads(bytes(record[2]).decode("utf-8"))
            subject = ack["subject"]
            effect_ids = list(ack["effect_refs"])
            if subject["kind"] == EXECUTION_KIND:
                xid = subject["id"]
            elif subject["kind"] == EFFECT_KIND:
                row = self.load_effect(subject["id"])
                if row is not None:
                    xid = row[1]["descriptor"]["target"]["id"]
                    effect_ids = [subject["id"]]
            if xid is not None:
                row = self.load(xid)
                # A queued submit recorded its delivery later, at admission.
                if row is not None and row[1]["operation_ref"] == ack["operation_ref"] and "delivery_id" in row[1]:
                    if row[1]["delivery_id"] not in effect_ids:
                        effect_ids.insert(0, row[1]["delivery_id"])
        else:
            row = self.load_effect(payload["delivery_id"])
            if row is None or row[1]["descriptor"]["kind"] not in ("execution.prompt_submission",
                                                                    "execution.steering_delivery"):
                return empty
            xid = row[1]["descriptor"]["target"]["id"]
            effect_ids = [payload["delivery_id"]]
        if xid is None:
            return empty
        subject = self.subject_of(xid)
        row = self.load(xid)
        if row is None or not auth.sees(subject):
            return empty  # hidden executions are not held for this reader (G-RECONCILE-AUTH)
        _, x = row
        deliveries = []
        for eid in effect_ids:
            if eid == x.get("delivery_id"):
                deliveries.append(x["delivery_record"])
                continue
            entry = next((s for s in x["steering"] if s.get("delivery_id") == eid), None)
            if entry is not None:
                rec = {"delivery_id": eid, "delivery": entry["delivery"]}
                for member in ("evidence", "proof_class"):
                    if member in entry:
                        rec[member] = entry[member]
                rec["history"] = []
                deliveries.append(rec)
        return {"executions": [subject], "deliveries": deliveries, "obligations": self._open_obligations(effect_ids)}

    def op_discovery(self, env: dict, auth) -> dict:
        out = []
        for inst in self.installations:
            item = {"installation_id": inst["installation_id"], "harness": inst["harness"]}
            if "version" in inst:
                item["version"] = inst["version"]
            item["detected"] = inst["detected"]
            # Missing facts are unknown, never positive (EXECUTION 11.5).
            for member in ("adapter_recognized", "version_supported"):
                item[member] = inst.get(member, "unknown")
            item["authentication"] = inst.get("authentication", "unknown")
            item["reachable"] = inst.get("reachable", "unknown")
            if "last_verified" in inst:
                item["last_verified"] = inst["last_verified"]
            item["usable"] = (item["detected"] and item["adapter_recognized"] == "yes"
                              and item["version_supported"] == "yes" and item["authentication"] == "authenticated"
                              and item["reachable"] == "yes" and "last_verified" in item)
            out.append(item)
        return {"installations": out}

    def op_output_read(self, env: dict, auth) -> dict:
        payload = env["payload"]
        xid = payload["execution"]
        self.check_exists(xid)
        start, data, lost = self.store.output(xid)
        end = start + len(data)
        requested = payload.get("offset")
        if requested is None or requested < start:
            offset = start  # the oldest retained byte
        else:
            offset = min(requested, end)  # G-OUTPUT-OFFSET
        max_bytes = payload.get("max_bytes", DEFAULT_READ_BYTES)
        chunk = data[offset - start: offset - start + max_bytes]

        def build(n):
            piece = chunk[:n]
            result = {"execution": self.subject_of(xid), "offset": offset,
                      "data_base64": base64.b64encode(piece).decode("ascii"), "next_offset": offset + len(piece),
                      "end_offset": end, "lost_ranges": lost, "coverage": "incomplete" if lost else "complete",
                      "policy": {"spool_bytes": self.spool_bytes, "overflow": "discard_oldest"}}
            return result

        n = len(chunk)
        result = build(n)
        while n > 0 and not self.p.response_fits(result):
            n = n * 3 // 4  # shrink until the response frame fits the caller's receive limit
            result = build(n)
        return result

    # --------------------------------------------------------- recovery
    def _submitter_authorized(self, x: dict) -> bool:
        authorization = x["authorization"]
        if "grant" not in authorization:
            return authorization["principal"] in self.p.authorities
        row = self.store.grant(authorization["grant"])
        if row is None or row[1]["holder"] != authorization["principal"]:
            return False
        return G.usable_problem(row[1], self.now(), self.p.epoch_of) is None

    def _deadline_passed(self, x: dict) -> bool:
        now = self.now()
        for name in ("delivery", "execution_deadline"):
            if name in x["timeouts"] and now >= C.add_seconds(x["admitted_at"], x["timeouts"][name]):
                return True
        return False

    def recover(self, journal_intact: bool) -> None:
        """EXECUTION 7.1, run once at every start before any request."""
        for xid in self.store.ids_of(EXECUTION_KIND):
            self._unit(xid, lambda st: self._recover_one(st, journal_intact))

    def _recover_one(self, st, journal_intact: bool) -> bool:
        x = st["x"]
        if x["admission"] != "admitted" or x["delivery"] != "pending":
            return False  # only a delivery still pending is recovered; terminal ones are never reopened
        if x.get("marker") is not None:
            decision, reason = "ambiguous", "dispatch_may_have_begun"
        elif not journal_intact:
            decision, reason = "ambiguous", "journal_not_intact"
        elif x["counters"]["cancels"] > 0:
            decision, reason = "failed_before_delivery", "cancelled"
        elif not self._submitter_authorized(x):
            decision, reason = "failed_before_delivery", "authorization_lost"
        elif self._deadline_passed(x):
            decision, reason = "failed_before_delivery", "deadline_passed"
        elif self.policy == "terminate":
            decision, reason = "failed_before_delivery", "recovery_policy"
        else:
            decision, reason = "dispatch_resumed", "provably_not_dispatched"
        delivery_id = x["delivery_id"]
        x["recovery"].append({"delivery_id": delivery_id, "decision": decision, "reason": reason,
                              "recorded_at": self.now()})
        if decision == "dispatch_resumed":
            # Fencing: the recovered dispatcher is a new host generation.
            x["generation"] += 1
            host = {"id": self.host_id, "generation": x["generation"]}
            self._xevent(st, "execution.host.changed", {"host": host})
            self._xevent(st, "execution.recovery.decided",
                         {"delivery_id": delivery_id, "decision": decision, "reason": reason, "host": host})
        else:
            host = {"id": self.host_id, "generation": x["generation"]}
            self._xevent(st, "execution.recovery.decided",
                         {"delivery_id": delivery_id, "decision": decision, "reason": reason, "host": host})
            self._determine(st, decision, {"class": "recovery", "source": reason})
        return True

    # ------------------------------------------------------------- ticks
    def _unit(self, xid, fn) -> bool:
        """One unit of executor work in its own owner transaction."""
        st_store = self.store
        st_store.begin()
        try:
            row = self.load(xid)
            if row is None:
                st_store.rollback()
                return False
            st = {"id": xid, "rev": row[0], "x": row[1], "changes": None}
            progressed = fn(st)
            if progressed:
                st_store.put_json(EXECUTION_KIND, xid, st["rev"], st["x"])
                st_store.commit()
            else:
                st_store.rollback()
            if st.get("crash"):
                log("scripted crash:", st["crash"], "in", xid)
                os._exit(CRASH_EXIT_STATUS)
            return progressed
        except BaseException:
            st_store.rollback()
            raise

    def tick(self, allow_crash: bool) -> bool:
        """Advance time-driven transitions, queued admissions, pending effect
        attempts and scripts until nothing more can happen now."""
        progressed_any = False
        for _ in range(100000):
            if not self._tick_once(allow_crash):
                break
            progressed_any = True
        return progressed_any

    def _tick_once(self, allow_crash: bool) -> bool:
        ids = self.store.ids_of(EXECUTION_KIND)
        for xid in ids:
            if self._unit(xid, self._timeouts):
                return True
        for xid in ids:
            row = self.load(xid)
            if row[1]["admission"] == "queued" and self._unit(xid, self._try_admit):
                return True  # oldest first
        for xid in ids:
            if self._unit(xid, self._pending_attempts):
                return True
            if self._unit(xid, lambda st: self._script_step(st, allow_crash)):
                return True
        return False

    def _timeouts(self, st) -> bool:
        x = st["x"]
        timeouts = x["timeouts"]
        passed = x["passed"]
        now = self.now()

        def due(name, since):
            return (name in timeouts and name not in passed and since is not None
                    and now >= C.add_seconds(since, timeouts[name]))

        def pass_(name):
            passed.append(name)
            self._xevent(st, "execution.timeout.passed", {"timeout": name})

        if x["admission"] == "queued":
            if due("queue", x["submitted_at"]):
                pass_("queue")
                x["admission"] = "refused"
                x["reason"] = "queue_timeout"
                x.pop("queue_reason", None)
                x["delivery"] = "failed_before_delivery"
                self._release_budget(x)
                self._xevent(st, "execution.admission.changed", {"admission": "refused", "reason": "queue_timeout"})
                return True
            return False
        if x["admission"] != "admitted":
            return False
        progressed = False
        admitted_at = x["admitted_at"]
        if x["delivery"] == "pending" and due("delivery", admitted_at):
            pass_("delivery")
            evidence = {"class": "timeout", "source": "delivery timeout"}
            # The evidence wait ends (EXECUTION 3.1, 8).
            self._determine(st, "ambiguous" if x.get("marker") is not None else "failed_before_delivery", evidence)
            self._overdue(st, x["delivery_id"])
            progressed = True
        if due("execution_deadline", admitted_at):
            pass_("execution_deadline")  # nothing else changes
            progressed = True
        if x["runtime"] != "exited" and due("inactivity", x["last_observation_at"]):
            pass_("inactivity")  # runtime and exit stay as observed
            progressed = True
        if x["delivery"] == "ambiguous" and due("reconciliation", x.get("ambiguous_since")):
            pass_("reconciliation")  # delivery stays ambiguous
            self._overdue(st, x["delivery_id"])
            progressed = True
        # Obligation deadlines pass even with no other traffic (CORE 19.4).
        for eid in x["effects"]:
            row = self.load_effect(eid)
            if any(o["state"] == "open" and o["deadline"] is not None and now >= o["deadline"]
                   for o in row[1]["obligations"]):
                marked = []

                def fn(record):
                    for o in record["obligations"]:
                        if o["state"] == "open" and o["deadline"] is not None and now >= o["deadline"]:
                            o["state"] = "overdue"
                            marked.append(o["id"])
                    return bool(marked)

                self._effect_update(eid, fn)
                for ob_id in marked:
                    self._xevent(st, "core.effect.obligation.overdue", {"effect": eid, "obligation": ob_id})
                progressed = True
        return progressed

    def _try_admit(self, st) -> bool:
        x = st["x"]
        reason = self._queue_reason(x)
        if reason is None:
            delivery_id = self._admit(st)
            self._xevent(st, "execution.admission.changed", {"admission": "admitted", "delivery_id": delivery_id})
            return True
        if reason != x.get("queue_reason"):
            x["queue_reason"] = reason
            self._xevent(st, "execution.admission.changed", {"admission": "queued", "queue_reason": reason})
            return True
        return False

    def _pending_attempts(self, st) -> bool:
        x = st["x"]
        if x["cancel_forwards"]:
            eid = x["cancel_forwards"].pop(0)
            completed = False
            for _ in range(MAX_ATTEMPTS):  # idempotent_key: retried with the same key
                if self._attempt(st, eid) == "completed":
                    completed = True
                    break
            if completed:
                x["cancel_awaiting"].append(eid)
                if x.get("on_cancel") is not None:
                    self._observe_cancel(st, eid, x["on_cancel"])
            else:
                self._observe_cancel(st, eid, "unknown")  # no attempt completed
            return True
        if x["attempts_due"]:
            eid = x["attempts_due"].pop(0)
            outcome = self._attempt(st, eid)  # non_repeatable: one try only
            if outcome == "unknown":
                evidence = {"class": "attempt_outcome_unknown", "source": SOURCE}
                entry = next((s for s in x["steering"] if s.get("delivery_id") == eid), None)
                if entry is not None and entry["delivery"] == "pending":
                    entry["delivery"] = "ambiguous"
                    entry["evidence"] = evidence
                    self._xevent(st, "execution.steer.delivery.observed",
                                 {"steer_id": entry["steer_id"], "delivery_id": eid, "delivery": "ambiguous",
                                  "evidence": evidence})
                self._effect_update(eid, lambda record: self._observe(record, "unknown", evidence))
            return True
        return False

    def _observe_cancel(self, st, eid: str, outcome: str) -> None:
        x = st["x"]
        reported = eid in x["cancel_awaiting"]  # the forwarding reached the harness
        if reported:
            x["cancel_awaiting"].remove(eid)
        if x.get("cancellation", {}).get("effect") == eid:
            x["cancellation"]["outcome"] = outcome
        self._xevent(st, "execution.cancel.observed", {"outcome": outcome})
        if outcome == "unknown":
            evidence = {"class": "harness_report" if reported else "attempt_outcome_unknown", "source": SOURCE}
            self._effect_update(eid, lambda record: self._observe(record, "unknown", evidence))
        else:
            evidence = {"class": "harness_report", "source": SOURCE}

            def fn(record):
                changed = self._observe(record, "succeeded", evidence)
                return self._close_obligations(record, "satisfied") or changed

            self._effect_update(eid, fn)

    # ------------------------------------------------------------ scripts
    def _dispatch_needed(self, x: dict) -> bool:
        return not x["dispatched"] and x["delivery"] == "pending"

    def _dispatch(self, st) -> bool:
        """Write-ahead marker first, in its own committed transaction; the
        harness write and its attempt record come in the next unit."""
        x = st["x"]
        if x.get("marker") is None:
            x["marker"] = {"generation": x["generation"], "recorded_at": self.now()}
            return True
        outcome = self._attempt(st, x["delivery_id"])
        x["dispatched"] = True
        if outcome == "unknown":
            self._determine(st, "ambiguous", {"class": "attempt_outcome_unknown", "source": SOURCE})
        return True

    def _script_step(self, st, allow_crash: bool) -> bool:
        x = st["x"]
        if x["admission"] != "admitted" or x["stalled"]:
            return False
        script = self.script_for(st["id"])
        pos = x["script_pos"]
        if pos >= len(script):
            return self._dispatch(st) if self._dispatch_needed(x) else False
        (key, val), = script[pos].items()
        now = self.now()
        if key == "wait_until":
            if now < val:
                return False
        elif key == "wait_for":
            counters = x["counters"]
            have, used = {"cancel": ("cancels", "wait_cancel"), "steer": ("steers", "wait_steer"),
                          "action": ("answered", "wait_action")}[val]
            if counters[have] <= counters[used]:
                return False
            counters[used] += 1
        elif key == "crash":
            if not allow_crash:
                return False  # never crash in the middle of answering a request
            if val == "after_write" and self._dispatch_needed(x):
                return self._dispatch(st)
            x["script_pos"] = pos + 1
            st["crash"] = val
            return True
        elif key not in CONTROL_STEPS and self._dispatch_needed(x):
            return self._dispatch(st)
        else:
            self._apply(st, key, val)
        x["script_pos"] = pos + 1
        return True

    def _apply(self, st, key: str, val) -> None:
        x = st["x"]
        now = self.now()
        handler = getattr(self, f"_step_{key}")
        if key not in CONTROL_STEPS:
            self._observed(st)
        handler(st, x, val, now)

    def _step_transport_errors(self, st, x, val, now):
        x["transport_errors"] = val

    def _step_on_cancel(self, st, x, val, now):
        x["on_cancel"] = val
        for eid in list(x["cancel_awaiting"]):
            self._observe_cancel(st, eid, val)

    def _step_stall(self, st, x, val, now):
        x["stalled"] = True  # the harness never reports again

    def _step_deliver(self, st, x, proof_class, now):
        if x["delivery"] in TERMINAL:
            log("deliver step after a terminal determination ignored for", st["id"])
            return
        evidence = {"class": proof_class, "source": SOURCE}
        establishes = proof_class == "provider_ack_id" or (proof_class == "echo" and self.echo_proves)
        if establishes:
            self._determine(st, "acknowledged", evidence, proof_class)
        else:
            # Transport acceptance, terminal writes and an unproven echo
            # establish nothing (EXECUTION 3.1).
            self._determine(st, x["delivery"], evidence, proof_class)

    def _step_reconcile_finds(self, st, x, finding, now):
        if "delivery_record" not in x or x["delivery"] in TERMINAL:
            log("reconcile_finds step without an unresolved delivery ignored for", st["id"])
            return
        evidence = {"class": "reconciliation", "source": SOURCE}
        if finding in ("delivered", "not_delivered"):
            self._determine(st, finding, evidence, reconciled=finding)
        else:
            self._determine(st, x["delivery"], evidence, reconciled="unknown")

    def _step_runtime(self, st, x, runtime, now):
        payload = {"runtime": runtime}
        if runtime == "requires_action":
            pending = [a for a in x["actions"] if a["state"] == "pending"]
            if not pending:
                # requires_action always carries an action identity (EXE-4).
                log("runtime requires_action without a pending action ignored for", st["id"])
                return
            detail = {"action_id": pending[-1]["action_id"], "owner": pending[-1]["owner"]}
            x["runtime_detail"] = detail
            payload.update(detail)
        else:
            x.pop("runtime_detail", None)
        x["runtime"] = runtime
        self._xevent(st, "execution.runtime.changed", payload)

    def _step_runtime_burst(self, st, x, count, now):
        for _ in range(count):
            self._step_runtime(st, x, "quiescent" if x["runtime"] == "active" else "active", now)

    def _step_host_restart(self, st, x, val, now):
        x["generation"] += 1
        self._xevent(st, "execution.host.changed", {"host": {"id": self.host_id, "generation": x["generation"]}})
        x.pop("runtime_detail", None)
        x["runtime"] = "unknown"  # the new host process has not been observed yet
        self._xevent(st, "execution.runtime.changed", {"runtime": "unknown"})

    def _step_complete(self, st, x, val, now):
        generation = val.get("generation", x["generation"])
        digest = sha256_of(val["content"].encode("utf-8"))
        cid = val["completion_id"]
        existing = [c for c in x["completions"] if c["completion_id"] == cid and c["status"] == "recorded"]
        if generation < x["generation"]:
            status = "superseded_attempt"  # retained, never finalizes (EXECUTION 6)
        elif existing:
            status = "duplicate" if existing[0]["digest"] == digest else "conflict"
        else:
            status = "recorded"
        x["completions"].append({"completion_id": cid, "digest": digest, "generation": generation, "status": status})
        host = {"id": self.host_id, "generation": generation}
        self._xevent(st, "execution.completion.recorded",
                     {"completion_id": cid, "digest": digest, "invocation_id": f"{st['id']}.invocation-{generation}",
                      "host": host, "status": status})
        if status == "recorded" and "finalized_by" not in x:
            x["finalized_by"] = cid
            x["result"] = "returned"
            self._xevent(st, "execution.result.changed", {"result": "returned"})

    def _step_exit(self, st, x, exit_value, now):
        x["exit"] = dict(exit_value)
        self._xevent(st, "execution.exit.observed", {"exit": dict(exit_value)})

    def _step_stale_dispatch(self, st, x, val, now):
        if val["generation"] < x["generation"]:
            self._xevent(st, "execution.dispatch.fenced",
                         {"delivery_id": x.get("delivery_id", ""), "generation": val["generation"],
                          "current_generation": x["generation"]})
        else:
            log("stale_dispatch from the current generation is not stale; ignored for", st["id"])

    def _steering_target(self, x, want):
        for entry in x["steering"]:
            if entry["request"] == "recorded" and want(entry):
                return entry
        return None

    def _step_steer_deliver(self, st, x, proof_class, now):
        entry = self._steering_target(x, lambda e: e["delivery"] in ("pending", "ambiguous"))
        if entry is None:
            log("steer_deliver without a recorded steer awaiting evidence ignored for", st["id"])
            return
        evidence = {"class": proof_class, "source": SOURCE}
        establishes = proof_class == "provider_ack_id" or (proof_class == "echo" and self.echo_proves)
        if establishes:
            entry["delivery"] = "acknowledged"
        entry["proof_class"] = proof_class
        entry["evidence"] = evidence
        self._xevent(st, "execution.steer.delivery.observed",
                     {"steer_id": entry["steer_id"], "delivery_id": entry["delivery_id"],
                      "delivery": entry["delivery"], "evidence": evidence})
        if establishes:
            def fn(record):
                changed = self._observe(record, "succeeded", evidence)
                return self._close_obligations(record, "satisfied") or changed

            self._effect_update(entry["delivery_id"], fn)

    def _step_steer_behavior(self, st, x, val, now):
        entry = self._steering_target(x, lambda e: e["behavior"] == "not_observed")
        if entry is None:
            log("steer_behavior without a recorded steer ignored for", st["id"])
            return
        evidence = {"class": "observed_behavior", "source": SOURCE}
        entry["behavior"] = "observed"
        entry["behavior_evidence"] = evidence
        self._xevent(st, "execution.steer.behavior.observed", {"steer_id": entry["steer_id"], "evidence": evidence})

    def _step_request_action(self, st, x, val, now):
        if any(a["action_id"] == val["action_id"] for a in x["actions"]):
            log("duplicate action request ignored for", st["id"])
            return
        x["actions"].append({"action_id": val["action_id"], "owner": val["owner"], "state": "pending",
                             "requested_at": now})
        self._step_runtime(st, x, "requires_action", now)

    def _step_workspace(self, st, x, val, now):
        probe = {"probes": list(val["probes"]), "probed_at": now}
        for member in ("head", "dirty_paths", "untracked_paths"):
            if member in val:
                probe[member] = val[member]
        x["probe"] = probe

    def _step_agent_reports_commit(self, st, x, value, now):
        x["annotations"].append({"kind": "agent_reported_commit", "value": value, "basis": "agent_report",
                                 "recorded_at": now})

    def _step_usage(self, st, x, val, now):
        obs = {"invocation_id": val["invocation_id"], "basis": val["basis"]}
        for member in ("measure", "amount"):
            if member in val:
                obs[member] = val[member]
        obs["recorded_at"] = now
        x["usage_observations"].append(obs)
        self._xevent(st, "execution.usage.observed", dict(obs))
        budget = x.get("budget")
        if budget and budget["reservation"] == "reserved" and self._liability(x) == "resolved":
            budget["reservation"] = "settled"

    def _step_context_delivery(self, st, x, val, now):
        binding = next((b for b in x.get("context_bindings", []) if b["binding_id"] == val["binding_id"]), None)
        if binding is None:
            log("context_delivery for an unknown binding ignored for", st["id"])
            return
        if val["boundary"] not in self.context_boundaries:
            outcome = "unavailable"
        elif binding["obligation"] == "required_before_transition" and binding["transition"] in x["transitions"]:
            outcome = "late"
        else:
            outcome = {"acknowledged": "acknowledged", "accepted": "delivered", "queued": "queued",
                       "lost": "unknown"}[val["harness"]]
        obs = {"binding_id": binding["binding_id"], "digest": binding["packet"]["digest"],
               "target": {"execution": self.subject_of(st["id"])}, "boundary": val["boundary"],
               "outcome": outcome, "recorded_at": now}
        x["context_deliveries"].append(obs)
        self._xevent(st, "execution.context.delivery.observed", dict(obs))

    def _step_transition(self, st, x, name, now):
        x["transitions"].append(name)
        self._xevent(st, "execution.transition.observed", {"transition": name})

    def _step_probe_status(self, st, x, val, now):
        n = x["counters"]["probes"] + 1
        x["counters"]["probes"] = n
        eid = f"{st['id']}.probe-{n}"
        self._effect_new(st, eid, "execution.status_probe", sha256_of(b""), "read", x["operation_ref"],
                         "executor status probe")
        completed = False
        for _ in range(MAX_ATTEMPTS):  # read class: retried under the executor's policy
            if self._attempt(st, eid) == "completed":
                completed = True
                break
        evidence = {"class": "status_probe", "source": SOURCE}
        self._effect_update(eid, lambda record: self._observe(record, "succeeded" if completed else "unknown", evidence))

    def _step_output(self, st, x, val, now):
        if "text" in val:
            data = val["text"].encode("utf-8")
        else:
            unit = val["repeat"].encode("utf-8")
            count = val.get("bytes", 0)
            data = (unit * (count // len(unit) + 1))[:count]
        start, spool, lost = self.store.output(st["id"])
        spool += data
        if len(spool) > self.spool_bytes:
            discard = len(spool) - self.spool_bytes
            loss = {"from": start, "to": start + discard, "bytes": discard, "reason": "spool_limit",
                    "coverage": "incomplete"}
            if lost and lost[-1]["reason"] == "spool_limit" and lost[-1]["to"] == start:
                lost[-1]["to"] += discard  # adjacent discards coalesce
                lost[-1]["bytes"] += discard
            else:
                lost.append(dict(loss))
            start += discard
            spool = spool[discard:]
            self._xevent(st, "execution.output.lost", loss)
        self.store.set_output(st["id"], start, bytes(spool), lost)

    def _step_output_lost(self, st, x, val, now):
        start, spool, lost = self.store.output(st["id"])
        end = start + len(spool)
        loss = {"from": end, "to": end}
        if "bytes" in val:
            loss["bytes"] = val["bytes"]
        loss.update(reason=val["reason"], coverage="incomplete")
        lost.append(dict(loss))
        self.store.set_output(st["id"], start, bytes(spool), lost)
        self._xevent(st, "execution.output.lost", loss)
