"""The context/1 profile over scripted preparation (CONTEXT.md, M4 draft).

Written from CONTEXT.md, EVIDENCE.md, CORE.md, the conformance README's
``context`` launch control and schemas/context/1 only.

Durable records are ordinary subjects in the provider's SQLite store:

- ``context.request``: the submitted request, its state and item results, and
  every published packet revision with its reference and result facts. Its
  revision rises by one with every request event.
- ``context.job``: the preparation work: subscribers in submission order, the
  script position and everything the script produced so far (sections,
  coverage, unmet items, omissions, corrections, conditions).
- ``context.packet``: one subject per request (CONTEXT 5), whose revision is
  the latest packet revision. Packet bytes are sealed as ordinary Evidence
  artifacts in this provider's own store (M4-Q2, the standalone case).

Scripts advance in ticks before and after every request and on the idle
re-check. Section H of DIVERGENCES.md records the choices the documents leave
open; the tags below refer to it.
"""

from __future__ import annotations

import base64

import clock as C
import envelope as E
import evidence as EVD
import grants as G
import knowledge as K
import valuedomain as V
from envelope import Invalid, ptr
from errors import ProtocolError, denied

REQUEST_KIND = "context.request"
JOB_KIND = "context.job"
PACKET_KIND = "context.packet"
PACKET_MEDIA_TYPE = "application/vnd.combraton.context-packet+json"
PACKET_FORMAT = "combraton-context-packet/1"
CLAIMS_PACKET_FORMAT = "combraton-context-packet/2"  # CONTEXT 14
RELIANCE_RANK = {"reference": 0, "hypothesis": 1, "evidence": 2, "binding": 3}
COMPILER = "combraton-independent-python-core"  # CONTEXT 12: names the implementation (H8-STEP8-COMPILER)
DEFAULT_EXCERPT_BYTES = 4096  # CONTEXT 6
OBLIGATION_FEATURES = {"advisory": "context.advisory", "required_before_start": "context.required_before_start",
                       "required_before_transition": "context.required_before_transition"}
RELIANCE = ("binding", "evidence", "hypothesis", "reference")
LABELS = ("binding", "observation", "hypothesis", "unknown", "declared_requirement", "source_inspected",
          "runtime_observed", "inferred", "stale")
OMISSION_REASONS = ("output_capacity", "authorization", "applicability", "unavailable")
CONDITION_KINDS = ("repository_tree", "dirty_snapshot", "environment_digest", "authority_revision")
STEP_KEYS = ("wait_until", "investigate", "section", "coverage", "unmet", "omit", "correction", "conditions",
             "publish", "end", "stall")


class ContextConfigError(Exception):
    pass


def _fail(msg: str):
    raise ContextConfigError(msg)


def _try(fn, what: str):
    try:
        fn()
    except Invalid as exc:
        _fail(f"{what}: {exc}")


def condition(v, path: str) -> dict:
    """schemas/context/1/common.schema.json#/$defs/condition"""
    E.closed(v, path, ("condition_id", "kind", "expected"), ("repository", "item_id"))
    E.identifier(v["condition_id"], ptr(path, "condition_id"))
    if v["kind"] not in CONDITION_KINDS:
        raise Invalid(ptr(path, "kind"), "unknown condition kind")
    if "repository" in v:
        E.string(v["repository"], ptr(path, "repository"), 1, 128)
    if "item_id" in v:
        E.identifier(v["item_id"], ptr(path, "item_id"))
    E.string(v["expected"], ptr(path, "expected"), 1, 256)
    return v


def packet_reference(v, path: str) -> dict:
    """schemas/context/1/common.schema.json#/$defs/packet_reference"""
    E.closed(v, path, ("packet", "revision", "artifact"))
    E.subject(v["packet"], ptr(path, "packet"), PACKET_KIND)
    E.integer(v["revision"], ptr(path, "revision"), 1)
    a = E.closed(v["artifact"], ptr(path, "artifact"), ("provider", "artifact", "digest"))
    E.identifier(a["provider"], ptr(ptr(path, "artifact"), "provider"))
    E.subject(a["artifact"], ptr(ptr(path, "artifact"), "artifact"), EVD.ARTIFACT_KIND)
    E.digest_string(a["digest"], ptr(ptr(path, "artifact"), "digest"))
    return v


def _validate_step(step, what: str) -> None:
    if not isinstance(step, dict) or len(step) != 1:
        _fail(f"{what} must be an object with exactly one member")
    (key, val), = step.items()
    w = f"{what}.{key}"
    if key == "execute":
        _fail(f"{w}: investigation executions need a separately running executor (socket binding)")
    if key not in STEP_KEYS:
        _fail(f"{what}: unknown context script step {key!r}")
    if key == "wait_until":
        if not C.valid_instant(val):
            _fail(f"{w} must be an instant")
    elif key == "investigate":
        if not E.is_int(val) or val < 0:
            _fail(f"{w} must be an integer >= 0")
    elif key == "section":
        def check():
            E.closed(val, w, ("section_id", "label", "content"),
                     ("item_id", "citations", "authority_revision", "source", "claim"))
            E.identifier(val["section_id"], ptr(w, "section_id"))
            if val["label"] not in LABELS:
                raise Invalid(ptr(w, "label"), "unknown label")
            E.string(val["content"], ptr(w, "content"))
            if "item_id" in val:
                E.identifier(val["item_id"], ptr(w, "item_id"))
            if "citations" in val:
                E.array(val["citations"], ptr(w, "citations"), max_items=256)
                for idx, c in enumerate(val["citations"]):
                    cp = ptr(ptr(w, "citations"), idx)
                    E.closed(c, cp, ("citation_id", "evidence"))
                    E.identifier(c["citation_id"], ptr(cp, "citation_id"))
                    EVD.reference(c["evidence"], ptr(cp, "evidence"))
            if "authority_revision" in val:
                E.integer(val["authority_revision"], ptr(w, "authority_revision"))
            if "claim" in val:
                K.claim_reference(val["claim"], ptr(w, "claim"))
            if "source" in val:
                s = E.closed(val["source"], ptr(w, "source"), ("repository", "path"), ("tree",))
                E.string(s["repository"], ptr(ptr(w, "source"), "repository"), 1, 128)
                E.string(s["path"], ptr(ptr(w, "source"), "path"), 1, 1024)
                if "tree" in s:
                    E.string(s["tree"], ptr(ptr(w, "source"), "tree"), 1, 128)
        _try(check, w)
    elif key == "coverage":
        def check():
            E.closed(val, w, ("producer", "frontier", "gaps"))
            E.string(val["producer"], ptr(w, "producer"), 1, 128)
            E.string(val["frontier"], ptr(w, "frontier"), 1, 256)
            E.array(val["gaps"], ptr(w, "gaps"), max_items=256)
            for idx, gap in enumerate(val["gaps"]):
                E.string(gap, ptr(ptr(w, "gaps"), idx), 1, 512)
        _try(check, w)
    elif key == "unmet":
        def check():
            E.closed(val, w, ("item_id", "reason"))
            E.identifier(val["item_id"], ptr(w, "item_id"))
            E.string(val["reason"], ptr(w, "reason"), 1, 64)
        _try(check, w)
    elif key == "omit":
        def check():
            E.closed(val, w, ("reason",), ("item_id", "section_id"))
            if val["reason"] not in OMISSION_REASONS:
                raise Invalid(ptr(w, "reason"), "unknown omission reason")
            for member in ("item_id", "section_id"):
                if member in val:
                    E.identifier(val[member], ptr(w, member))
        _try(check, w)
    elif key == "correction":
        def check():
            E.closed(val, w, ("item_id", "authority_revision"))
            E.identifier(val["item_id"], ptr(w, "item_id"))
            E.integer(val["authority_revision"], ptr(w, "authority_revision"))
        _try(check, w)
    elif key == "conditions":
        def check():
            E.array(val, w, max_items=64)
            for idx, cond in enumerate(val):
                condition(cond, ptr(w, idx))
        _try(check, w)
    elif key == "publish":
        if val != {}:
            _fail(f"{w}: this provider understands only publish: {{}}")
    elif key == "end":
        if not isinstance(val, str) or not 1 <= len(val) <= 64:
            _fail(f"{w} must be a reason string")
    elif key == "stall":
        if val is not True:
            _fail(f"{w} must be true")


