"""The knowledge/1 profile (KNOWLEDGE.md, proposed M5 draft).

Written from KNOWLEDGE.md, CORE.md, EVIDENCE.md, CONTEXT.md section 14, the
conformance README's ``knowledge`` launch control and schemas/knowledge/1
only.

Durable records are ordinary subjects in the provider's SQLite store:

- ``knowledge.claim``: the lineage; its value lists every revision record
  with its digest, producer and recording instant. Its Core revision is the
  number of its latest claim revision (KNOWLEDGE 2).
- ``knowledge.decision``, ``knowledge.evaluation``, ``knowledge.conflict``:
  one immutable record each (a conflict gains its resolution once).
- ``knowledge.authority``: one binding per scope; the subject ID is the scope.

History positions are looked up in the retained event stream at the read.
Section H.M5 of DIVERGENCES.md records the choices the documents leave open;
the ``HM5-`` tags below refer to it.
"""

from __future__ import annotations

import envelope as E
import evidence as EVD
import valuedomain as V
from envelope import Invalid, ptr
from errors import ProtocolError, denied

CLAIM_KIND = "knowledge.claim"
DECISION_KIND = "knowledge.decision"
EVALUATION_KIND = "knowledge.evaluation"
CONFLICT_KIND = "knowledge.conflict"
AUTHORITY_KIND = "knowledge.authority"
KINDS = (CLAIM_KIND, DECISION_KIND, EVALUATION_KIND, CONFLICT_KIND, AUTHORITY_KIND)
RECORD_FORMAT = "combraton-knowledge-claim/1"
PLANES = ("normative", "observed", "interpretive")
CARDINALITIES = ("single", "multiple", "unknown")
HEALTH = ("healthy", "degraded", "failing", "unknown")
DERIVATIONS = ("deterministic", "model_assisted", "human")
CONDITION_KINDS = ("repository_tree", "dirty_snapshot", "environment_digest")
DECISIONS = ("accepted_for_use", "rejected", "superseded")
PERMITTED_USES = ("binding", "evidence", "hypothesis", "reference")
RESOLUTIONS = ("select", "narrow_scope", "reject_support", "supersede", "request_observation", "not_a_conflict")
MAX_VALUE_BYTES = 4096
DEFAULT_EVALUATOR = {"id": "reference-conditions", "version": "1", "condition_kinds": list(CONDITION_KINDS)}


class KnowledgeConfigError(Exception):
    pass


def parse_knowledge_config(raw) -> dict:
    if not isinstance(raw, dict):
        raise KnowledgeConfigError("knowledge must be an object")
    unknown = set(raw) - {"evaluator", "serve_altered_claims"}
    if unknown:
        raise KnowledgeConfigError(f"knowledge: unknown members {sorted(unknown)}")
    evaluator = dict(DEFAULT_EVALUATOR)
    if "evaluator" in raw:
        ev = raw["evaluator"]
        if not isinstance(ev, dict) or set(ev) - {"id", "version", "condition_kinds"} or not {"id", "version"} <= set(ev):
            raise KnowledgeConfigError("knowledge.evaluator is { id, version, condition_kinds? }")
        for member in ("id", "version"):
            if not isinstance(ev[member], str) or not ev[member]:
                raise KnowledgeConfigError(f"knowledge.evaluator.{member} must be a non-empty string")
        kinds = ev.get("condition_kinds", list(CONDITION_KINDS))
        if not isinstance(kinds, list) or any(k not in CONDITION_KINDS for k in kinds):
            raise KnowledgeConfigError("knowledge.evaluator.condition_kinds lists known condition kinds")
        evaluator = {"id": ev["id"], "version": ev["version"], "condition_kinds": list(kinds)}
    altered = raw.get("serve_altered_claims", [])
    if not isinstance(altered, list) or not all(isinstance(c, str) for c in altered):
        raise KnowledgeConfigError("knowledge.serve_altered_claims must be an array of claim IDs")
    return {"evaluator": evaluator, "serve_altered_claims": list(altered)}


def subject_of(kind: str, sid: str) -> dict:
    return {"kind": kind, "id": sid}


# ------------------------------------------------------------- validation

def _primary_precondition(env: dict, create: bool, what: str) -> None:
    if len(env["preconditions"]) != 1:
        raise Invalid("/preconditions", "exactly one precondition is required")
    if env["preconditions"][0]["subject"] != env["subject"]:
        raise Invalid("/preconditions/0/subject", "the precondition must name the command's subject")
    revision = env["preconditions"][0]["revision"]
    if create and revision != 0:
        raise Invalid("/preconditions/0/revision", f"{what} creates the subject: revision must be 0")
    if not create and revision < 1:
        raise Invalid("/preconditions/0/revision", f"{what} needs the subject's current revision")


def evidence_reference(v, path: str) -> dict:
    """knowledge/1 and verification/1 evidence_reference: provider required."""
    EVD.reference(v, path)
    if "provider" not in v:
        raise Invalid(ptr(path, "provider"), "required field missing")
    return v


def claim_reference(v, path: str) -> dict:
    E.closed(v, path, ("provider", "claim", "revision", "digest"))
    E.identifier(v["provider"], ptr(path, "provider"))
    E.identifier(v["claim"], ptr(path, "claim"))
    E.integer(v["revision"], ptr(path, "revision"), 1)
    E.digest_string(v["digest"], ptr(path, "digest"))
    return v


def receipt_reference(v, path: str) -> dict:
    E.closed(v, path, ("provider", "receipt", "artifact"))
    E.identifier(v["provider"], ptr(path, "provider"))
    E.identifier(v["receipt"], ptr(path, "receipt"))
    evidence_reference(v["artifact"], ptr(path, "artifact"))
    return v


