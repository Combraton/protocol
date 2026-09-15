"""The verification/1 profile over a scripted evaluator (VERIFICATION.md, proposed M5 draft).

Written from VERIFICATION.md, CORE.md, EVIDENCE.md, the conformance README's
``verifier`` launch control and schemas/verification/1 only.

Durable records are ordinary subjects in the provider's SQLite store:

- ``verification.job``: contract, subjects, requested environment, pinned
  evaluator, the contract's roles and properties as read at submission,
  state, recorded property results, observed anchors, script position and,
  once completed, the receipt reference.
- ``verification.receipt``: the immutable mapping from the receipt subject to
  the sealed artifact ``receipt.<id>`` (VERIFICATION 2, 5). The content is
  read back from the sealed bytes.

Contracts are read from this provider's own evidence store. Section H.M5 of
DIVERGENCES.md, with its realignment H.M5b, records the choices the documents
leave open; the ``HM5-`` and ``HM5B-`` tags below refer to it.
"""

from __future__ import annotations

import re

import clock as C
import envelope as E
import evidence as EVD
import knowledge as K
import valuedomain as V
from envelope import Invalid, ptr
from errors import ProtocolError

JOB_KIND = "verification.job"
RECEIPT_KIND = "verification.receipt"
CONTRACT_FORMAT = "combraton-verification-contract/1"
RECEIPT_FORMAT = "combraton-verification-receipt/1"
RECEIPT_MEDIA_TYPE = "application/vnd.combraton.verification-receipt+json"
RESULTS = ("pass", "fail", "not_evaluated", "indeterminate")
REASONED = ("not_evaluated", "indeterminate")
LAYERS = ("static", "component", "integration", "journey", "runtime_path", "qualitative")
EVALUATOR_ID = r"[a-z][a-z0-9_-]{0,63}"
EVALUATOR_VERSION = r"[a-z0-9_-]{1,32}"
STEP_KEYS = ("wait_until", "property", "observe", "valid_until", "complete")

ID_RE = re.compile(EVALUATOR_ID)
VERSION_RE = re.compile(EVALUATOR_VERSION)


class VerifierConfigError(Exception):
    pass


def _fail(msg: str):
    raise VerifierConfigError(msg)


def predicate_name(evaluator: dict) -> str:
    """VERIFICATION 4 "Evaluator capability"."""
    return f"verification.evaluator.{evaluator['id']}.v{evaluator['version']}"


def _validate_step(step, what: str) -> None:
    if not isinstance(step, dict) or len(step) != 1:
        _fail(f"{what} must be an object with exactly one member")
    (key, val), = step.items()
    if key not in STEP_KEYS:
        _fail(f"{what}: unknown verifier script step {key!r}")
    if key in ("wait_until", "valid_until"):
        if not C.valid_instant(val):
            _fail(f"{what}.{key} must be an instant")
    elif key == "property":
        if not isinstance(val, dict) or set(val) - {"property_id", "result", "reason"} or \
                not {"property_id", "result"} <= set(val):
            _fail(f"{what}.property is {{ property_id, result, reason? }}")
        if not isinstance(val["property_id"], str) or val["result"] not in RESULTS:
            _fail(f"{what}.property has an ill-typed property_id or result")
        if "reason" in val and (not isinstance(val["reason"], str) or not 1 <= len(val["reason"]) <= 256):
            _fail(f"{what}.property.reason must be a string of 1-256 characters")
        if (val["result"] in REASONED) != ("reason" in val):
            # VERIFICATION 11 and the launch schema: reason exactly for not_evaluated and indeterminate.
            _fail(f"{what}.property.reason is required exactly for not_evaluated and indeterminate")
    elif key == "observe":
        if not isinstance(val, dict) or set(val) != {"anchors"} or not isinstance(val["anchors"], dict) \
                or not all(isinstance(v, str) for v in val["anchors"].values()):
            _fail(f"{what}.observe is {{ anchors: {{ name: value }} }}")
    elif key == "complete":
        if val is not True:
            _fail(f"{what}.complete must be true")