def parse_context_config(raw) -> dict:
    if not isinstance(raw, dict):
        _fail("context must be an object")
    for member in ("executor", "evidence_provider", "knowledge_provider"):
        if member in raw:
            _fail(f"context.{member} needs the socket binding; this provider is stdio only")
    unknown = set(raw) - {"scripts", "default_script"}
    if unknown:
        _fail(f"context: unknown members {sorted(unknown)}")
    scripts = raw.get("scripts", {})
    if not isinstance(scripts, dict):
        _fail("context.scripts must be an object")
    for rid, script in scripts.items():
        if not isinstance(script, list):
            _fail(f"context.scripts.{rid} must be an array")
        for idx, step in enumerate(script):
            _validate_step(step, f"context.scripts.{rid}[{idx}]")
    default = raw.get("default_script", [])
    if not isinstance(default, list):
        _fail("context.default_script must be an array")
    for idx, step in enumerate(default):
        _validate_step(step, f"context.default_script[{idx}]")
    return raw


def request_subject(rid: str) -> dict:
    return {"kind": REQUEST_KIND, "id": rid}


def job_subject(jid: str) -> dict:
    return {"kind": JOB_KIND, "id": jid}


def packet_subject(rid: str) -> dict:
    return {"kind": PACKET_KIND, "id": rid}


def _only_primary_precondition(env: dict) -> dict:
    if len(env["preconditions"]) != 1:
        raise Invalid("/preconditions", "exactly one precondition is required")
    if env["preconditions"][0]["subject"] != env["subject"]:
        raise Invalid("/preconditions/0/subject", "the precondition must name the command's subject")
    return env["preconditions"][0]


def _utf8_len(text: str) -> int:
    return len(text.encode("utf-8"))