def basis(v, path: str) -> dict:
    """schemas/knowledge/1/common.schema.json#/$defs/basis (also targets)."""
    E.closed(v, path, ("repositories", "completeness"), ("environment", "build"))
    repos = E.array(v["repositories"], ptr(path, "repositories"), max_items=64)
    for idx, repo in enumerate(repos):
        p = ptr(ptr(path, "repositories"), idx)
        E.closed(repo, p, ("id", "tree"), ("dirty",))
        E.string(repo["id"], ptr(p, "id"), 1, 128)
        E.string(repo["tree"], ptr(p, "tree"), 1, 128)
        if "dirty" in repo:
            d = E.closed(repo["dirty"], ptr(p, "dirty"), ("snapshot_digest",))
            E.digest_string(d["snapshot_digest"], ptr(ptr(p, "dirty"), "snapshot_digest"))
    for member in ("environment", "build"):
        if member in v:
            E.string(v[member], ptr(path, member), 1, 256)
    if v["completeness"] not in ("complete", "partial"):
        raise Invalid(ptr(path, "completeness"), "must be complete or partial")
    return v


def condition(v, path: str) -> dict:
    if not isinstance(v, dict):
        raise Invalid(path, "must be an object")
    kind = v.get("kind")
    if kind in ("repository_tree", "dirty_snapshot"):
        E.closed(v, path, ("condition_id", "kind", "repository", "expected"))
        E.string(v["repository"], ptr(path, "repository"), 1, 128)
    elif kind == "environment_digest":
        E.closed(v, path, ("condition_id", "kind", "expected"))
    else:
        raise Invalid(ptr(path, "kind"), "unknown condition kind")
    E.identifier(v["condition_id"], ptr(path, "condition_id"))
    E.string(v["expected"], ptr(path, "expected"), 1, 256)
    return v


def root(v, path: str) -> dict:
    if not isinstance(v, dict):
        raise Invalid(path, "must be an object")
    kind = v.get("kind")
    if kind == "evidence":
        E.closed(v, path, ("kind", "provider", "artifact", "digest"))
        E.subject(v["artifact"], ptr(path, "artifact"), EVD.ARTIFACT_KIND)
    elif kind == "claim":
        E.closed(v, path, ("kind", "provider", "claim", "revision", "digest"))
        E.identifier(v["claim"], ptr(path, "claim"))
        E.integer(v["revision"], ptr(path, "revision"), 1)
    else:
        raise Invalid(ptr(path, "kind"), "must be evidence or claim")
    E.identifier(v["provider"], ptr(path, "provider"))
    E.digest_string(v["digest"], ptr(path, "digest"))
    return v


def _content(payload: dict, revise: bool) -> None:
    required = ("plane", "statement", "scope", "support", "derivation") + (("supersedes",) if revise else ())
    E.closed(payload, "/payload", required, ("validity", "basis", "dependencies", "conditions", "health"))
    if payload["plane"] not in PLANES:
        raise Invalid("/payload/plane", "unknown plane")  # KNW-2
    st = E.closed(payload["statement"], "/payload/statement", ("subject", "predicate", "value", "cardinality"))
    E.subject(st["subject"], "/payload/statement/subject")
    E.string(st["predicate"], "/payload/statement/predicate", 1, 128)
    if len(V.canonical(st["value"])) > MAX_VALUE_BYTES:
        raise Invalid("/payload/statement/value", "canonical JSON of at most 4096 bytes")
    if st["cardinality"] not in CARDINALITIES:
        raise Invalid("/payload/statement/cardinality", "must be single, multiple or unknown")
    scope = E.closed(payload["scope"], "/payload/scope", ("id", "qualifiers"))
    E.identifier(scope["id"], "/payload/scope/id")
    q = scope["qualifiers"]
    if not isinstance(q, dict) or len(q) > 32:
        raise Invalid("/payload/scope/qualifiers", "must be an object with at most 32 members")
    for name, value in q.items():
        E.string(name, ptr("/payload/scope/qualifiers", name), 1, 64)
        E.string(value, ptr("/payload/scope/qualifiers", name), 1, 256)
    if "validity" in payload:
        val = E.closed(payload["validity"], "/payload/validity", (), ("from", "until"))
        for member in ("from", "until"):
            if member in val:
                E.instant(val[member], ptr("/payload/validity", member))
    if "basis" in payload:
        basis(payload["basis"], "/payload/basis")
    support = E.array(payload["support"], "/payload/support", max_items=64)
    for idx, entry in enumerate(support):
        p = ptr("/payload/support", idx)
        E.closed(entry, p, ("support_id", "evidence", "ancestry"))
        E.identifier(entry["support_id"], ptr(p, "support_id"))
        evidence_reference(entry["evidence"], ptr(p, "evidence"))
        anc = E.closed(entry["ancestry"], ptr(p, "ancestry"), ("completeness", "roots"))
        if anc["completeness"] not in ("complete", "partial", "unknown"):
            raise Invalid(ptr(ptr(p, "ancestry"), "completeness"), "must be complete, partial or unknown")
        roots = E.array(anc["roots"], ptr(ptr(p, "ancestry"), "roots"), max_items=32)
        for j, r in enumerate(roots):
            root(r, ptr(ptr(ptr(p, "ancestry"), "roots"), j))
        if anc["completeness"] == "unknown" and roots:
            # KNOWLEDGE 5: with unknown ancestry roots MUST be empty (HM5-UNKNOWN-ROOTS-PATH).
            raise Invalid(ptr(ptr(p, "ancestry"), "roots"), "unknown ancestry declares no roots")
    der = E.closed(payload["derivation"], "/payload/derivation", ("kind", "inputs"), ("record",))
    if der["kind"] not in DERIVATIONS:
        raise Invalid("/payload/derivation/kind", "must be deterministic, model_assisted or human")
    if "record" in der:
        evidence_reference(der["record"], "/payload/derivation/record")
    E.array(der["inputs"], "/payload/derivation/inputs", max_items=32)
    for idx, ref in enumerate(der["inputs"]):
        evidence_reference(ref, ptr("/payload/derivation/inputs", idx))
    if "dependencies" in payload:
        E.array(payload["dependencies"], "/payload/dependencies", max_items=32)
        for idx, ref in enumerate(payload["dependencies"]):
            claim_reference(ref, ptr("/payload/dependencies", idx))
    if "conditions" in payload:
        E.array(payload["conditions"], "/payload/conditions", max_items=32)
        for idx, c in enumerate(payload["conditions"]):
            condition(c, ptr("/payload/conditions", idx))
    if "health" in payload:
        if payload["health"] not in HEALTH:
            raise Invalid("/payload/health", "must be healthy, degraded, failing or unknown")
        if payload["plane"] != "observed":
            raise Invalid("/payload/health", "health is allowed only on observed claims")
    if revise:
        sup = E.closed(payload["supersedes"], "/payload/supersedes", ("revision", "digest"))
        E.integer(sup["revision"], "/payload/supersedes/revision", 1)
        E.digest_string(sup["digest"], "/payload/supersedes/digest")