def parse_verifier_config(raw) -> dict:
    if not isinstance(raw, dict):
        _fail("verifier must be an object")
    unknown = set(raw) - {"evaluators", "scripts", "default_script"}
    if unknown:
        _fail(f"verifier: unknown members {sorted(unknown)}")
    evaluators = raw.get("evaluators", [])
    if not isinstance(evaluators, list):
        _fail("verifier.evaluators must be an array")
    seen = set()
    for idx, ev in enumerate(evaluators):
        if not isinstance(ev, dict) or set(ev) - {"id", "version", "status"} or not {"id", "version"} <= set(ev):
            _fail(f"verifier.evaluators[{idx}] is {{ id, version, status? }}")
        if not isinstance(ev["id"], str) or not ID_RE.fullmatch(ev["id"]):
            _fail(f"verifier.evaluators[{idx}].id must match {EVALUATOR_ID}")
        if not isinstance(ev["version"], str) or not VERSION_RE.fullmatch(ev["version"]):
            _fail(f"verifier.evaluators[{idx}].version must match {EVALUATOR_VERSION}")
        if ev.get("status", "supported") not in ("supported", "unsupported", "unknown"):
            _fail(f"verifier.evaluators[{idx}].status must be supported, unsupported or unknown")
        name = predicate_name(ev)
        if name in seen:
            _fail(f"verifier.evaluators lists {name} twice")
        seen.add(name)
    scripts = raw.get("scripts", {})
    if not isinstance(scripts, dict):
        _fail("verifier.scripts must be an object")
    for jid, script in scripts.items():
        if not isinstance(script, list):
            _fail(f"verifier.scripts.{jid} must be an array")
        for idx, step in enumerate(script):
            _validate_step(step, f"verifier.scripts.{jid}[{idx}]")
    default = raw.get("default_script")
    if default is not None:
        if not isinstance(default, list):
            _fail("verifier.default_script must be an array")
        for idx, step in enumerate(default):
            _validate_step(step, f"verifier.default_script[{idx}]")
    return raw


def job_subject(jid: str) -> dict:
    return {"kind": JOB_KIND, "id": jid}


def receipt_subject(rid: str) -> dict:
    return {"kind": RECEIPT_KIND, "id": rid}


def parse_contract(data: bytes):
    """VERIFICATION 3: the contract content, or None when it is not a valid
    contract (HM5-CONTRACT-FORMAT)."""
    try:
        doc = V.loads(data.decode("utf-8"))
        E.closed(doc, "", ("format", "outcome", "subjects", "properties", "environment", "evaluators", "freshness"))
        if doc["format"] != CONTRACT_FORMAT:
            return None  # VERIFICATION 3: format is required
        E.string(doc["outcome"], "/outcome")
        roles = set()
        for idx, s in enumerate(E.array(doc["subjects"], "/subjects", 1)):
            E.closed(s, ptr("/subjects", idx), ("role", "kind"))
            E.identifier(s["role"], ptr(ptr("/subjects", idx), "role"))
            E.string(s["kind"], ptr(ptr("/subjects", idx), "kind"), 1)
            if s["role"] in roles:
                return None
            roles.add(s["role"])
        ids = set()
        for idx, prop in enumerate(E.array(doc["properties"], "/properties", 1)):
            p = ptr("/properties", idx)
            E.closed(prop, p, ("property_id", "layer", "required", "statement"))
            E.identifier(prop["property_id"], ptr(p, "property_id"))
            if prop["layer"] not in LAYERS or not isinstance(prop["required"], bool):
                return None
            E.string(prop["statement"], ptr(p, "statement"))
            if prop["property_id"] in ids:
                return None
            ids.add(prop["property_id"])
        env = E.closed(doc["environment"], "/environment", ("required_anchors",))
        for idx, a in enumerate(E.array(env["required_anchors"], "/environment/required_anchors")):
            E.string(a, ptr("/environment/required_anchors", idx), 1)
        if doc["evaluators"] is not None:
            for idx, a in enumerate(E.array(doc["evaluators"], "/evaluators")):
                E.string(a, ptr("/evaluators", idx), 1)
        if doc["freshness"] is not None:
            f = E.closed(doc["freshness"], "/freshness", ("max_age_seconds",))
            E.integer(f["max_age_seconds"], "/freshness/max_age_seconds")
    except (UnicodeDecodeError, V.ParseError, Invalid):
        return None
    return doc


def _subject_entries(v, path: str, with_subject: bool) -> None:
    E.array(v, path, 1, 32)
    for idx, entry in enumerate(v):
        p = ptr(path, idx)
        E.closed(entry, p, ("role", "digest"), ("subject",) if with_subject else ())
        E.identifier(entry["role"], ptr(p, "role"))
        E.string(entry["digest"], ptr(p, "digest"), 1, 256)
        if "subject" in entry:
            E.subject(entry["subject"], ptr(p, "subject"))