class Context:
    def __init__(self, provider, cfg: dict):
        self.p = provider
        self.scripts = cfg.get("scripts", {})
        self.default_script = cfg.get("default_script", [])

    @property
    def store(self):
        return self.p.store

    @property
    def evidence(self):
        return self.p.evidence

    def now(self) -> str:
        return self.p.clock.now()

    def script_for(self, rid: str) -> list:
        return self.scripts.get(rid, self.default_script)

    def load(self, rid: str):
        return self.store.get_json(REQUEST_KIND, rid)

    def load_job(self, jid: str):
        return self.store.get_json(JOB_KIND, jid)

    def _emit(self, changes, etype: str, subject: dict, revision: int, payload: dict) -> None:
        if changes is not None:
            changes.append((etype, subject, revision, payload))
        else:
            self.store.append_event({"type": etype, "subject": subject, "revision": revision, "origin": "provider",
                                     "caused_by": [], "recorded_at": self.now(), "payload": payload})

    # ------------------------------------------------------- validators
    def v_submit(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", REQUEST_KIND)
        if _only_primary_precondition(env)["revision"] != 0:
            raise Invalid("/preconditions/0/revision", "context.request.submit creates the request: revision must be 0")
        payload = E.closed(env["payload"], "/payload", ("consumer", "basis", "items", "limits"),
                           ("fallback", "authority_content", "origin"))
        consumer = E.closed(payload["consumer"], "/payload/consumer", ("task", "principal"), ("executor",))
        E.string(consumer["task"], "/payload/consumer/task", 1, 256)
        E.identifier(consumer["principal"], "/payload/consumer/principal")
        if "executor" in consumer:
            E.identifier(consumer["executor"], "/payload/consumer/executor")
        self._basis(payload["basis"], "/payload/basis")
        items = E.array(payload["items"], "/payload/items", 1, 256)
        seen = set()
        for idx, item in enumerate(items):
            p = ptr("/payload/items", idx)
            self._item(item, p)
            if item["item_id"] in seen:
                raise Invalid(ptr(p, "item_id"), "item_id listed more than once")
            seen.add(item["item_id"])
        if "fallback" in payload and payload["fallback"] not in ("proceed_with_gap", "wait_until_deadline"):
            raise Invalid("/payload/fallback", "must be proceed_with_gap or wait_until_deadline")
        limits = E.closed(payload["limits"], "/payload/limits", ("deadline", "investigation", "output_capacity"))
        E.instant(limits["deadline"], "/payload/limits/deadline")
        inv = E.closed(limits["investigation"], "/payload/limits/investigation", ("units", "amount"))
        E.string(inv["units"], "/payload/limits/investigation/units", 1, 64)
        E.integer(inv["amount"], "/payload/limits/investigation/amount")
        cap = E.closed(limits["output_capacity"], "/payload/limits/output_capacity", ("units", "amount"))
        if cap["units"] != "bytes":
            raise Invalid("/payload/limits/output_capacity/units", "must be bytes")
        E.integer(cap["amount"], "/payload/limits/output_capacity/amount")
        if "authority_content" in payload:
            E.array(payload["authority_content"], "/payload/authority_content", max_items=256)
            for idx, entry in enumerate(payload["authority_content"]):
                p = ptr("/payload/authority_content", idx)
                E.closed(entry, p, ("item_id", "evidence", "authority_revision"))
                E.identifier(entry["item_id"], ptr(p, "item_id"))
                if entry["item_id"] not in seen:
                    raise Invalid(ptr(p, "item_id"), "names no item of this request")
                EVD.reference(entry["evidence"], ptr(p, "evidence"))
                E.integer(entry["authority_revision"], ptr(p, "authority_revision"))
        if "origin" in payload:
            o = E.closed(payload["origin"], "/payload/origin", ("initiator", "depth", "call_budget"))
            E.subject(o["initiator"], "/payload/origin/initiator")
            E.integer(o["depth"], "/payload/origin/depth")
            E.integer(o["call_budget"], "/payload/origin/call_budget")

    @staticmethod
    def _basis(v, path: str) -> None:
        E.closed(v, path, ("repositories", "completeness"), ("environment", "configuration", "build"))
        repos = E.array(v["repositories"], ptr(path, "repositories"), 1, 64)
        commit_only = False
        for idx, repo in enumerate(repos):
            p = ptr(ptr(path, "repositories"), idx)
            E.closed(repo, p, ("id", "tree", "workspace", "dirty"))
            E.string(repo["id"], ptr(p, "id"), 1, 128)
            E.string(repo["tree"], ptr(p, "tree"), 1, 128)
            if repo["workspace"] not in ("clean", "dirty", "unknown"):
                raise Invalid(ptr(p, "workspace"), "must be clean, dirty or unknown")
            if repo["dirty"] is not None:
                d = E.closed(repo["dirty"], ptr(p, "dirty"), ("snapshot_digest",))
                E.digest_string(d["snapshot_digest"], ptr(ptr(p, "dirty"), "snapshot_digest"))
            elif repo["workspace"] != "clean":
                commit_only = True
        for member in ("environment", "configuration", "build"):
            if member in v:
                E.string(v[member], ptr(path, member), 1, 256)
        if v["completeness"] not in ("complete", "partial"):
            raise Invalid(ptr(path, "completeness"), "must be complete or partial")
        if commit_only and v["completeness"] != "partial":
            # CONTEXT 3 (CTX-2): a commit-only basis must declare partial.
            raise Invalid(ptr(path, "completeness"), "a commit-only basis for a non-clean workspace must be partial")

    @staticmethod
    def _item(item, p: str) -> None:
        E.closed(item, p, ("item_id", "selector", "obligation", "reliance", "selected_by", "check"), ("transition",))
        E.identifier(item["item_id"], ptr(p, "item_id"))
        sel = E.closed(item["selector"], ptr(p, "selector"), ("kind", "value"))
        E.string(sel["kind"], ptr(ptr(p, "selector"), "kind"), 1, 64)
        E.string(sel["value"], ptr(ptr(p, "selector"), "value"), 1, 512)
        if item["obligation"] not in OBLIGATION_FEATURES:
            raise Invalid(ptr(p, "obligation"), "unknown obligation")
        if "transition" in item:
            E.string(item["transition"], ptr(p, "transition"), 1, 128)
        # CONTEXT 3: present exactly when required_before_transition (H-TRANSITION-EXACTLY).
        if ("transition" in item) != (item["obligation"] == "required_before_transition"):
            raise Invalid(ptr(p, "transition"), "transition is present exactly for required_before_transition")
        if item["reliance"] not in RELIANCE:
            raise Invalid(ptr(p, "reliance"), "unknown reliance label")
        E.identifier(item["selected_by"], ptr(p, "selected_by"))
        check = item["check"]
        cp = ptr(p, "check")
        if not isinstance(check, dict):
            raise Invalid(cp, "must be an object")
        kind = check.get("kind")
        if kind == "source_included":
            E.closed(check, cp, ("kind", "repository", "path"))
            E.string(check["repository"], ptr(cp, "repository"), 1, 128)
            E.string(check["path"], ptr(cp, "path"), 1, 1024)
        elif kind == "evidence_included":
            E.closed(check, cp, ("kind", "evidence"))
            EVD.reference(check["evidence"], ptr(cp, "evidence"))
        elif kind == "authority_content_included":
            E.closed(check, cp, ("kind",))
        elif kind == "claim_included":
            E.closed(check, cp, ("kind", "claim"))
            K.claim_reference(check["claim"], ptr(cp, "claim"))
        else:
            raise Invalid(ptr(cp, "kind"), "unknown check kind")

    def submit_features(self, env) -> list:
        """CONTEXT 3: an obligation whose feature was not negotiated is refused
        at step 3 (H-CTX-OBLIGATION-STEP)."""
        selected = self.p.session.features()
        missing = []
        for item in env["payload"]["items"]:
            feature = OBLIGATION_FEATURES[item["obligation"]]
            if feature not in selected and feature not in missing:
                missing.append(feature)
            # CONTEXT 14: claim_included needs context.claims (HM5-CLAIMS-FEATURE-ORDER).
            if (item["check"]["kind"] == "claim_included" and "context.claims" not in selected
                    and "context.claims" not in missing):
                missing.append("context.claims")
        return missing

    def v_cancel(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", REQUEST_KIND)
        E.closed(env["payload"], "/payload", ())
        _only_primary_precondition(env)

    def v_request_inspect(self, env, method, features) -> None:
        E.query_envelope(env, method, "core.grants" in features)
        payload = E.closed(env["payload"], "/payload", ("request",))
        E.identifier(payload["request"], "/payload/request")

    def v_packet_inspect(self, env, method, features) -> None:
        E.query_envelope(env, method, "core.grants" in features)
        payload = E.closed(env["payload"], "/payload", ("packet", "revision"), ("offset", "max_bytes"))
        E.identifier(payload["packet"], "/payload/packet")
        E.integer(payload["revision"], "/payload/revision", 1)
        if "offset" in payload:
            E.integer(payload["offset"], "/payload/offset")
        if "max_bytes" in payload:
            E.integer(payload["max_bytes"], "/payload/max_bytes", 1, 16777216)

    def v_expand(self, env, method, features) -> None:
        E.query_envelope(env, method, "core.grants" in features)
        payload = E.closed(env["payload"], "/payload", ("packet", "revision", "citation"), ("offset", "max_bytes"))
        E.identifier(payload["packet"], "/payload/packet")
        E.integer(payload["revision"], "/payload/revision", 1)
        E.identifier(payload["citation"], "/payload/citation")
        if "offset" in payload:
            E.integer(payload["offset"], "/payload/offset")
        if "max_bytes" in payload:
            E.integer(payload["max_bytes"], "/payload/max_bytes", 1, 16777216)

    # ---------------------------------------------------- authorization
    def authorize(self, method: str, env: dict):
        p = self.p
        payload = env["payload"]
        if method in ("context.request.submit", "context.request.cancel"):
            return p.authorize_under(env, [("context.request", env["subject"])])
        if method == "context.request.inspect":
            return p.authorize_under(env, [("context.read", request_subject(payload["request"]))])
        if method == "context.packet.inspect":
            return p.authorize_under(env, [("context.packet.read", packet_subject(payload["packet"]))])
        if method == "context.expand":
            # H-CTX-EXPAND: the evidence.read right is existence-independent.
            return p.authorize_under(env, [("context.packet.read", packet_subject(payload["packet"])),
                                           ("evidence.read", None)])
        raise AssertionError(method)

    def job_visible(self, grant: dict, jid: str) -> bool:
        """CONTEXT 10: events of the jobs of covered requests (H-CTX-EVENT-VISIBILITY)."""
        if "context.read" not in grant["rights"]:
            return False
        row = self.load_job(jid)
        return row is not None and any(G.grant_covers(grant, request_subject(r)) for r in row[1]["requests"])

    # -------------------------------------------------------- commands
    def _mandatory_size(self, script: list, items: list) -> int:
        """H-CTX-BUDGET: the required items' scripted content before the first publish."""
        required = {i["item_id"] for i in items if i["obligation"] != "advisory"}
        total = 0
        for step in script:
            (key, val), = step.items()
            if key == "publish":
                break
            if key == "section" and val.get("item_id") in required:
                total += _utf8_len(val["content"])
        return total

    def _shared_job(self, payload: dict):
        """H-CTX-SHARED."""
        for jid in self.store.ids_of(JOB_KIND):
            job = self.load_job(jid)[1]
            if (job["ended"] is None and not job["published"] and job["principal"] == self.p.principal
                    and V.canonical(job["basis"]) == V.canonical(payload["basis"])
                    and V.canonical(job["items"]) == V.canonical(payload["items"])):
                return jid, job
        return None, None

    def op_submit(self, env, auth):
        self.p.check_preconditions(env, auth)
        rid = env["subject"]["id"]
        payload = env["payload"]
        features = self.p.session.features()
        jid, job = (None, None)
        if "context.shared_jobs" in features:
            jid, job = self._shared_job(payload)
        owner = job["owner"] if job is not None else rid
        mandatory = self._mandatory_size(self.script_for(owner), payload["items"])
        advisory = any(i["obligation"] == "advisory" for i in payload["items"])
        rec = {"consumer": payload["consumer"], "basis": payload["basis"], "items": payload["items"],
               "limits": payload["limits"], "authority_content": payload.get("authority_content", []),
               "principal": self.p.principal, "updates": "context.updates" in features,
               "claims": "context.claims" in features,
               "submitted_at": self.now(), "packets": []}
        if "fallback" in payload or advisory:
            rec["fallback"] = payload.get("fallback", "proceed_with_gap")  # H-CTX-SUBMIT-VALIDATION
        if "origin" in payload:
            rec["origin"] = payload["origin"]
        subject = request_subject(rid)
        if mandatory > payload["limits"]["output_capacity"]["amount"]:
            # CONTEXT 3 (CTX-8): never drop mandatory items to fit.
            needed = {"units": "bytes", "amount": mandatory}
            rec.update(state="refused", reason="budget_insufficient", needed=needed, job=None,
                       item_results=[self._result(i, False, "budget_insufficient") for i in payload["items"]])
            self.store.put_json(REQUEST_KIND, rid, 1, rec)
            return 1, {"request": subject, "state": "refused", "reason": "budget_insufficient", "needed": needed}, [
                ("context.request.changed", subject, 1, {"state": "refused", "reason": "budget_insufficient"})]
        if job is None:
            jid = rid  # H3-JOB-ID (fixture-informed): a job is named after the request that starts it
            job = {"requests": [], "owner": rid, "pos": 0, "spent": 0, "budget": payload["limits"]["investigation"]["amount"],
                   "sections": [], "coverage": [], "unmet": {}, "omissions": [], "corrections": {}, "conditions": [],
                   "published": False, "ended": None, "stalled": False, "principal": self.p.principal,
                   "basis": payload["basis"], "items": payload["items"]}
        job["requests"].append(rid)
        self.store.put_json(JOB_KIND, jid, self.store.revision(JOB_KIND, jid), job)
        rec.update(state="preparing", job=jid,
                   item_results=[{"item_id": i["item_id"], "obligation": i["obligation"], "result": "pending"}
                                 for i in payload["items"]])
        self.store.put_json(REQUEST_KIND, rid, 1, rec)
        return 1, {"request": subject, "state": "preparing", "job": job_subject(jid)}, [
            ("context.request.changed", subject, 1, {"state": "preparing", "job": job_subject(jid)})]

    @staticmethod
    def _result(item: dict, satisfied: bool, reason: str | None) -> dict:
        out = {"item_id": item["item_id"], "obligation": item["obligation"]}
        if satisfied:
            out["result"] = "satisfied"
        else:
            out["result"] = "degraded" if item["obligation"] == "advisory" else "unmet"
            out["reason"] = reason
        return out

    def op_cancel(self, env, auth):
        self.p.check_preconditions(env, auth)
        rid = env["subject"]["id"]
        row = self.load(rid)
        if row is None or row[1]["state"] != "preparing":
            raise ProtocolError("not_found")  # CONTEXT 4: nothing is being prepared for it
        revision, rec = row
        jid = rec["job"]
        jrev, job = self.load_job(jid)
        rec["state"] = "cancelled"
        revision += 1
        self.store.put_json(REQUEST_KIND, rid, revision, rec)
        continues = any(r != rid and self.load(r)[1]["state"] == "preparing" for r in job["requests"])
        changes = [("context.request.cancelled", request_subject(rid), revision, {"job_continues": continues})]
        if not continues and job["ended"] is None:
            job["ended"] = "no_subscribers"  # H-CTX-JOB-END
            jrev += 1
            changes.append(("context.job.ended", job_subject(jid), jrev, {"reason": "no_subscribers"}))
        self.store.put_json(JOB_KIND, jid, jrev, job)
        return revision, {"request": request_subject(rid), "state": "cancelled", "job": job_subject(jid),
                          "job_continues": continues}, changes

    # --------------------------------------------------------- queries
    def op_request_inspect(self, env, auth) -> dict:
        rid = env["payload"]["request"]
        row = self.load(rid)
        if row is None:
            raise ProtocolError("not_found")
        revision, rec = row
        view = {"request": request_subject(rid), "revision": revision, "state": rec["state"]}
        for member in ("reason", "needed"):
            if member in rec:
                view[member] = rec[member]
        if rec.get("job") is not None:
            view["job"] = job_subject(rec["job"])
        items = rec["item_results"]
        if rec["state"] == "preparing":
            items = self._live_items(rec)  # H3-LIVE-ITEMS (fixture-informed)
        view.update(consumer=rec["consumer"], limits=rec["limits"], items=items)
        last = len(rec["packets"])
        packets = []
        for pk in rec["packets"]:
            entry = {"reference": pk["reference"], "current": pk["revision"] == last}
            if "supersedes" in pk:
                entry["supersedes"] = pk["supersedes"]
            packets.append(entry)
        view["packets"] = packets
        return view

    def _packet(self, rid: str, revision: int):
        row = self.load(rid)
        if row is None:
            raise ProtocolError("not_found")
        rec = row[1]
        if not 1 <= revision <= len(rec["packets"]):
            raise ProtocolError("not_found")
        return rec, rec["packets"][revision - 1]

    def op_packet_inspect(self, env, auth) -> dict:
        payload = env["payload"]
        rec, pk = self._packet(payload["packet"], payload["revision"])
        facts = dict(pk["facts"])
        facts["current"] = pk["revision"] == len(rec["packets"])
        if pk["revision"] < len(rec["packets"]):
            facts["superseded_by"] = {"revision": len(rec["packets"])}  # CONTEXT 8: the current revision
        # CONTEXT 8 "Corrections after publication": reported at the read; the
        # stored facts never change.
        job = self.load_job(rec["job"])[1]
        current_authority = self._current_authority(rec, job)
        facts["invalidated_items"] = [
            {"item_id": a["item_id"], "authority_revision": current_authority[a["item_id"]]}
            for a in facts["authority"] if current_authority.get(a["item_id"], -1) > a["authority_revision"]]
        if "context.claims" in self.p.session.features():
            changes, invalidated, unverified = self.claim_reads(rec, facts)
            facts["invalidated_items"] += invalidated
            facts["claim_changes"] = changes
            facts["unverified_items"] = unverified
        else:
            # CONTEXT 14: sessions without context.claims see no claim members.
            facts["sections"] = [{k: v for k, v in sec.items() if k != "claim"} for sec in facts["sections"]]
        aid = pk["reference"]["artifact"]["artifact"]["id"]
        data = self.store.blob(aid) or b""
        size = self.evidence.load(aid)[1]["descriptor"]["size"]
        offset = payload.get("offset", 0)
        # CONTEXT 6: an exact byte range, never presented as complete bytes.
        chunk = data[offset: offset + payload.get("max_bytes", DEFAULT_EXCERPT_BYTES)] if offset < len(data) else b""
        return self.p.fit_bytes(lambda piece: dict(facts, excerpt={
            "offset": offset, "length": len(piece), "size": size,
            "data_base64": base64.b64encode(piece).decode("ascii")}), chunk)

    def op_expand(self, env, auth) -> dict:
        payload = env["payload"]
        _, pk = self._packet(payload["packet"], payload["revision"])
        citation = next((c for c in pk["facts"]["citations"] if c["citation_id"] == payload["citation"]), None)
        # CONTEXT 6: unreadable, nonexistent and foreign citations are refused identically.
        if citation is None:
            raise denied("out_of_scope")
        ref = citation["evidence"]
        if ref.get("provider", self.p.provider_id) != self.p.provider_id:
            raise denied("out_of_scope")
        aid = ref["artifact"]["id"]
        row = self.evidence.load(aid)
        if row is None or not self.evidence.artifact_readable(auth, aid):
            raise denied("out_of_scope")
        arec = row[1]
        if arec["state"] == "sealed" and arec["descriptor"]["digest"] != ref["digest"]:
            raise ProtocolError("artifact_digest_mismatch")
        offset = payload.get("offset", 0)
        size = arec["descriptor"]["size"]
        chunk = b""
        if arec["state"] == "sealed" and self.evidence.availability(aid, arec)["state"] == "available":
            data = self.store.blob(aid) or b""
            chunk = data[offset: offset + payload.get("max_bytes", DEFAULT_EXCERPT_BYTES)] if offset < len(data) else b""
        return self.p.fit_bytes(lambda piece: {"citation": payload["citation"], "evidence": ref, "excerpt": {
            "offset": offset, "length": len(piece), "size": size,
            "data_base64": base64.b64encode(piece).decode("ascii")}}, chunk)

    def _live_items(self, rec: dict) -> list:
        """Item results while preparing: satisfied once the job's content
        satisfies the item, otherwise pending."""
        job = self.load_job(rec["job"])[1]
        ev = self._evaluate(rec, job, "unavailable")
        return [r if r["result"] == "satisfied" else {"item_id": r["item_id"], "obligation": r["obligation"],
                                                     "result": "pending"} for r in ev["items"]]

    def packets_citing(self, aid: str) -> list:
        out = []
        for rid in self.store.ids_of(REQUEST_KIND):
            rec = self.load(rid)[1]
            if any(c["evidence"]["artifact"]["id"] == aid for pk in rec["packets"] for c in pk["facts"]["citations"]):
                out.append(packet_subject(rid))
        return out

    # -------------------------------------------------------- publishing
    def _current_authority(self, rec: dict, job: dict) -> dict:
        current = {e["item_id"]: e["authority_revision"] for e in rec["authority_content"]}
        current.update(job["corrections"])
        return current

    @staticmethod
    def _satisfies(item: dict, section: dict, current_authority: dict) -> bool:
        """CONTEXT 3, 12 "Scripted content and checks": the item's check
        decides (H-CTX-SATISFACTION, restored in H.7 #1)."""
        check = item["check"]
        if check["kind"] == "source_included":
            src = section.get("source")
            return src is not None and src["repository"] == check["repository"] and src["path"] == check["path"]
        if check["kind"] == "evidence_included":
            want = check["evidence"]
            return any(c["evidence"]["artifact"] == want["artifact"] and c["evidence"]["digest"] == want["digest"]
                       for c in section.get("citations", []))
        return (item["item_id"] in current_authority
                and section.get("authority_revision") == current_authority[item["item_id"]])

    def _evaluate(self, rec: dict, job: dict, default_reason: str) -> dict:
        items = rec["items"]
        obligation = {i["item_id"]: i["obligation"] for i in items}
        required = {iid for iid, ob in obligation.items() if ob != "advisory"}
        current_authority = self._current_authority(rec, job)
        omitted_sections = {o["section_id"] for o in job["omissions"] if "section_id" in o}
        claims = rec.get("claims", False)
        candidates, claim_omissions, claim_failures = [], [], []
        for s in job["sections"]:
            if s["section_id"] in omitted_sections:
                continue
            iid = s.get("item_id")
            historical = (iid in job["corrections"] and "authority_revision" in s
                          and s["authority_revision"] < current_authority[iid])  # H-CTX-CORRECTION
            snap = None
            label = s["label"]
            if claims and "claim" in s:
                # CONTEXT 14: a claim is read, and its digest recomputed, when the packet is prepared.
                snap, failure = self.claim_view(rec, s["claim"])
                if snap is None:
                    omission = {"section_id": s["section_id"], "reason": "unavailable"}
                    if iid is not None:
                        omission["item_id"] = iid
                    claim_omissions.append(omission)
                    claim_failures.append((s, failure))
                    continue
                if snap["applicability"]["result"] == "invalid_for_target":
                    historical = True  # only invalid_for_target makes a claim section stale
                rel = snap["reliance"]
                if label == "binding" and not (rel["state"] == "accepted_for_use"
                                               and rel.get("permitted_use") == "binding"):
                    label = "hypothesis"  # labels cannot promote a claim (HM5-LABEL-DEMOTION)
            if historical:
                label = "stale"
            candidates.append((s, historical, label, snap))
        remaining = rec["limits"]["output_capacity"]["amount"] - sum(
            _utf8_len(c[0]["content"]) for c in candidates if c[0].get("item_id") in required)
        included, capacity_items = [], set()
        omissions = [dict(o) for o in job["omissions"]] + claim_omissions
        for candidate in candidates:
            s = candidate[0]
            size = _utf8_len(s["content"])
            if s.get("item_id") in required:
                included.append(candidate)
            elif size <= remaining:
                remaining -= size
                included.append(candidate)
            else:
                omission = {"section_id": s["section_id"], "reason": "output_capacity"}
                if "item_id" in s:
                    omission["item_id"] = s["item_id"]
                    capacity_items.add(s["item_id"])
                omissions.append(omission)
        results = []
        for item in items:
            iid = item["item_id"]
            if claims and item["check"]["kind"] == "claim_included":
                ref = item["check"]["claim"]
                # CONTEXT 14: a section for the item that is not historical carries exactly its
                # reference and meets the table (HM5B-HISTORICAL-CLAIM-REASON).
                outcomes = [self.claim_item_outcome(item, c[3], c[1]) for c in included
                            if c[0].get("item_id") == iid and c[3] is not None
                            and K.Knowledge.same_revision(c[0]["claim"], ref)]
                if None in outcomes:
                    results.append(self._result(item, True, None))
                    continue
                failure = outcomes[0] if outcomes else next(
                    (f for sec, f in claim_failures if sec.get("item_id") == iid
                     and K.Knowledge.same_revision(sec["claim"], ref)), None)
                if failure is not None:
                    # CONTEXT 12 "Scripted unmet reasons": a scripted reason takes precedence
                    # over the claim check's (HM5B-SCRIPTED-UNMET-SCOPE).
                    results.append(self._result(item, False, job["unmet"].get(iid, failure)))
                    continue
            elif any(c[0].get("item_id") == iid and not c[1] and self._satisfies(item, c[0], current_authority)
                     for c in included):
                results.append(self._result(item, True, None))
                continue
            omission_reason = next((o["reason"] for o in job["omissions"] if o.get("item_id") == iid), None)
            if iid in job["unmet"]:
                reason = job["unmet"][iid]
            elif iid in job["corrections"]:
                reason = "corrected_during_preparation"
            elif iid in capacity_items:
                reason = "output_capacity"
            elif omission_reason is not None:
                reason = omission_reason
            else:
                reason = default_reason
            results.append(self._result(item, False, reason))
        if any(r["result"] == "unmet" for r in results):
            state = "unmet"
        elif any(r["result"] == "degraded" for r in results):
            state = "partial"
        else:
            state = "ready"
        return {"included": included, "omissions": omissions, "items": results, "state": state}

    # ------------------------------------------------ CONTEXT 14 claims
    @staticmethod
    def claim_target(basis: dict) -> dict:
        """The Knowledge target equal to a packet's basis (HM5-PACKET-BASIS-TARGET):
        each repository's ID, tree and non-null dirty snapshot, environment,
        build and completeness. ``workspace`` and ``configuration`` have no
        Knowledge counterpart."""
        repos = []
        for r in basis["repositories"]:
            entry = {"id": r["id"], "tree": r["tree"]}
            if r.get("dirty") is not None:
                entry["dirty"] = r["dirty"]
            repos.append(entry)
        target = {"repositories": repos}
        for member in ("environment", "build"):
            if member in basis:
                target[member] = basis[member]
        target["completeness"] = basis["completeness"]
        return target

    def claim_view(self, rec: dict, ref: dict, keep_current: bool = False):
        """The claim snapshot read now from this provider's own knowledge
        store, or ``(None, reason)``."""
        snap, reason = self.p.knowledge.snapshot(ref, self.claim_target(rec["basis"]))
        if snap is not None and not keep_current:
            snap = {k: v for k, v in snap.items() if k != "current_revision"}
        return snap, reason

    @staticmethod
    def claim_item_outcome(item: dict, snap: dict, historical: bool):
        """CONTEXT 14 "What a claim section can satisfy": None when satisfied,
        otherwise the unmet reason. A historical section never satisfies; for
        hypothesis and reference items its reason is invalid_for_target
        (HM5B-HISTORICAL-CLAIM-REASON)."""
        rel = snap["reliance"]
        if item["reliance"] in ("binding", "evidence"):
            if (rel["state"] != "accepted_for_use"
                    or RELIANCE_RANK[rel.get("permitted_use", "reference")] < RELIANCE_RANK[item["reliance"]]):
                return "not_accepted"
            result = snap["applicability"]["result"]
            if result == "invalid_for_target":
                return "invalid_for_target"
            if result != "applicable":
                return "applicability_not_established"
            return None
        if rel["state"] == "rejected":
            return "not_accepted"
        return "invalid_for_target" if historical else None

    def claim_reads(self, rec: dict, facts: dict):
        """CONTEXT 14 "At the read": claim_changes, claim invalidations and
        unverified items for one published revision."""
        views, changes = {}, []
        for sec in facts["sections"]:
            if "claim" not in sec:
                continue
            old = sec["claim"]
            ref = old["reference"]
            snap, reason = self.claim_view(rec, ref, keep_current=True)
            views[sec["section_id"]] = (snap, reason)
            if snap is None:
                changes.append({"section_id": sec["section_id"], "claim": ref, "change": "unavailable"})
                continue
            kinds = []
            if V.canonical(old["reliance"]) != V.canonical(snap["reliance"]):
                kinds.append("reliance_changed")
            if old["applicability"]["result"] != snap["applicability"]["result"]:
                kinds.append("applicability_changed")  # CONTEXT 14: the result only
            known = {c["conflict"] for c in old["conflicts"]}
            if any(c["conflict"] not in known for c in snap["conflicts"]):
                kinds.append("conflict_opened")
            if snap["current_revision"] > ref["revision"]:
                kinds.append("lineage_revised")
            changes.extend({"section_id": sec["section_id"], "claim": ref, "change": k} for k in kinds)
        invalidated, unverified = [], []
        published = {i["item_id"]: i["result"] for i in facts["items"]}
        for item in rec["items"]:
            if item["obligation"] == "advisory" or item["check"]["kind"] != "claim_included":
                continue
            if published.get(item["item_id"]) != "satisfied":
                continue
            ref = item["check"]["claim"]
            sec = next((x for x in facts["sections"] if x.get("item_id") == item["item_id"] and "claim" in x
                        and K.Knowledge.same_revision(x["claim"]["reference"], ref)), None)
            if sec is None:
                continue
            snap, reason = views[sec["section_id"]]
            entry = {"item_id": item["item_id"], "claim": ref}
            if snap is None:
                unverified.append(dict(entry, reason=reason))
                continue
            outcome = self.claim_item_outcome(item, snap, snap["applicability"]["result"] == "invalid_for_target")
            if outcome == "not_accepted":
                invalidated.append(dict(entry, reason="permitted_use_lost"))
            elif outcome == "invalid_for_target":
                invalidated.append(dict(entry, reason="invalid_for_target"))
            elif outcome == "applicability_not_established":
                unverified.append(dict(entry, reason="applicability_not_established"))
        return changes, invalidated, unverified

    def _advisory_waits(self, rec: dict, job: dict) -> bool:
        """CONTEXT 12: a publish whose advisory items are unsatisfied under
        wait_until_deadline waits for the deadline."""
        if rec.get("fallback") != "wait_until_deadline" or self.now() >= rec["limits"]["deadline"]:
            return False
        ev = self._evaluate(rec, job, "unavailable")
        return any(r["result"] == "degraded" for r in ev["items"])

    def _publish(self, rid: str, job_id: str, job: dict, default_reason: str) -> None:
        """Publish the next packet revision for one request (CONTEXT 5, 8).
        Call inside a transaction."""
        revision, rec = self.load(rid)
        ev = self._evaluate(rec, job, default_reason)
        n = len(rec["packets"]) + 1
        subject = request_subject(rid)
        sections_bytes, sections_facts, citations, selected, inclusions = [], [], [], [], []
        seen_citations = set()
        for s in job["sections"]:
            for c in s.get("citations", []):
                if c["citation_id"] not in seen_citations:
                    seen_citations.add(c["citation_id"])
                    citations.append({"citation_id": c["citation_id"], "evidence": c["evidence"]})
        for s, historical, label, snap in ev["included"]:
            cites = [c["citation_id"] for c in s.get("citations", [])]
            body = {"section_id": s["section_id"]}
            fact = {"section_id": s["section_id"]}
            if "item_id" in s:
                body["item_id"] = s["item_id"]
                fact["item_id"] = s["item_id"]
            body.update(label=label, historical=historical, content=s["content"], citations=cites)
            fact.update(label=label, length=_utf8_len(s["content"]), citations=cites, historical=historical)
            if snap is not None:
                body["claim"] = snap
                fact["claim"] = snap
            sections_bytes.append(body)
            sections_facts.append(fact)
            inclusions.append(s["section_id"])
            if "source" in s:
                entry = {k: s["source"][k] for k in ("repository", "path", "tree") if k in s["source"]}
                if entry not in selected:
                    selected.append(entry)
            for c in s.get("citations", []):
                entry = {"evidence": c["evidence"]}
                if entry not in selected:
                    selected.append(entry)
        current_authority = self._current_authority(rec, job)
        # H3-AUTHORITY-ENTRIES (fixture-informed): one entry per authority
        # content item, at the item's current authority revision, whether or
        # not content at that revision is included.
        authority = [{"item_id": e["item_id"], "authority_revision": current_authority[e["item_id"]],
                      "evidence": e["evidence"], "coverage": list(job["coverage"])} for e in rec["authority_content"]]
        applicability = {"basis": rec["basis"], "conditions": list(job["conditions"])}
        content = {"format": CLAIMS_PACKET_FORMAT if rec.get("claims") else PACKET_FORMAT, "request": subject,
                   "revision": n}
        if n > 1:
            content["supersedes"] = {"revision": n - 1}
        content.update(sections=sections_bytes, items=ev["items"], coverage=list(job["coverage"]),
                       omissions=ev["omissions"], applicability=applicability, authority=authority)
        data = V.canonical(content)
        aid = f"packet.{rid}.{n}"  # H-CTX-IDS
        gaps = [g for c in job["coverage"] for g in c["gaps"]][:256]
        digest = self.evidence.seal_internal(
            aid, data, PACKET_MEDIA_TYPE, {"kind": PACKET_KIND, "id": rid}, "context",
            {"completeness": "complete" if ev["state"] == "ready" else "partial", "gaps": gaps})
        reference = {"packet": packet_subject(rid), "revision": n,
                     "artifact": {"provider": self.p.provider_id, "artifact": EVD.artifact_subject(aid),
                                  "digest": digest}}
        facts = {"reference": reference, "request": subject, "job": job_subject(job_id)}
        if n > 1:
            facts["supersedes"] = {"revision": n - 1}
        facts.update(selected=selected, provenance={"compiler": COMPILER, "job": job_subject(job_id),
                                                    "basis": rec["basis"]},
                     sections=sections_facts, inclusions=inclusions, omissions=ev["omissions"],
                     citations=citations, coverage=list(job["coverage"]), items=ev["items"],
                     applicability=applicability, authority=authority)
        packet = {"revision": n, "reference": reference, "facts": facts}
        if n > 1:
            packet["supersedes"] = {"revision": n - 1}
        rec["packets"].append(packet)
        rec["item_results"] = ev["items"]
        old_state = rec["state"]
        rec["state"] = ev["state"]
        # H3-PUBLISH-REVISION (fixture-informed): one publication is one change
        # of the request; its events share the new revision.
        revision += 1
        self._emit(None, "context.packet.published", subject, revision, {"reference": reference, "items": ev["items"]})
        if ev["state"] != old_state:
            self._emit(None, "context.request.changed", subject, revision, {"state": ev["state"], "job": job_subject(job_id)})
        self.store.put_json(REQUEST_KIND, rid, revision, rec)
        self.store.put_json(PACKET_KIND, rid, n, {"request": rid})

    def _end_job(self, jid: str, job: dict, reason: str) -> None:
        for rid in job["requests"]:
            if self.load(rid)[1]["state"] == "preparing":
                self._publish(rid, jid, job, reason)
        job["ended"] = reason
        revision = self.store.revision(JOB_KIND, jid) + 1
        self.store.put_json(JOB_KIND, jid, revision, job)
        self._emit(None, "context.job.ended", job_subject(jid), revision, {"reason": reason})

    # ------------------------------------------------------------- ticks
    def _in_tx(self, fn) -> bool:
        st = self.store
        st.begin()
        try:
            progressed = fn()
            if progressed:
                st.commit()
            else:
                st.rollback()
            return progressed
        except BaseException:
            st.rollback()
            raise

    def tick(self) -> bool:
        progressed = False
        now = self.now()
        for rid in self.store.ids_of(REQUEST_KIND):
            rec = self.load(rid)[1]
            if rec["state"] == "preparing" and now >= rec["limits"]["deadline"]:
                progressed = self._in_tx(lambda rid=rid, rec=rec: self._deadline(rid, rec)) or progressed
        for jid in self.store.ids_of(JOB_KIND):
            while self._in_tx(lambda jid=jid: self._job_step(jid)):
                progressed = True
        return progressed

    def _deadline(self, rid: str, rec: dict) -> bool:
        """CONTEXT 5 (CTX-7): at the deadline, unsupplied items are unmet (H-CTX-DEADLINE)."""
        jid = rec["job"]
        jrev, job = self.load_job(jid)
        self._publish(rid, jid, job, "deadline_passed")
        # H3-DEADLINE-PUBLISH (fixture-informed): a publication at the
        # deadline is the one the pending script step was waiting for; that
        # step does not publish an update for this request again.
        revision, rec = self.load(rid)
        rec["published_pos"] = job["pos"]
        self.store.put_json(REQUEST_KIND, rid, revision, rec)
        return True

    def _job_step(self, jid: str) -> bool:
        jrev, job = self.load_job(jid)
        if job["ended"] is not None or job["stalled"]:
            return False
        script = self.script_for(job["owner"])
        pos = job["pos"]
        if pos >= len(script):
            return False
        (key, val), = script[pos].items()
        if key == "wait_until":
            if self.now() < val:
                return False
        elif key == "investigate":
            if job["spent"] + val > job["budget"]:
                job["pos"] = pos + 1
                self._end_job(jid, job, "investigation_budget_exhausted")  # H-CTX-INVESTIGATE
                return True
            job["spent"] += val
        elif key == "section":
            job["sections"].append(dict(val))
        elif key == "coverage":
            job["coverage"].append(dict(val))
        elif key == "unmet":
            job["unmet"][val["item_id"]] = val["reason"]
        elif key == "omit":
            job["omissions"].append(dict(val))
        elif key == "correction":
            job["corrections"][val["item_id"]] = val["authority_revision"]
        elif key == "conditions":
            job["conditions"].extend(dict(c) for c in val)
        elif key == "publish":
            subscribers = [(rid, self.load(rid)[1]) for rid in job["requests"]]
            if any(rec["state"] == "preparing" and self._advisory_waits(rec, job) for _, rec in subscribers):
                return False
            job["pos"] = pos + 1
            for rid, rec in subscribers:
                if rec["state"] == "preparing" or (rec["packets"] and rec["updates"]
                                                   and rec["state"] in ("ready", "partial", "unmet")
                                                   and rec.get("published_pos", -1) < pos):
                    self._publish(rid, jid, job, "unavailable")
            job["published"] = True
            self.store.put_json(JOB_KIND, jid, jrev, job)
            return True
        elif key == "end":
            job["pos"] = pos + 1
            self._end_job(jid, job, val)
            return True
        elif key == "stall":
            job["stalled"] = True
        job["pos"] = pos + 1
        self.store.put_json(JOB_KIND, jid, jrev, job)
        return True