def _rank(use: str) -> int:
    return {"reference": 0, "hypothesis": 1, "evidence": 2, "binding": 3}[use]


# --------------------------------------------------------------- ancestry

def _pair(a: dict, b: dict) -> str:
    """KNOWLEDGE 5 pair relation."""
    ra, rb = a["ancestry"]["roots"], b["ancestry"]["roots"]
    if any(x == y for x in ra for y in rb):
        return "shared"
    if (a["ancestry"]["completeness"] == "complete" and b["ancestry"]["completeness"] == "complete"
            and not any(x["digest"] == y["digest"] for x in ra for y in rb)):
        return "disjoint"
    return "undetermined"


def support_class(entries: list) -> str:
    """KNOWLEDGE 5 support class; the first matching rule applies."""
    if not entries:
        return "unsupported"
    n = len(entries)
    pairs = [_pair(entries[i], entries[j]) for i in range(n) for j in range(i + 1, n)]
    if any(e["ancestry"]["completeness"] == "unknown" for e in entries) and "disjoint" not in pairs:
        return "undetermined"
    first = entries[0]["ancestry"]["roots"]
    if any(all(any(r == x for x in e["ancestry"]["roots"]) for e in entries) for r in first):
        return "single_lineage"
    if "disjoint" in pairs:
        return "multiple_lineages"
    # HM5-SINGLE-ENTRY-OVERLAP: "every pair is shared" needs at least one pair.
    if pairs and all(p == "shared" for p in pairs):
        return "overlapping_lineages"
    return "undetermined"


# ------------------------------------------------------------- comparison

def _fully_known(validity) -> bool:
    return validity is not None and "from" in validity and "until" in validity


def _bases_disjoint(a, b) -> bool:
    if a is None or b is None:
        return False
    trees_b = {}
    for r in b["repositories"]:
        trees_b.setdefault(r["id"], set()).add(r["tree"])
    for r in a["repositories"]:
        if r["id"] in trees_b and r["tree"] not in trees_b[r["id"]]:
            return True
    for member in ("environment", "build"):
        if member in a and member in b and a[member] != b[member]:
            return True
    return False


def _same_complete_bases(a, b) -> bool:
    if a is None or b is None or a["completeness"] != "complete" or b["completeness"] != "complete":
        return False
    repos_a = sorted((r["id"], r["tree"]) for r in a["repositories"])
    repos_b = sorted((r["id"], r["tree"]) for r in b["repositories"])
    return repos_a == repos_b and a.get("environment") == b.get("environment") and a.get("build") == b.get("build")


def compare(ra: dict, rb: dict):
    """KNOWLEDGE 7: (refusal reason or None, kind, status, uncertain)."""
    sa, sb = ra["statement"], rb["statement"]
    if V.canonical(sa["subject"]) != V.canonical(sb["subject"]):
        return "subject_differs", None, None, None
    if sa["predicate"] != sb["predicate"]:
        return "predicate_differs", None, None, None
    if ra["scope"]["id"] != rb["scope"]["id"]:
        return "scope_differs", None, None, None
    qa, qb = ra["scope"]["qualifiers"], rb["scope"]["qualifiers"]
    if any(k in qb and qb[k] != v for k, v in qa.items()):
        return "qualifiers_disjoint", None, None, None
    if _bases_disjoint(ra["basis"], rb["basis"]):
        return "basis_disjoint", None, None, None
    va, vb = ra["validity"], rb["validity"]
    if _fully_known(va) and _fully_known(vb) and not (va["from"] < vb["until"] and vb["from"] < va["until"]):
        return "validity_disjoint", None, None, None
    if "multiple" in (sa["cardinality"], sb["cardinality"]):
        return "multiple_values_permitted", None, None, None
    if V.canonical(sa["value"]) == V.canonical(sb["value"]):
        return "values_equal", None, None, None
    planes = {ra["plane"], rb["plane"]}
    kind = "drift" if planes == {"normative", "observed"} else "conflict"
    uncertain = []
    if qa != qb:
        uncertain.append("qualifiers")
    if not _same_complete_bases(ra["basis"], rb["basis"]):
        uncertain.append("basis")
    if not (_fully_known(va) and _fully_known(vb)):
        uncertain.append("validity")
    if not (sa["cardinality"] == "single" and sb["cardinality"] == "single"):
        uncertain.append("cardinality")
    if not (ra["plane"] == rb["plane"] or planes == {"normative", "observed"}):
        uncertain.append("plane")
    return None, kind, ("potential" if uncertain else "demonstrated"), uncertain