def _environment(v, path: str) -> None:
    env = E.closed(v, path, ("anchors",))
    anchors = env["anchors"]
    if not isinstance(anchors, dict) or len(anchors) > 32:
        raise Invalid(ptr(path, "anchors"), "must be an object with at most 32 members")
    for name, value in anchors.items():
        E.string(name, ptr(ptr(path, "anchors"), name), 1, 64)
        E.string(value, ptr(ptr(path, "anchors"), name), 1, 256)


def _evaluator(v, path: str) -> None:
    E.closed(v, path, ("id", "version"))
    E.pattern(v["id"], ptr(path, "id"), ID_RE, "evaluator ID")
    E.pattern(v["version"], ptr(path, "version"), VERSION_RE, "evaluator version")


def listing_problems(names: list, expected: list) -> dict:
    """missing, duplicated and unknown names against the contract's."""
    seen, duplicated = set(), []
    for n in names:
        if n in seen and n not in duplicated:
            duplicated.append(n)
        seen.add(n)
    missing = [n for n in expected if n not in seen]
    unknown = list(dict.fromkeys(n for n in names if n not in expected))
    # VERIFICATION 4, 5: all three lists, empty ones included (HM5-LISTING-DETAILS, resolved).
    return {"missing": missing, "duplicated": duplicated, "unknown": unknown}


def _refuse_listing(path: str, problems: dict) -> None:
    if any(problems.values()):
        # HM5-LISTING-DETAILS: the details name the problem kinds with their names.
        details = {"path": path, "reason": "the listing does not match the contract"}
        details.update(problems)
        raise ProtocolError("invalid_envelope", details)


class Verification:
    def __init__(self, provider, cfg: dict):
        self.p = provider
        self.evaluators = cfg.get("evaluators", [])
        self.scripts = cfg.get("scripts", {})
        self.default_script = cfg.get("default_script")

    @property
    def store(self):
        return self.p.store

    @property
    def evidence(self):
        return self.p.evidence

    def now(self) -> str:
        return self.p.clock.now()

    def predicates(self) -> list:
        return [{"name": predicate_name(ev), "status": ev.get("status", "supported"),
                 "evidence": {"source": "conformance launch configuration (verifier)"}} for ev in self.evaluators]

    def predicate_status(self, evaluator: dict) -> str:
        _, predicates = self.store.capabilities()
        name = predicate_name(evaluator)
        # VERIFICATION 4 step 1: an unlisted version has no evidence of support, so unknown (CORE 17.1).
        return next((pr["status"] for pr in predicates if pr["name"] == name), "unknown")

    def _emit(self, changes, etype: str, subject: dict, revision: int, payload: dict) -> None:
        if changes is not None:
            changes.append((etype, subject, revision, payload))
        else:
            self.store.append_event({"type": etype, "subject": subject, "revision": revision, "origin": "provider",
                                     "caused_by": [], "recorded_at": self.now(), "payload": payload})

    # ------------------------------------------------------- validators
    @staticmethod
    def _create_precondition(env: dict) -> None:
        if len(env["preconditions"]) != 1:
            raise Invalid("/preconditions", "exactly one precondition is required")
        if env["preconditions"][0]["subject"] != env["subject"]:
            raise Invalid("/preconditions/0/subject", "the precondition must name the command's subject")
        if env["preconditions"][0]["revision"] != 0:
            raise Invalid("/preconditions/0/revision", "the command creates the subject: revision must be 0")

    def v_evaluate(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", JOB_KIND)
        self._create_precondition(env)
        payload = E.closed(env["payload"], "/payload", ("contract", "subjects", "evaluator"), ("environment",))
        K.evidence_reference(payload["contract"], "/payload/contract")
        _subject_entries(payload["subjects"], "/payload/subjects", True)
        if "environment" in payload:
            _environment(payload["environment"], "/payload/environment")
        _evaluator(payload["evaluator"], "/payload/evaluator")

    def v_id_query(self, member: str):
        def validator(env, method, features) -> None:
            E.query_envelope(env, method, "core.grants" in features)
            payload = E.closed(env["payload"], "/payload", (member,))
            E.identifier(payload[member], ptr("/payload", member))
        return validator

    def v_record(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", RECEIPT_KIND)
        self._create_precondition(env)
        payload = E.closed(env["payload"], "/payload",
                           ("subjects", "contract", "evaluator", "environment", "inputs", "properties",
                            "observed_from", "observed_until", "valid_until", "scope", "provenance"))
        _subject_entries(payload["subjects"], "/payload/subjects", True)
        K.evidence_reference(payload["contract"], "/payload/contract")
        _evaluator(payload["evaluator"], "/payload/evaluator")
        _environment(payload["environment"], "/payload/environment")
        E.array(payload["inputs"], "/payload/inputs", max_items=32)
        for idx, ref in enumerate(payload["inputs"]):
            K.evidence_reference(ref, ptr("/payload/inputs", idx))
        props = E.array(payload["properties"], "/payload/properties", 1, 128)
        for idx, prop in enumerate(props):
            p = ptr("/payload/properties", idx)
            E.closed(prop, p, ("property_id", "result", "evidence"), ("reason",))
            E.identifier(prop["property_id"], ptr(p, "property_id"))
            if prop["result"] not in RESULTS:
                raise Invalid(ptr(p, "result"), "must be pass, fail, not_evaluated or indeterminate")
            if "reason" in prop:
                E.string(prop["reason"], ptr(p, "reason"), 1, 256)
            E.array(prop["evidence"], ptr(p, "evidence"), max_items=32)
            for j, ref in enumerate(prop["evidence"]):
                K.evidence_reference(ref, ptr(ptr(p, "evidence"), j))
        E.instant(payload["observed_from"], "/payload/observed_from")
        E.instant(payload["observed_until"], "/payload/observed_until")
        if payload["valid_until"] is not None:
            E.instant(payload["valid_until"], "/payload/valid_until")
        E.string(payload["scope"], "/payload/scope", 1, 1024)
        E.array(payload["provenance"], "/payload/provenance", max_items=32)
        for idx, item in enumerate(payload["provenance"]):
            p = ptr("/payload/provenance", idx)
            if isinstance(item, dict) and "kind" in item:
                E.subject(item, p)
            else:
                K.evidence_reference(item, p)
        # VERIFICATION 5 "Complete listing": a reason exactly when required (HM5-REASON-STEP).
        for idx, prop in enumerate(props):
            if (prop["result"] in REASONED) != ("reason" in prop):
                raise Invalid(ptr(ptr("/payload/properties", idx), "reason"),
                              "reason is required exactly for not_evaluated and indeterminate")
        if payload["observed_from"] > payload["observed_until"]:
            raise Invalid("/payload/observed_until", "observed_from must not be after observed_until")  # HM5-OBSERVED-ORDER

    def v_assess(self, env, method, features) -> None:
        E.query_envelope(env, method, "core.grants" in features)
        payload = E.closed(env["payload"], "/payload", ("receipt", "contract", "subjects"), ("environment", "at"))
        K.receipt_reference(payload["receipt"], "/payload/receipt")
        K.evidence_reference(payload["contract"], "/payload/contract")
        _subject_entries(payload["subjects"], "/payload/subjects", False)
        if "environment" in payload:
            _environment(payload["environment"], "/payload/environment")
        if "at" in payload:
            E.instant(payload["at"], "/payload/at")

    # ---------------------------------------------------- authorization
    def authorize(self, method: str, env: dict):
        p = self.p
        payload = env["payload"]
        if method == "verification.evaluate_contract":
            return p.authorize_under(env, [("verification.evaluate", env["subject"]),
                                           ("evidence.read", payload["contract"]["artifact"])])
        if method == "verification.receipt.record":
            return p.authorize_under(env, [("verification.record", env["subject"]),
                                           ("evidence.read", payload["contract"]["artifact"])])
        if method == "verification.job.inspect":
            return p.authorize_under(env, [("verification.read", job_subject(payload["job"]))])
        if method == "verification.receipt.inspect":
            return p.authorize_under(env, [("verification.read", receipt_subject(payload["receipt"]))])
        if method == "verification.receipt.assess":
            return p.authorize_under(env, [("verification.read", receipt_subject(payload["receipt"]["receipt"])),
                                           ("evidence.read", payload["contract"]["artifact"])])
        raise AssertionError(method)

    # -------------------------------------------------------- contracts
    def read_contract(self, ref: dict):
        """VERIFICATION 3: ``(contract, None)``, ``(None, reason)`` for
        ``contract_unavailable``, or ``(None, "artifact_digest_mismatch")``."""
        if ref["provider"] != self.p.provider_id:
            return None, "unreachable"
        aid = ref["artifact"]["id"]
        row = self.evidence.load(aid)
        if row is None:
            return None, "not_found"
        rec = row[1]
        if rec["state"] != "sealed":
            return None, "not_sealed"
        if self.evidence.availability(aid, rec)["state"] != "available":
            return None, "unavailable"
        if rec["descriptor"]["digest"] != ref["digest"]:
            return None, "artifact_digest_mismatch"
        contract = parse_contract(self.store.blob(aid) or b"")
        if contract is None:
            return None, "invalid_format"
        return contract, None

    def contract_or_refuse(self, ref: dict) -> dict:
        contract, reason = self.read_contract(ref)
        if reason == "artifact_digest_mismatch":
            raise ProtocolError("artifact_digest_mismatch")
        if reason is not None:
            raise ProtocolError("contract_unavailable", {"reason": reason})
        return contract

    # --------------------------------------------------------- commands
    def _id_taken(self, auth, kind: str, sid: str) -> None:
        """VERIFICATION 4 "ID shared with receipts" (HM5B-COLLISION-DETAILS)."""
        current = self.store.revision(kind, sid)
        if current:
            item = {"subject": {"kind": kind, "id": sid}, "expected": 0}
            if auth.may_read(item["subject"]):
                item["current"] = current
            raise ProtocolError("precondition_failed", {"failed": [item]})

    def op_evaluate(self, env, auth):
        payload = env["payload"]
        evaluator = payload["evaluator"]
        # VERIFICATION 4 "Order at step 7" (CORE 10, 17): evaluator capability, Core
        # preconditions, ID collision, contract, roles.
        status = self.predicate_status(evaluator)
        if status != "supported":
            # Never another version.
            raise ProtocolError("capability_unavailable", {"capability": predicate_name(evaluator), "status": status})
        self.p.check_preconditions(env, auth)
        self._id_taken(auth, RECEIPT_KIND, env["subject"]["id"])
        contract = self.contract_or_refuse(payload["contract"])
        roles = [s["role"] for s in contract["subjects"]]
        _refuse_listing("/payload/subjects", listing_problems([s["role"] for s in payload["subjects"]], roles))
        jid = env["subject"]["id"]
        rec = {"contract": payload["contract"], "subjects": payload["subjects"], "evaluator": evaluator,
               "requested_environment": payload.get("environment"), "roles": roles,
               "properties": [{"property_id": pr["property_id"], "required": pr["required"]}
                              for pr in contract["properties"]],
               "outcome": contract["outcome"],
               "state": "queued", "recorded": [], "anchors": {}, "valid_until": None, "pos": 0,
               "submitted_at": self.now(), "started_at": None}
        self.store.put_json(JOB_KIND, jid, 1, rec)
        return 1, {"job": job_subject(jid), "state": "queued"}, [
            ("verification.job.changed", job_subject(jid), 1, {"state": "queued", "evaluator": evaluator})]

    def op_record(self, env, auth):
        self.p.check_preconditions(env, auth)
        self._id_taken(auth, JOB_KIND, env["subject"]["id"])  # VERIFICATION 4 "ID shared with receipts"
        payload = env["payload"]
        contract = self.contract_or_refuse(payload["contract"])
        _refuse_listing("/payload/subjects", listing_problems([s["role"] for s in payload["subjects"]],
                                                             [s["role"] for s in contract["subjects"]]))
        _refuse_listing("/payload/properties", listing_problems([pr["property_id"] for pr in payload["properties"]],
                                                                [pr["property_id"] for pr in contract["properties"]]))
        rid = env["subject"]["id"]
        content = {"format": RECEIPT_FORMAT, "receipt": rid, "subjects": payload["subjects"],
                   "contract": payload["contract"],
                   "evaluator": dict(payload["evaluator"], principal=self.p.principal),
                   "environment": payload["environment"], "inputs": payload["inputs"],
                   "properties": payload["properties"], "observed_from": payload["observed_from"],
                   "observed_until": payload["observed_until"], "valid_until": payload["valid_until"],
                   "scope": payload["scope"], "job": None, "provenance": payload["provenance"]}
        changes = []
        reference = self._seal_receipt(rid, content, None, changes)
        # CORE 16.3: the primary subject's event first.
        changes.insert(0, ("verification.receipt.recorded", receipt_subject(rid), 1, {"reference": reference}))
        return 1, {"reference": reference}, changes

    def _seal_receipt(self, rid: str, content: dict, jid, changes) -> dict:
        aid = f"receipt.{rid}"  # VERIFICATION 5 "Artifact convention"
        digest = self.evidence.seal_internal(aid, V.canonical(content), RECEIPT_MEDIA_TYPE, receipt_subject(rid),
                                             "verification", {"completeness": "complete", "gaps": []},
                                             changes=changes)
        reference = {"provider": self.p.provider_id, "receipt": rid,
                     "artifact": {"provider": self.p.provider_id, "artifact": EVD.artifact_subject(aid),
                                  "digest": digest}}
        self.store.put_json(RECEIPT_KIND, rid, 1, {"reference": reference, "job": jid})
        return reference

    # ---------------------------------------------------------- queries
    def op_job_inspect(self, env, auth) -> dict:
        jid = env["payload"]["job"]
        row = self.store.get_json(JOB_KIND, jid)
        if row is None:
            raise ProtocolError("not_found")
        rec = row[1]
        out = {"job": job_subject(jid), "state": rec["state"], "contract": rec["contract"],
               "subjects": rec["subjects"], "evaluator": rec["evaluator"], "recorded": rec["recorded"]}
        if rec.get("receipt") is not None:
            out["receipt"] = rec["receipt"]
        return out

    def receipt_content(self, rid: str):
        """The receipt's sealed bytes, parsed, or None when they cannot be
        read or fail integrity."""
        row = self.store.get_json(RECEIPT_KIND, rid)
        if row is None:
            return None
        aid = row[1]["reference"]["artifact"]["artifact"]["id"]
        arow = self.evidence.load(aid)
        if arow is None or arow[1]["state"] != "sealed" or self.evidence.availability(aid, arow[1])["state"] != "available":
            return None
        try:
            return V.loads((self.store.blob(aid) or b"").decode("utf-8"))
        except (UnicodeDecodeError, V.ParseError):
            return None

    def op_receipt_inspect(self, env, auth) -> dict:
        rid = env["payload"]["receipt"]
        row = self.store.get_json(RECEIPT_KIND, rid)
        if row is None:
            raise ProtocolError("not_found")
        content = self.receipt_content(rid)
        if content is None:
            raise ProtocolError("unavailable", {}, "the receipt's sealed bytes cannot be read")  # HM5-RECEIPT-UNREADABLE
        return {"reference": row[1]["reference"], "content": content}

    def op_assess(self, env, auth) -> dict:
        payload = env["payload"]
        ref = payload["receipt"]
        row = self.store.get_json(RECEIPT_KIND, ref["receipt"]) if ref["provider"] == self.p.provider_id else None
        if row is None:
            # VERIFICATION 7 "Authorization": after step 6, a nonexistent receipt is not_found for an
            # authorized reader; an unauthorized one was already refused identically for both.
            raise ProtocolError("not_found")
        contract, contract_reason = self.read_contract(payload["contract"])
        if contract is not None:
            _refuse_listing("/payload/subjects", listing_problems([s["role"] for s in payload["subjects"]],
                                                                 [s["role"] for s in contract["subjects"]]))
        at = payload.get("at")
        assessed_at = at if at is not None else self.now()
        checks = {name: {"status": "passed", "reasons": []}
                  for name in ("receipt", "contract", "subjects", "environment", "evaluator", "time")}

        def fail(name, reason, status="failed"):
            c = checks[name]
            if reason not in c["reasons"]:
                c["reasons"].append(reason)
            if status == "failed" or c["status"] == "passed":
                c["status"] = status

        mapped = row[1]["reference"]["artifact"]
        if V.canonical(ref["artifact"]) != V.canonical(mapped):
            # VERIFICATION 7: a reference differing from its mapping makes the receipt unusable,
            # so no property result is presented from the mapped bytes.
            fail("receipt", "receipt_reference_mismatch")
            content = None
        else:
            content = self.receipt_content(ref["receipt"])
            if content is None:
                fail("receipt", "receipt_unavailable", "unverifiable")
        if contract is None:
            # VERIFICATION 7 table: the contract cannot be read.
            for name in ("contract", "environment", "evaluator"):
                fail(name, "contract_unavailable", "unverifiable")
        if content is None:
            # VERIFICATION 7 table: the receipt cannot be used (HM5B-UNUSABLE-REASON-ORDER).
            for name in ("contract", "subjects", "environment", "evaluator", "time"):
                fail(name, checks["receipt"]["reasons"][0], "unverifiable")
        else:
            if V.canonical(content["contract"]) != V.canonical(payload["contract"]):
                fail("contract", "contract_mismatch")
            by_role = {s["role"]: s["digest"] for s in content["subjects"]}
            if any(by_role.get(s["role"]) != s["digest"] for s in payload["subjects"]):
                fail("subjects", "subject_mismatch")
            if contract is not None:
                requested = (payload.get("environment") or {}).get("anchors", {})
                observed = content["environment"]["anchors"]
                for anchor in contract["environment"]["required_anchors"]:
                    if anchor not in requested or anchor not in observed:
                        fail("environment", "environment_unverified", "unverifiable")
                    elif requested[anchor] != observed[anchor]:
                        fail("environment", "environment_mismatch")
                if contract["evaluators"] is not None and content["evaluator"]["id"] not in contract["evaluators"]:
                    fail("evaluator", "evaluator_not_permitted")
            if assessed_at < content["observed_until"]:
                fail("time", "before_observation")
            stale = content["valid_until"] is not None and assessed_at >= content["valid_until"]
            if contract is not None and contract["freshness"] is not None:
                limit = C.add_seconds(content["observed_until"], contract["freshness"]["max_age_seconds"])
                stale = stale or assessed_at > limit
            if stale:
                fail("time", "stale")
        blocking = [r for c in checks.values() if c["status"] != "passed" for r in c["reasons"]]
        blocking = list(dict.fromkeys(blocking))
        properties = []
        if contract is not None and content is not None:
            results = {pr["property_id"]: pr for pr in content["properties"]}
            for prop in contract["properties"]:
                entry = results.get(prop["property_id"])
                if entry is None:
                    continue
                result = entry["result"]
                if blocking:
                    status, reasons = "not_satisfied", list(blocking)
                elif result == "pass":
                    status, reasons = "satisfied", []
                elif result == "fail":
                    status, reasons = "failed", []
                else:
                    status, reasons = "not_satisfied", [result]
                properties.append({"property_id": prop["property_id"], "required": prop["required"],
                                   "result": result, "status": status, "reasons": reasons})
        required = [pr for pr in properties if pr["required"]]
        # VERIFICATION 7 "Overall".
        if blocking or not properties:
            overall = "not_satisfied"
        elif any(pr["status"] == "failed" for pr in required):
            overall = "failed"
        elif any(pr["status"] != "satisfied" for pr in required):
            overall = "not_satisfied"
        else:
            overall = "satisfied"
        time_basis = "provider_clock" if at is None else "caller_selected"
        return {"assessed_at": assessed_at, "time_basis": time_basis,
                "present_validity": "established" if time_basis == "provider_clock" and overall == "satisfied"
                else "not_established",
                "basis": {"subjects": payload["subjects"], "environment": payload.get("environment")},
                "checks": checks, "properties": properties, "overall": overall, "reasons": blocking}

    # ------------------------------------------------------------ ticks
    def script_for(self, jid: str):
        if jid in self.scripts:
            return self.scripts[jid]
        return self.default_script

    def tick(self) -> bool:
        progressed = False
        for jid in self.store.ids_of(JOB_KIND):
            while self._in_tx(lambda jid=jid: self._job_step(jid)):
                progressed = True
        return progressed

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

    def _job_event(self, jid: str, revision: int, etype: str, payload: dict) -> int:
        revision += 1
        self._emit(None, etype, job_subject(jid), revision, payload)
        return revision

    def _record_result(self, jid: str, revision: int, rec: dict, property_id: str, result: str, reason) -> int:
        entry = {"property_id": property_id, "result": result}
        if reason is not None:
            entry["reason"] = reason
        rec["recorded"].append(entry)
        return self._job_event(jid, revision, "verification.property.recorded", dict(entry))

    def _job_step(self, jid: str) -> bool:
        revision, rec = self.store.get_json(JOB_KIND, jid)
        if rec["state"] == "completed":
            return False
        if self.predicate_status(rec["evaluator"]) != "supported":
            # VERIFICATION 4 "Losing the pinned evaluator"; a queued job never ran and goes
            # straight to completed, with observed_from equal to observed_until (HM5C-QUEUED-LOSS-EVENTS).
            return self._complete(jid, revision, rec, "indeterminate", "evaluator_unavailable")
        if rec["state"] == "queued":
            # VERIFICATION 11: a job stays queued while its script waits at a
            # leading wait_until; its first evaluation is the first other step
            # (HM6-QUEUED-WAIT-STEPS). A script left with no other step makes
            # the job running once the waits pass (HM6-WAIT-ONLY-SCRIPT).
            script = self.script_for(jid)
            if script is not None and rec["pos"] < len(script) and "wait_until" in script[rec["pos"]]:
                if self.now() < script[rec["pos"]]["wait_until"]:
                    return False
                rec["pos"] += 1
                self.store.put_json(JOB_KIND, jid, revision, rec)
                return True
            rec["state"] = "running"
            rec["started_at"] = self.now()
            revision = self._job_event(jid, revision, "verification.job.changed",
                                       {"state": "running", "evaluator": rec["evaluator"]})
            self.store.put_json(JOB_KIND, jid, revision, rec)
            return True
        script = self.script_for(jid)
        if script is None:
            return self._complete(jid, revision, rec, "not_evaluated", "not_reported")  # absent: complete at once
        pos = rec["pos"]
        if pos >= len(script):
            return False  # HM5-SCRIPT-END: a script without complete keeps the job running
        (key, val), = script[pos].items()
        if key == "wait_until":
            if self.now() < val:
                return False
        elif key == "property":
            known = {pr["property_id"] for pr in rec["properties"]}
            already = {r["property_id"] for r in rec["recorded"]}
            if val["property_id"] in known and val["property_id"] not in already:
                # The launch configuration carries a reason exactly where one is required.
                revision = self._record_result(jid, revision, rec, val["property_id"], val["result"],
                                               val.get("reason"))
        elif key == "observe":
            rec["anchors"].update(val["anchors"])
        elif key == "valid_until":
            rec["valid_until"] = val
        elif key == "complete":
            rec["pos"] = pos + 1
            return self._complete(jid, revision, rec, "not_evaluated", "not_reported")
        rec["pos"] = pos + 1
        self.store.put_json(JOB_KIND, jid, revision, rec)
        return True

    def _complete(self, jid: str, revision: int, rec: dict, fill: str, reason: str) -> bool:
        now = self.now()
        # VERIFICATION 4: results assigned at completion appear in the receipt only;
        # ``recorded`` keeps what the evaluator reported.
        by_id = {r["property_id"]: r for r in rec["recorded"]}
        properties = []
        for prop in rec["properties"]:
            entry = dict(by_id.get(prop["property_id"]) or {"property_id": prop["property_id"], "result": fill,
                                                             "reason": reason})
            entry["evidence"] = []
            properties.append(entry)
        content = {"format": RECEIPT_FORMAT, "receipt": jid, "subjects": rec["subjects"], "contract": rec["contract"],
                   "evaluator": dict(rec["evaluator"], principal=self.p.provider_id),  # HM5-VERIFIER-PRINCIPAL
                   "environment": {"anchors": dict(rec["anchors"])}, "inputs": [], "properties": properties,
                   "observed_from": rec["started_at"] or now, "observed_until": now,
                   "valid_until": rec["valid_until"], "scope": rec["outcome"], "job": jid,
                   "provenance": []}
        # VERIFICATION 4 "Issued receipt members": receipt.issued, then the artifact's
        # events, then job.changed (HM5B-ISSUED-EVENTS).
        artifact_events = []
        reference = self._seal_receipt(jid, content, jid, artifact_events)
        self._emit(None, "verification.receipt.issued", receipt_subject(jid), 1,
                   {"reference": reference, "job": jid})
        for etype, subject, rev, event_payload in artifact_events:
            self._emit(None, etype, subject, rev, event_payload)
        rec["state"] = "completed"
        rec["receipt"] = reference
        revision = self._job_event(jid, revision, "verification.job.changed",
                                   {"state": "completed", "evaluator": rec["evaluator"]})
        self.store.put_json(JOB_KIND, jid, revision, rec)
        return True