# ---------------------------------------------------------------- profile

class Knowledge:
    def __init__(self, provider, cfg: dict):
        self.p = provider
        self.evaluator = cfg.get("evaluator", DEFAULT_EVALUATOR)
        self.altered = set(cfg.get("serve_altered_claims", []))

    @property
    def store(self):
        return self.p.store

    def now(self) -> str:
        return self.p.clock.now()

    # ------------------------------------------------------- validators
    def v_propose(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", CLAIM_KIND)
        _primary_precondition(env, True, method)
        _content(env["payload"], revise=False)

    def v_revise(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", CLAIM_KIND)
        _primary_precondition(env, False, method)
        _content(env["payload"], revise=True)

    def v_claim_query(self, env, method, features) -> None:
        E.query_envelope(env, method, "core.grants" in features)
        optional = ("revision",) if method == "knowledge.claim.inspect" else ()
        payload = E.closed(env["payload"], "/payload", ("claim",), optional)
        E.identifier(payload["claim"], "/payload/claim")
        if "revision" in payload:
            E.integer(payload["revision"], "/payload/revision", 1)

    def v_bind(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", AUTHORITY_KIND)
        _primary_precondition(env, method == "knowledge.authority.bind", method)
        payload = E.closed(env["payload"], "/payload", ("authority",))
        E.identifier(payload["authority"], "/payload/authority")

    def v_authority_get(self, env, method, features) -> None:
        E.query_envelope(env, method, "core.grants" in features)
        payload = E.closed(env["payload"], "/payload", ("scope",))
        E.identifier(payload["scope"], "/payload/scope")

    def v_decision(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", DECISION_KIND)
        _primary_precondition(env, True, method)
        payload = E.closed(env["payload"], "/payload", ("claim", "decision", "validation_basis", "rationale"),
                           ("permitted_use", "supersedes_decision"))
        claim_reference(payload["claim"], "/payload/claim")
        if payload["decision"] not in DECISIONS:
            raise Invalid("/payload/decision", "must be accepted_for_use, rejected or superseded")
        if "permitted_use" in payload and payload["permitted_use"] not in PERMITTED_USES:
            raise Invalid("/payload/permitted_use", "unknown permitted use")
        if (payload["decision"] == "accepted_for_use") != ("permitted_use" in payload):
            raise Invalid("/payload/permitted_use", "required for accepted_for_use and absent otherwise")
        if "supersedes_decision" in payload:
            E.identifier(payload["supersedes_decision"], "/payload/supersedes_decision")
        vb = E.closed(payload["validation_basis"], "/payload/validation_basis", ("evidence", "receipts"), ("target",))
        E.array(vb["evidence"], "/payload/validation_basis/evidence", max_items=32)
        for idx, ref in enumerate(vb["evidence"]):
            evidence_reference(ref, ptr("/payload/validation_basis/evidence", idx))
        E.array(vb["receipts"], "/payload/validation_basis/receipts", max_items=32)
        for idx, ref in enumerate(vb["receipts"]):
            receipt_reference(ref, ptr("/payload/validation_basis/receipts", idx))
        if "target" in vb:
            basis(vb["target"], "/payload/validation_basis/target")
        E.string(payload["rationale"], "/payload/rationale", 1, 4096)
        if "authority_epoch" not in env:
            raise Invalid("/authority_epoch", "required for knowledge.decision.record")  # KNOWLEDGE 6

    def v_open(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", CONFLICT_KIND)
        _primary_precondition(env, True, method)
        payload = E.closed(env["payload"], "/payload", ("revisions",), ("note",))
        E.array(payload["revisions"], "/payload/revisions", 2, 2)
        for idx, ref in enumerate(payload["revisions"]):
            claim_reference(ref, ptr("/payload/revisions", idx))
        if "note" in payload:
            E.string(payload["note"], "/payload/note", 1, 4096)
        a, b = payload["revisions"]
        if (a["provider"], a["claim"], a["revision"]) == (b["provider"], b["claim"], b["revision"]):
            raise Invalid("/payload/revisions", "the same revision is named twice")

    def v_resolve(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", CONFLICT_KIND)
        _primary_precondition(env, False, method)
        payload = E.closed(env["payload"], "/payload", ("resolution", "rationale"), ("selected",))
        if payload["resolution"] not in RESOLUTIONS:
            raise Invalid("/payload/resolution", "unknown resolution")
        if "selected" in payload:
            claim_reference(payload["selected"], "/payload/selected")
        E.string(payload["rationale"], "/payload/rationale", 1, 4096)
        if "authority_epoch" not in env:
            raise Invalid("/authority_epoch", "required for knowledge.conflict.resolve")  # CORE 8 (HM5-RESOLVE-EPOCH)

    def v_evaluate(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", EVALUATION_KIND)
        _primary_precondition(env, True, method)
        payload = E.closed(env["payload"], "/payload", ("claim", "target"))
        claim_reference(payload["claim"], "/payload/claim")
        basis(payload["target"], "/payload/target")

    # ---------------------------------------------------- authorization
    def _read_needs(self, refs) -> list:
        """KNOWLEDGE 10: commands that name revisions need knowledge.read on
        those claims; only claims at this provider can be covered here
        (HM5-NAMED-REVISION-RIGHTS)."""
        needs = []
        for ref in refs:
            if ref.get("provider") == self.p.provider_id:
                subj = subject_of(CLAIM_KIND, ref["claim"])
                if ("knowledge.read", subj) not in needs:
                    needs.append(("knowledge.read", subj))
        return needs

    def authorize(self, method: str, env: dict):
        p = self.p
        payload = env["payload"]
        if method in ("knowledge.authority.bind", "knowledge.authority.transfer"):
            # KNOWLEDGE 6: only authority principals, whatever grant is named (HM5-BIND-GRANT).
            if not p.is_authority or env.get("grant") is not None:
                raise denied("not_authority")
            return p.make_auth(None)
        if method in ("knowledge.claim.propose", "knowledge.claim.revise"):
            refs = list(payload.get("dependencies", []))
            refs += [r for e in payload["support"] for r in e["ancestry"]["roots"] if r["kind"] == "claim"]
            needs = [("knowledge.propose", env["subject"])]
            if method == "knowledge.claim.revise":
                needs.append(("knowledge.read", env["subject"]))  # supersedes names its revision
            needs += [n for n in self._read_needs(refs) if n not in needs]
            return p.authorize_under(env, needs)
        if method in ("knowledge.claim.inspect", "knowledge.claim.history"):
            return p.authorize_under(env, [("knowledge.read", subject_of(CLAIM_KIND, payload["claim"]))])
        if method == "knowledge.authority.get":
            return p.authorize_under(env, [("knowledge.read", subject_of(AUTHORITY_KIND, payload["scope"]))])
        if method == "knowledge.decision.record":
            return p.authorize_under(env, [("knowledge.decide", env["subject"])] + self._read_needs([payload["claim"]]))
        if method == "knowledge.conflict.open":
            return p.authorize_under(env, [("knowledge.propose", env["subject"])] + self._read_needs(payload["revisions"]))
        if method == "knowledge.conflict.resolve":
            refs = [payload["selected"]] if "selected" in payload else []
            return p.authorize_under(env, [("knowledge.decide", env["subject"])] + self._read_needs(refs))
        if method == "knowledge.applicability.evaluate":
            return p.authorize_under(env, [("knowledge.propose", env["subject"])] + self._read_needs([payload["claim"]]))
        raise AssertionError(method)

    # ------------------------------------------------------------ reads
    def lineage(self, cid: str):
        return self.store.get_json(CLAIM_KIND, cid)

    def revision_entry(self, ref: dict):
        """The stored revision a reference names at this provider, or None."""
        if ref["provider"] != self.p.provider_id:
            return None
        row = self.lineage(ref["claim"])
        if row is None or not 1 <= ref["revision"] <= len(row[1]["revisions"]):
            return None
        return row[1]["revisions"][ref["revision"] - 1]

    def served_record(self, entry: dict) -> dict:
        """The record as this provider serves it to readers; the adversarial
        ``serve_altered_claims`` alters it under the same reference."""
        record = entry["record"]
        if record["claim"] in self.altered:
            record = dict(record, statement=dict(record["statement"], value={"altered": record["statement"]["value"]}))
        return record

    @staticmethod
    def reference_of(entry: dict) -> dict:
        r = entry["record"]
        return {"provider": r["provider"], "claim": r["claim"], "revision": r["revision"], "digest": entry["digest"]}

    @staticmethod
    def same_revision(a: dict, b: dict) -> bool:
        return all(a[k] == b[k] for k in ("provider", "claim", "revision", "digest"))

    def decisions_for(self, ref: dict) -> list:
        out = []
        for did in self.store.ids_of(DECISION_KIND):
            rec = self.store.get_json(DECISION_KIND, did)[1]
            if self.same_revision(rec["claim"], ref):
                out.append((did, rec))
        return out

    def evaluations_for(self, ref: dict) -> list:
        out = []
        for eid in self.store.ids_of(EVALUATION_KIND):
            rec = self.store.get_json(EVALUATION_KIND, eid)[1]
            if self.same_revision(rec["claim"], ref):
                out.append((eid, rec))
        return out

    def conflicts_for(self, ref: dict) -> list:
        out = []
        for cid in self.store.ids_of(CONFLICT_KIND):
            rec = self.store.get_json(CONFLICT_KIND, cid)[1]
            if any(self.same_revision(r, ref) for r in rec["revisions"]):
                out.append((cid, rec))
        return out

    def reliance(self, ref: dict) -> dict:
        decisions = self.decisions_for(ref)
        if not decisions:
            return {"state": "proposed"}
        did, rec = decisions[-1]
        out = {"state": rec["value"], "decision": did}
        if rec["permitted_use"] is not None:
            out["permitted_use"] = rec["permitted_use"]
        out["author_is_decider"] = rec["author_is_decider"]
        return out

    def latest_evaluation(self, ref: dict, target: dict):
        want = V.canonical(target)
        found = None
        for eid, rec in self.evaluations_for(ref):
            if V.canonical(rec["target"]) == want:
                found = (eid, rec)
        return found

    def availability(self, support: list) -> dict:
        """KNOWLEDGE 5 "Availability", observed at the read (HM5-AVAILABILITY)."""
        counts = {"available": 0, "unavailable": 0, "purged": 0, "unknown": 0}
        ev = self.p.evidence
        for entry in support:
            ref = entry["evidence"]
            if ref["provider"] != self.p.provider_id:
                counts["unknown"] += 1
                continue
            row = ev.load(ref["artifact"]["id"])
            if row is None or row[1]["state"] != "sealed" or row[1]["descriptor"]["digest"] != ref["digest"]:
                counts["unavailable"] += 1
                continue
            state = ev.availability(ref["artifact"]["id"], row[1])["state"]
            counts["available" if state == "available" else "purged" if state == "purged" else "unavailable"] += 1
        total = sum(counts.values())
        if counts["available"] == total:
            state = "complete"
        elif counts["available"]:
            state = "partial"
        elif counts["unknown"]:
            state = "unknown"
        elif counts["purged"] == total:
            state = "purged"
        else:
            state = "unavailable"
        return {"state": state, "counts": counts}

    # --------------------------------------------------------- commands
    def _record(self, env: dict, cid: str, revision: int) -> dict:
        payload = env["payload"]
        return {"format": RECORD_FORMAT, "provider": self.p.provider_id, "claim": cid, "revision": revision,
                "producer": self.p.principal, "plane": payload["plane"], "statement": payload["statement"],
                "scope": payload["scope"], "validity": payload.get("validity"), "basis": payload.get("basis"),
                "support": payload["support"], "derivation": payload["derivation"],
                "dependencies": payload.get("dependencies", []), "conditions": payload.get("conditions", []),
                "health": payload.get("health", "not_applicable"), "supersedes": payload.get("supersedes")}

    def _add_revision(self, env: dict, lineage: dict | None):
        cid = env["subject"]["id"]
        lineage = lineage or {"revisions": []}
        revision = len(lineage["revisions"]) + 1
        record = self._record(env, cid, revision)
        digest = V.digest("sha256", V.canonical(record))
        lineage["revisions"].append({"record": record, "digest": digest, "recorded_at": self.now()})
        self.store.put_json(CLAIM_KIND, cid, revision, lineage)
        reference = {"provider": self.p.provider_id, "claim": cid, "revision": revision, "digest": digest}
        return revision, {"reference": reference}, [
            ("knowledge.claim.revised", subject_of(CLAIM_KIND, cid), revision, {"reference": reference})]

    def op_propose(self, env, auth):
        self.p.check_preconditions(env, auth)
        return self._add_revision(env, None)

    def op_revise(self, env, auth):
        self.p.check_preconditions(env, auth)
        _, lineage = self.lineage(env["subject"]["id"])
        current = lineage["revisions"][-1]
        sup = env["payload"]["supersedes"]
        if sup["revision"] != current["record"]["revision"]:
            raise ProtocolError("invalid_envelope", {"path": "/payload/supersedes/revision",
                                                     "reason": "supersedes must name the current revision"})
        if sup["digest"] != current["digest"]:
            raise ProtocolError("invalid_envelope", {"path": "/payload/supersedes/digest",
                                                     "reason": "the digest of the current revision differs"})
        return self._add_revision(env, lineage)

    def op_bind(self, env, auth):
        self.p.check_preconditions(env, auth)
        scope = env["subject"]["id"]
        authority = env["payload"]["authority"]
        if env["operation"] == "knowledge.authority.bind":
            epoch = 1
            etype = "knowledge.authority.bound"
        else:
            epoch = self.store.get_json(AUTHORITY_KIND, scope)[1]["epoch"] + 1
            etype = "knowledge.authority.transferred"
        self.store.put_json(AUTHORITY_KIND, scope, epoch, {"authority": authority, "epoch": epoch})
        return epoch, {"scope": scope, "authority": authority, "epoch": epoch}, [
            (etype, subject_of(AUTHORITY_KIND, scope), epoch, {"authority": authority, "epoch": epoch})]

    def _authority_checks(self, env: dict, auth, scope: str) -> dict:
        """KNOWLEDGE 6 checks 2 to 4: binding, bound authority, epoch."""
        row = self.store.get_json(AUTHORITY_KIND, scope)
        if row is None:
            raise denied("not_authority")
        binding = row[1]
        if binding["authority"] != self.p.principal:
            raise denied("not_authority")
        claimed = env["authority_epoch"]
        if claimed < binding["epoch"]:
            details = {"current_epoch": binding["epoch"]} if auth.may_read(subject_of(AUTHORITY_KIND, scope)) else {}
            raise ProtocolError("stale_authority_epoch", details)
        if claimed > binding["epoch"]:
            raise ProtocolError("unknown_authority_epoch")
        return binding

    def op_decision(self, env, auth):
        payload = env["payload"]
        ref = payload["claim"]
        entry = self.revision_entry(ref)
        if entry is None:
            raise ProtocolError("not_found")
        binding = self._authority_checks(env, auth, entry["record"]["scope"]["id"])
        # HM5-CORE-PRECONDITION-ORDER: Core preconditions follow the epoch check (CORE 10 step 7).
        self.p.check_preconditions(env, auth)
        if entry["digest"] != ref["digest"]:
            raise ProtocolError("invalid_envelope", {"path": "/payload/claim/digest",
                                                     "reason": "the digest differs from the revision's"})
        decisions = self.decisions_for(ref)
        latest = decisions[-1][0] if decisions else None
        if payload.get("supersedes_decision") != latest:
            raise ProtocolError("precondition_failed", {"latest_decision": latest})
        did = env["subject"]["id"]
        author = entry["record"]["producer"] == self.p.principal
        permitted = payload.get("permitted_use")
        rec = {"claim": ref, "value": payload["decision"], "permitted_use": permitted, "decider": self.p.principal,
               "author_is_decider": author, "epoch": binding["epoch"], "supersedes_decision": latest,
               "validation_basis": payload["validation_basis"], "rationale": payload["rationale"],
               "recorded_at": self.now()}
        self.store.put_json(DECISION_KIND, did, 1, rec)
        outcome = {"decision": subject_of(DECISION_KIND, did), "claim": ref, "value": payload["decision"]}
        event = {"claim": ref, "decision": payload["decision"]}
        if permitted is not None:
            outcome["permitted_use"] = permitted
            event["permitted_use"] = permitted
        outcome.update(author_is_decider=author, epoch=binding["epoch"])
        event.update(author_is_decider=author, supersedes_decision=latest)
        return 1, outcome, [("knowledge.decision.recorded", subject_of(DECISION_KIND, did), 1, event)]

    def op_open(self, env, auth):
        self.p.check_preconditions(env, auth)
        refs = env["payload"]["revisions"]
        entries = [self.revision_entry(r) for r in refs]
        if any(e is None for e in entries):
            raise ProtocolError("not_found")
        for idx, (ref, entry) in enumerate(zip(refs, entries)):
            if entry["digest"] != ref["digest"]:
                raise ProtocolError("invalid_envelope", {"path": f"/payload/revisions/{idx}/digest",
                                                         "reason": "the digest differs from the revision's"})
        reason, kind, status, uncertain = compare(entries[0]["record"], entries[1]["record"])
        if reason is not None:
            raise ProtocolError("claims_not_comparable", {"reason": reason})
        cid = env["subject"]["id"]
        rec = {"revisions": refs, "kind": kind, "status": status, "uncertain": uncertain, "state": "open",
               "resolution": None, "recorded_at": self.now()}
        if "note" in env["payload"]:
            rec["note"] = env["payload"]["note"]
        self.store.put_json(CONFLICT_KIND, cid, 1, rec)
        return 1, {"conflict": subject_of(CONFLICT_KIND, cid), "kind": kind, "status": status, "uncertain": uncertain}, [
            ("knowledge.conflict.opened", subject_of(CONFLICT_KIND, cid), 1,
             {"kind": kind, "status": status, "revisions": refs})]

    def op_resolve(self, env, auth):
        cid = env["subject"]["id"]
        row = self.store.get_json(CONFLICT_KIND, cid)
        if row is None or row[1]["state"] != "open":
            raise ProtocolError("not_found")
        revision, rec = row
        entry = self.revision_entry(rec["revisions"][0])
        binding = self._authority_checks(env, auth, entry["record"]["scope"]["id"])
        self.p.check_preconditions(env, auth)  # HM5-CORE-PRECONDITION-ORDER
        payload = env["payload"]
        resolution = payload["resolution"]
        if resolution == "select":
            if "selected" not in payload or not any(self.same_revision(payload["selected"], r) for r in rec["revisions"]):
                raise ProtocolError("invalid_envelope", {"path": "/payload/selected",
                                                         "reason": "select names one of the two revisions"})
        elif "selected" in payload:
            raise ProtocolError("invalid_envelope", {"path": "/payload/selected",
                                                     "reason": "selected is allowed only for select"})
        rec.update(state="resolved", resolution=resolution, rationale=payload["rationale"], resolver=self.p.principal,
                   epoch=binding["epoch"], resolved_at=self.now())
        if "selected" in payload:
            rec["selected"] = payload["selected"]
        revision += 1
        self.store.put_json(CONFLICT_KIND, cid, revision, rec)
        return revision, {"conflict": subject_of(CONFLICT_KIND, cid), "state": "resolved", "resolution": resolution}, [
            ("knowledge.conflict.resolved", subject_of(CONFLICT_KIND, cid), revision, {"resolution": resolution})]

    def evaluate(self, record: dict, ref: dict, target: dict):
        """KNOWLEDGE 8: findings and result against one target."""
        kinds = self.evaluator["condition_kinds"]
        repos = {}
        for r in target["repositories"]:
            repos.setdefault(r["id"], r)
        findings = []
        for c in record["conditions"]:
            if c["kind"] not in kinds:
                finding = "unsupported"
            elif c["kind"] == "repository_tree":
                repo = repos.get(c["repository"])
                finding = "missing_anchor" if repo is None else ("match" if repo["tree"] == c["expected"] else "mismatch")
            elif c["kind"] == "dirty_snapshot":
                repo = repos.get(c["repository"])
                if repo is None or "dirty" not in repo:
                    finding = "missing_anchor"
                else:
                    finding = "match" if repo["dirty"]["snapshot_digest"] == c["expected"] else "mismatch"
            else:
                env_value = target.get("environment")
                finding = "missing_anchor" if env_value is None else ("match" if env_value == c["expected"] else "mismatch")
            findings.append({"condition_id": c["condition_id"], "finding": finding})
        for dep in record["dependencies"]:
            finding = "unchecked"
            if not self.same_revision(dep, ref):
                latest = self.latest_evaluation(dep, target)
                if latest is not None and latest[1]["result"] == "applicable":
                    finding = "match"
                elif latest is not None and latest[1]["result"] == "invalid_for_target":
                    finding = "mismatch"
            findings.append({"dependency": dep, "finding": finding})
        values = [f["finding"] for f in findings]
        if "mismatch" in values:
            result = "invalid_for_target"
        elif "missing_anchor" in values or "unsupported" in values:
            result = "unknown"
        elif "unchecked" in values:
            result = "needs_check"
        elif not values:
            result = "unknown"
        else:
            result = "applicable"
        return findings, result

    def op_evaluate(self, env, auth):
        self.p.check_preconditions(env, auth)
        payload = env["payload"]
        ref, target = payload["claim"], payload["target"]
        entry = self.revision_entry(ref)
        if entry is None:
            raise ProtocolError("not_found")
        if entry["digest"] != ref["digest"]:
            raise ProtocolError("invalid_envelope", {"path": "/payload/claim/digest",
                                                     "reason": "the digest differs from the revision's"})
        findings, result = self.evaluate(entry["record"], ref, target)
        previous = self.latest_evaluation(ref, target)
        supersedes = previous[0] if previous else None
        evaluator = {"id": self.evaluator["id"], "version": self.evaluator["version"]}
        eid = env["subject"]["id"]
        rec = {"claim": ref, "target": target, "evaluator": evaluator, "result": result, "findings": findings,
               "supersedes_evaluation": supersedes, "recorded_at": self.now()}
        self.store.put_json(EVALUATION_KIND, eid, 1, rec)
        subject = subject_of(EVALUATION_KIND, eid)
        return 1, {"evaluation": subject, "claim": ref, "result": result, "evaluator": evaluator,
                   "findings": findings, "supersedes_evaluation": supersedes}, [
            ("knowledge.evaluation.recorded", subject, 1,
             {"claim": ref, "result": result, "evaluator": evaluator, "supersedes_evaluation": supersedes})]

    # ---------------------------------------------------------- queries
    def op_inspect(self, env, auth) -> dict:
        payload = env["payload"]
        row = self.lineage(payload["claim"])
        if row is None:
            raise ProtocolError("not_found")
        revisions = row[1]["revisions"]
        n = payload.get("revision", len(revisions))
        if not 1 <= n <= len(revisions):
            raise ProtocolError("not_found")
        entry = revisions[n - 1]
        ref = self.reference_of(entry)
        record = entry["record"]
        applicability, order = {}, []
        for eid, rec in self.evaluations_for(ref):
            key = V.canonical(rec["target"])
            if key not in applicability:
                order.append(key)
            applicability[key] = {"evaluation": eid, "target": rec["target"], "result": rec["result"],
                                  "evaluator": rec["evaluator"]}
        return {"reference": ref, "record": self.served_record(entry), "current_revision": len(revisions),
                "reliance": self.reliance(ref), "applicability": [applicability[k] for k in order],
                "health": record["health"], "availability": self.availability(record["support"]),
                "support": {"class": support_class(record["support"]),
                            "unknown_ancestry": [e["support_id"] for e in record["support"]
                                                 if e["ancestry"]["completeness"] == "unknown"]},
                "conflicts": [{"conflict": cid, "kind": rec["kind"], "status": rec["status"], "state": rec["state"]}
                              for cid, rec in self.conflicts_for(ref)]}

    def op_history(self, env, auth) -> dict:
        cid = env["payload"]["claim"]
        row = self.lineage(cid)
        if row is None:
            raise ProtocolError("not_found")
        st = self.store
        revisions, decisions, evaluations, conflicts = [], [], [], []
        refs = []
        for entry in row[1]["revisions"]:
            ref = self.reference_of(entry)
            refs.append(ref)
            revisions.append({"reference": ref, "producer": entry["record"]["producer"],
                              "supersedes": entry["record"]["supersedes"], "recorded_at": entry["recorded_at"],
                              "position": st.event_position("knowledge.claim.revised", CLAIM_KIND, cid, ref["revision"])})

        def ours(r):
            return any(self.same_revision(r, x) for x in refs)

        for did in st.ids_of(DECISION_KIND):
            rec = st.get_json(DECISION_KIND, did)[1]
            if ours(rec["claim"]):
                decisions.append({"decision": did, "claim": rec["claim"], "value": rec["value"],
                                  "permitted_use": rec["permitted_use"], "decider": rec["decider"],
                                  "author_is_decider": rec["author_is_decider"], "epoch": rec["epoch"],
                                  "supersedes_decision": rec["supersedes_decision"], "recorded_at": rec["recorded_at"],
                                  "position": st.event_position("knowledge.decision.recorded", DECISION_KIND, did, 1)})
        for eid in st.ids_of(EVALUATION_KIND):
            rec = st.get_json(EVALUATION_KIND, eid)[1]
            if ours(rec["claim"]):
                evaluations.append({"evaluation": eid, "claim": rec["claim"], "target": rec["target"],
                                    "result": rec["result"], "evaluator": rec["evaluator"],
                                    "supersedes_evaluation": rec["supersedes_evaluation"],
                                    "recorded_at": rec["recorded_at"],
                                    "position": st.event_position("knowledge.evaluation.recorded", EVALUATION_KIND,
                                                                  eid, 1)})
        for kid in st.ids_of(CONFLICT_KIND):
            rec = st.get_json(CONFLICT_KIND, kid)[1]
            if any(ours(r) for r in rec["revisions"]):
                conflicts.append({"conflict": kid, "kind": rec["kind"], "status": rec["status"],
                                  "revisions": rec["revisions"], "state": rec["state"],
                                  "resolution": rec["resolution"], "recorded_at": rec["recorded_at"],
                                  "position": st.event_position("knowledge.conflict.opened", CONFLICT_KIND, kid, 1)})
        return {"claim": cid, "revisions": revisions, "decisions": decisions, "evaluations": evaluations,
                "conflicts": conflicts}

    def op_authority_get(self, env, auth) -> dict:
        scope = env["payload"]["scope"]
        row = self.store.get_json(AUTHORITY_KIND, scope)
        if row is None:
            raise ProtocolError("not_found")
        return {"scope": scope, "authority": row[1]["authority"], "epoch": row[1]["epoch"], "revision": row[0]}

    # ------------------------------------------------- CONTEXT 14 reads
    def snapshot(self, ref: dict, target: dict):
        """A claim as a reader of this store sees it: ``(snapshot, None)``,
        or ``(None, reason)`` with ``knowledge_unavailable`` or
        ``claim_digest_mismatch`` (CONTEXT 14 "Recomputing digests")."""
        entry = self.revision_entry(ref)
        if entry is None:
            return None, "knowledge_unavailable"
        record = self.served_record(entry)
        if V.digest("sha256", V.canonical(record)) != ref["digest"]:
            return None, "claim_digest_mismatch"
        latest = self.latest_evaluation(ref, target)
        applicability = {"result": latest[1]["result"], "evaluation": latest[0]} if latest else {"result": "unknown"}
        row = self.lineage(ref["claim"])
        return {"reference": ref, "plane": record["plane"], "reliance": self.reliance(ref),
                "applicability": applicability,
                "conflicts": [{"conflict": cid, "kind": rec["kind"], "status": rec["status"]}
                              for cid, rec in self.conflicts_for(ref) if rec["state"] == "open"],
                "support": {"class": support_class(record["support"])},
                "current_revision": len(row[1]["revisions"])}, None
