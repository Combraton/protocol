"""The evidence/1 profile over a scripted store (EVIDENCE.md, M4 draft).

Written from EVIDENCE.md, CONTEXT.md, CORE.md, the conformance README's
``evidence_store`` launch control and schemas/evidence/1 only.

Durable records are ordinary subjects in the provider's SQLite store:

- ``evidence.artifact``: descriptor, state, bytes received, staging instant,
  abandonment reason, purge request and proof-loss record. Its revision rises
  by one with every artifact event. Staged and sealed bytes live in the
  store's ``blobs`` table under the artifact ID.
- ``evidence.hold``: artifact, holder reference, reason, expiry, owner and
  state.

The scripted store (``evidence_store``) makes named artifacts fail integrity
or be unavailable at every read, abandons staged uploads after a timeout, and
confirms physical deletion a delay after the purge request, on the provider
clock. Section H of DIVERGENCES.md records every choice the documents leave
open; the tags below refer to it.
"""

from __future__ import annotations

import base64
import binascii
import re
from urllib.parse import parse_qsl, urlsplit

import clock as C
import envelope as E
import grants as G
import valuedomain as V
from envelope import Invalid, ptr
from errors import ProtocolError, denied

ARTIFACT_KIND = "evidence.artifact"
HOLD_KIND = "evidence.hold"
MANIFEST_MEDIA_TYPE = "application/vnd.combraton.evidence-manifest+json"
MANIFEST_FORMAT = "combraton-evidence-manifest/1"
CHUNK_OVERHEAD_BYTES = 2048  # H-CHUNK-LIMIT
DEFAULT_FETCH_BYTES = 65536  # H-FETCH
DEFAULT_QUERY_LIMIT = 100  # H-QUERY
DIGEST_ALGORITHMS = ("sha256", "sha512")  # H-CONTENT-DIGEST-ALG
# H-LOSS-RECORD, H3-LOSS-KINDS (fixture-informed): the dependency kinds tracked.
HOLD_DEPENDENCY = "evidence.hold"
MANIFEST_DEPENDENCY = "evidence.manifest_child"
TRACKED_DEPENDENCIES = [HOLD_DEPENDENCY, MANIFEST_DEPENDENCY]
CREDENTIAL_PARAM = re.compile(r"token|sig|signature|credential|password|secret|key|auth", re.I)


class EvidenceConfigError(Exception):
    pass


def parse_store_config(raw) -> dict:
    if not isinstance(raw, dict):
        raise EvidenceConfigError("evidence_store must be an object")
    unknown = set(raw) - {"corrupt", "unavailable", "staging_timeout_seconds", "deletion_delay_seconds"}
    if unknown:
        raise EvidenceConfigError(f"evidence_store: unknown members {sorted(unknown)}")
    for member in ("corrupt", "unavailable"):
        value = raw.get(member, [])
        if not isinstance(value, list) or not all(isinstance(v, str) for v in value):
            raise EvidenceConfigError(f"evidence_store.{member} must be an array of artifact IDs")
    timeout = raw.get("staging_timeout_seconds")
    if timeout is not None and (not E.is_int(timeout) or timeout < 1):
        raise EvidenceConfigError("evidence_store.staging_timeout_seconds must be an integer >= 1")
    delay = raw.get("deletion_delay_seconds", 0)
    if not E.is_int(delay) or delay < 0:
        raise EvidenceConfigError("evidence_store.deletion_delay_seconds must be an integer >= 0")
    return raw


def artifact_subject(aid: str) -> dict:
    return {"kind": ARTIFACT_KIND, "id": aid}


def hold_subject(hid: str) -> dict:
    return {"kind": HOLD_KIND, "id": hid}


# ------------------------------------------------------------- validation

def reference(v, path: str) -> dict:
    """schemas/evidence/1/common.schema.json#/$defs/reference"""
    E.closed(v, path, ("artifact", "digest"), ("provider",))
    if "provider" in v:
        E.identifier(v["provider"], ptr(path, "provider"))
    E.subject(v["artifact"], ptr(path, "artifact"), ARTIFACT_KIND)
    E.digest_string(v["digest"], ptr(path, "digest"))
    return v


def _source(v, path: str) -> None:
    E.closed(v, path, ("kind", "id"))
    E.string(v["kind"], ptr(path, "kind"), 1, 64)
    E.string(v["id"], ptr(path, "id"), 1, 256)


def _coverage(v, path: str) -> None:
    E.closed(v, path, ("completeness",), ("covered", "gaps"))
    if v["completeness"] not in ("complete", "partial", "unknown"):
        raise Invalid(ptr(path, "completeness"), "must be complete, partial or unknown")
    for member in ("covered", "gaps"):
        if member in v:
            E.array(v[member], ptr(path, member), max_items=256)
            for idx, item in enumerate(v[member]):
                E.string(item, ptr(ptr(path, member), idx), 1, 512)


def _capture(v, path: str) -> None:
    E.closed(v, path, ("captured_at",), ("uncertainty", "anchors"))
    E.instant(v["captured_at"], ptr(path, "captured_at"))
    if "uncertainty" in v:
        u = E.closed(v["uncertainty"], ptr(path, "uncertainty"), ("not_before", "not_after"))
        E.instant(u["not_before"], ptr(ptr(path, "uncertainty"), "not_before"))
        E.instant(u["not_after"], ptr(ptr(path, "uncertainty"), "not_after"))
        if u["not_before"] > u["not_after"]:
            raise Invalid(ptr(ptr(path, "uncertainty"), "not_after"), "not_after precedes not_before")
    if "anchors" in v:
        E.array(v["anchors"], ptr(path, "anchors"), max_items=64)
        for idx, anchor in enumerate(v["anchors"]):
            p = ptr(ptr(path, "anchors"), idx)
            E.closed(anchor, p, ("kind", "id"))
            E.string(anchor["kind"], ptr(p, "kind"), 1, 64)
            E.string(anchor["id"], ptr(p, "id"), 1, 256)


def locator_has_credentials(locator: str) -> bool:
    """H-PREPARE-VALIDATION: user information, or a query parameter that
    looks like a token, signature or key."""
    try:
        parts = urlsplit(locator)
    except ValueError:
        return False
    if "@" in (parts.netloc or ""):
        return True
    for name, _ in parse_qsl(parts.query, keep_blank_values=True):
        if CREDENTIAL_PARAM.search(name) or name.lower().startswith("x-amz-"):
            return True
    return False


def _only_primary_precondition(env: dict) -> dict:
    if len(env["preconditions"]) != 1:
        raise Invalid("/preconditions", "exactly one precondition is required")
    if env["preconditions"][0]["subject"] != env["subject"]:
        raise Invalid("/preconditions/0/subject", "the precondition must name the command's subject")
    return env["preconditions"][0]


def decode_base64(text: str, path: str) -> bytes:
    if not isinstance(text, str) or not re.fullmatch(r"[A-Za-z0-9+/]*={0,2}", text):
        raise Invalid(path, "must be base64")
    try:
        return base64.b64decode(text, validate=True)
    except (binascii.Error, ValueError):
        raise Invalid(path, "must be padded base64") from None


def parse_manifest(data: bytes):
    """EVIDENCE 7: the manifest content, or None when it is not a valid
    manifest of a supported format (H-MANIFEST)."""
    try:
        doc = V.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, V.ParseError):
        return None
    try:
        E.closed(doc, "", ("format", "children"))
        if doc["format"] != MANIFEST_FORMAT:
            return None
        E.array(doc["children"], "/children", max_items=1024)
        for idx, child in enumerate(doc["children"]):
            p = ptr("/children", idx)
            E.closed(child, p, ("role", "evidence", "required"))
            E.string(child["role"], ptr(p, "role"), 1, 128)
            reference(child["evidence"], ptr(p, "evidence"))
            if not isinstance(child["required"], bool):
                return None
    except Invalid:
        return None
    return doc


# ------------------------------------------------------------------ profile

class Evidence:
    def __init__(self, provider, cfg: dict):
        self.p = provider
        self.corrupt = set(cfg.get("corrupt", []))
        self.unavailable = set(cfg.get("unavailable", []))
        self.staging_timeout = cfg.get("staging_timeout_seconds")
        self.deletion_delay = cfg.get("deletion_delay_seconds", 0)

    @property
    def store(self):
        return self.p.store

    def now(self) -> str:
        return self.p.clock.now()

    def load(self, aid: str):
        return self.store.get_json(ARTIFACT_KIND, aid)

    def load_hold(self, hid: str):
        return self.store.get_json(HOLD_KIND, hid)

    def _emit(self, changes, etype: str, subject: dict, revision: int, payload: dict) -> None:
        if changes is not None:
            changes.append((etype, subject, revision, payload))
        else:
            self.store.append_event({"type": etype, "subject": subject, "revision": revision, "origin": "provider",
                                     "caused_by": [], "recorded_at": self.now(), "payload": payload})

    # ------------------------------------------------------ chunk sizing
    def chunk_limit(self) -> int:
        """H-CHUNK-LIMIT: the largest decoded size whose base64 form, with the
        declared overhead, fits the payload, frame and string limits."""
        lim = self.p.limits
        encoded = min(lim["max_payload_bytes"] - CHUNK_OVERHEAD_BYTES,
                      lim["max_frame_bytes"] - CHUNK_OVERHEAD_BYTES,
                      lim["max_string_bytes"])
        return max(0, encoded // 4) * 3

    # ------------------------------------------------------- validators
    def v_prepare(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", ARTIFACT_KIND)
        if _only_primary_precondition(env)["revision"] != 0:
            raise Invalid("/preconditions/0/revision", "evidence.upload.prepare creates the artifact: revision must be 0")
        d = env["payload"]
        E.closed(d, "/payload", ("digest", "size", "media_type", "producer", "source", "scope", "capture", "coverage",
                                 "retention_class"), ("work", "locator"))
        E.digest_string(d["digest"], "/payload/digest")
        E.integer(d["size"], "/payload/size", 0)
        E.string(d["media_type"], "/payload/media_type", 1, 128)
        producer = E.closed(d["producer"], "/payload/producer", (), ("principal", "producer_id"))
        if "principal" in producer:
            E.identifier(producer["principal"], "/payload/producer/principal")
            if producer["principal"] != self.p.principal:
                # EVIDENCE 3 (EVD-4): always the session principal.
                raise Invalid("/payload/producer/principal", "the producer is the session principal")
        if "producer_id" in producer:
            E.string(producer["producer_id"], "/payload/producer/producer_id", 1, 128)
        _source(d["source"], "/payload/source")
        E.string(d["scope"], "/payload/scope", 1, 256)
        _capture(d["capture"], "/payload/capture")
        _coverage(d["coverage"], "/payload/coverage")
        if d["source"]["kind"] == "terminal_output" and d["coverage"]["completeness"] == "complete":
            raise Invalid("/payload/coverage/completeness",
                          "terminal output never declares complete coverage of a tool trace")  # H-TERMINAL-COVERAGE
        if "work" in d:
            E.subject(d["work"], "/payload/work")
        E.string(d["retention_class"], "/payload/retention_class", 1, 64)
        if "locator" in d:
            E.string(d["locator"], "/payload/locator", 1, 1024)
            if locator_has_credentials(d["locator"]):
                raise Invalid("/payload/locator", "a locator must not carry credentials")

    def v_append(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", ARTIFACT_KIND)
        _only_primary_precondition(env)
        payload = E.closed(env["payload"], "/payload", ("offset", "data_base64"))
        E.integer(payload["offset"], "/payload/offset", 0)
        data = decode_base64(payload["data_base64"], "/payload/data_base64")
        limit = self.chunk_limit()
        if len(data) > limit:
            # EVIDENCE 4: limit_exceeded even if the frame fits (H-CHUNK-STEP).
            raise ProtocolError("limit_exceeded", {"limit": "chunk_limit", "maximum": limit})

    def v_simple(self, kind: str):
        def validator(env, method, features) -> None:
            E.command_envelope(env, method, "core.grants" in features)
            E.subject(env["subject"], "/subject", kind)
            E.closed(env["payload"], "/payload", ())
            _only_primary_precondition(env)
        return validator

    def v_hold(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", HOLD_KIND)
        if _only_primary_precondition(env)["revision"] != 0:
            raise Invalid("/preconditions/0/revision", "evidence.hold creates the hold: revision must be 0")
        payload = E.closed(env["payload"], "/payload", ("artifact", "holder_ref", "reason"), ("expires_at",))
        E.subject(payload["artifact"], "/payload/artifact", ARTIFACT_KIND)
        E.subject(payload["holder_ref"], "/payload/holder_ref")
        E.string(payload["reason"], "/payload/reason", 1, 256)
        if "expires_at" in payload:
            E.instant(payload["expires_at"], "/payload/expires_at")

    def v_purge(self, env, method, features) -> None:
        E.command_envelope(env, method, "core.grants" in features)
        E.subject(env["subject"], "/subject", ARTIFACT_KIND)
        payload = E.closed(env["payload"], "/payload", (), ("release_holds",))
        holds = payload.get("release_holds", [])
        E.array(holds, "/payload/release_holds", max_items=64, unique=True)
        for idx, hid in enumerate(holds):
            E.identifier(hid, ptr("/payload/release_holds", idx))
        # EVIDENCE 9 "Preconditions": the artifact and each named hold (H-PURGE).
        expected = [env["subject"]] + [hold_subject(h) for h in holds]
        named = [entry["subject"] for entry in env["preconditions"]]
        if len(named) != len(expected) or any(s not in named for s in expected):
            raise Invalid("/preconditions", "preconditions must name the artifact and each hold in release_holds")

    def v_inspect(self, env, method, features) -> None:
        E.query_envelope(env, method, "core.grants" in features)
        payload = E.closed(env["payload"], "/payload", ("artifact",))
        E.subject(payload["artifact"], "/payload/artifact", ARTIFACT_KIND)

    def v_query(self, env, method, features) -> None:
        E.query_envelope(env, method, "core.grants" in features)
        payload = E.closed(env["payload"], "/payload", (), ("filters", "limit", "cursor"))
        if "filters" in payload:
            f = E.closed(payload["filters"], "/payload/filters", (),
                         ("producer_principal", "media_type", "digest", "scope", "source", "work"))
            if "producer_principal" in f:
                E.identifier(f["producer_principal"], "/payload/filters/producer_principal")
            if "media_type" in f:
                E.string(f["media_type"], "/payload/filters/media_type", 1, 128)
            if "digest" in f:
                E.digest_string(f["digest"], "/payload/filters/digest")
            if "scope" in f:
                E.string(f["scope"], "/payload/filters/scope", 1, 256)
            if "source" in f:
                _source(f["source"], "/payload/filters/source")
            if "work" in f:
                E.subject(f["work"], "/payload/filters/work")
        if "limit" in payload:
            E.integer(payload["limit"], "/payload/limit", 1, 1000)
        if "cursor" in payload:
            E.string(payload["cursor"], "/payload/cursor", 1, 512)

    def v_fetch(self, env, method, features) -> None:
        E.query_envelope(env, method, "core.grants" in features)
        payload = E.closed(env["payload"], "/payload", ("artifact", "digest"), ("offset", "max_bytes"))
        E.subject(payload["artifact"], "/payload/artifact", ARTIFACT_KIND)
        E.digest_string(payload["digest"], "/payload/digest")
        if "offset" in payload:
            E.integer(payload["offset"], "/payload/offset", 0)
        if "max_bytes" in payload:
            E.integer(payload["max_bytes"], "/payload/max_bytes", 1, 16777216)

    # ---------------------------------------------------- authorization
    def hold_owner(self, hid: str):
        row = self.load_hold(hid)
        return row[1]["owner"] if row is not None else None

    def authorize(self, method: str, env: dict):
        """CORE 10 step 6 for Evidence operations (EVIDENCE 8-10)."""
        p = self.p
        payload = env["payload"]
        if method == "evidence.upload.prepare":
            auth = p.authorize_under(env, [("evidence.publish", env["subject"])])
            self._check_bound_work(auth, payload)
            return auth
        if method in ("evidence.upload.append", "evidence.seal", "evidence.upload.abandon"):
            return p.authorize_under(env, [("evidence.publish", env["subject"])])
        if method in ("evidence.inspect", "evidence.fetch"):
            return p.authorize_under(env, [("evidence.read", payload["artifact"])])
        if method == "evidence.query":
            return p.authorize_under(env, [("evidence.read", None)])
        if method == "evidence.hold":
            auth = p.authorize_under(env, [("evidence.hold", payload["artifact"])])
            if "expires_at" in payload and payload["expires_at"] <= self.now():
                raise ProtocolError("invalid_envelope", {"path": "/payload/expires_at",
                                                         "reason": "expires_at must be after the provider's current time"})
            return auth
        if method == "evidence.release":
            return self.authorize_with_holds(env, [], [env["subject"]["id"]])
        if method == "evidence.purge":
            return self.authorize_with_holds(env, [("evidence.purge", env["subject"])],
                                             list(payload.get("release_holds", [])))
        raise AssertionError(method)

    def authorize_with_holds(self, env: dict, needs: list, holds: list):
        """EVIDENCE 9: release authority for each hold: its owner, an authority
        principal, or a grant with evidence.release covering it (H-RELEASE-AUTH)."""
        p = self.p
        principal = p.principal
        owned = {h for h in holds if self.hold_owner(h) == principal}
        if env.get("grant") is None:
            if p.is_authority or (not needs and len(owned) == len(holds)):
                return p.make_auth(None)
            raise denied("grant_required")
        record = p.usable_grant(env)
        rights = [r for r, _ in needs] + ["evidence.release" for h in holds if h not in owned]
        subjects = [s for _, s in needs] + [hold_subject(h) for h in holds if h not in owned]
        if any(r not in record["rights"] for r in rights):
            raise denied("right_missing")
        if any(not G.grant_covers(record, s) for s in subjects):
            raise denied("out_of_scope")
        return p.make_auth(record)

    @staticmethod
    def _check_bound_work(auth, payload: dict) -> None:
        """EVIDENCE 10 "Bound work" (H-BOUND-WORK)."""
        grant = auth.grant
        if grant is None:
            return
        work = [r for r in grant["resources"] if r["kind"] not in (ARTIFACT_KIND, HOLD_KIND)]
        if not work:
            return
        subject = payload.get("work")
        if subject is None or not any(G.covers(r, subject) for r in work):
            raise denied("binding_violation")

    # ------------------------------------------------------ visibility
    def hold_visible(self, auth, hid: str, hold: dict) -> bool:
        """H-HOLD-VISIBILITY, widened by H3-HOLD-VISIBILITY (fixture-informed):
        a reader of the held artifact sees its holds."""
        if auth.grant is None:
            return True
        if hold["owner"] == self.p.principal:
            return True
        if self.artifact_readable(auth, hold["artifact"]):
            return True
        return "evidence.read" in auth.grant["rights"] and G.grant_covers(auth.grant, hold_subject(hid))

    def artifact_readable(self, auth, aid: str) -> bool:
        if auth.grant is None:
            return True
        return "evidence.read" in auth.grant["rights"] and G.grant_covers(auth.grant, artifact_subject(aid))

    # ---------------------------------------------------- availability
    def blob(self, aid: str):
        return self.store.blob(aid)

    def availability(self, aid: str, rec: dict) -> dict:
        """H-AVAILABILITY-STATES: observed at the read, never recorded."""
        if rec["state"] == "staged":
            return {"state": "available"}  # H3-STAGED-AVAILABILITY (fixture-informed)
        if rec["state"] == "abandoned":
            return {"state": "unavailable", "reason": rec.get("abandoned_reason", "abandoned")}
        purge = rec.get("purge")
        if purge is not None:
            return {"state": "purged"} if "confirmed_at" in purge else {"state": "purge_pending"}
        if aid in self.unavailable:
            return {"state": "unavailable", "reason": "store_unavailable"}
        data = self.blob(aid)
        if aid in self.corrupt or data is None or V.digest(self._algorithm(rec), data) != rec["descriptor"]["digest"]:
            # EVIDENCE 5 "Integrity on read": never served.
            return {"state": "unavailable", "reason": "integrity_failed"}
        return {"state": "available"}

    @staticmethod
    def _algorithm(rec: dict) -> str:
        return rec["descriptor"]["digest"].split(":", 1)[0]

    # ------------------------------------------------------- commands
    def check_preconditions(self, env, auth) -> None:
        self.p.check_preconditions(env, auth)

    def op_prepare(self, env, auth):
        self.check_preconditions(env, auth)
        d = env["payload"]
        algorithm = d["digest"].split(":", 1)[0]
        if algorithm not in DIGEST_ALGORITHMS:
            raise ProtocolError("unsupported_digest_algorithm", {"algorithm": algorithm,
                                                                 "supported": list(DIGEST_ALGORITHMS)})
        aid = env["subject"]["id"]
        descriptor = self._descriptor(d)
        rec = {"descriptor": descriptor, "state": "staged", "received": 0, "staged_at": self.now(), "holds": []}
        self.store.put_json(ARTIFACT_KIND, aid, 1, rec)
        self.store.set_blob(aid, b"")
        limit = self.chunk_limit()
        outcome = {"artifact": artifact_subject(aid), "state": "staged", "received": 0, "chunk_limit": limit,
                   "chunk_overhead_bytes": CHUNK_OVERHEAD_BYTES}
        return 1, outcome, [("evidence.artifact.staged", artifact_subject(aid), 1, {"descriptor": descriptor})]

    def _descriptor(self, d: dict) -> dict:
        producer = {"principal": self.p.principal}
        if "producer_id" in d["producer"]:
            producer["producer_id"] = d["producer"]["producer_id"]
        descriptor = {k: v for k, v in d.items() if k != "producer"}
        descriptor["producer"] = producer
        return descriptor

    def _existing(self, env, states):
        """Preconditions first, then the upload the command acts on
        (H-EVD-STEP7-ORDER)."""
        aid = env["subject"]["id"]
        row = self.load(aid)
        if row is None or row[1]["state"] not in states:
            raise ProtocolError("not_found")
        return aid, row[0], row[1]

    def op_append(self, env, auth):
        self.check_preconditions(env, auth)
        aid, revision, rec = self._existing(env, ("staged",))
        payload = env["payload"]
        data = base64.b64decode(payload["data_base64"])
        if payload["offset"] != rec["received"]:
            raise ProtocolError("upload_offset_mismatch", {"received": rec["received"]})
        size = rec["descriptor"]["size"]
        if rec["received"] + len(data) > size:
            raise ProtocolError("upload_size_exceeded", {"received": rec["received"], "size": size})
        stored = self.blob(aid) or b""
        self.store.set_blob(aid, stored + data)
        rec["received"] += len(data)
        revision += 1
        self.store.put_json(ARTIFACT_KIND, aid, revision, rec)
        return revision, {"artifact": artifact_subject(aid), "received": rec["received"]}, [
            ("evidence.artifact.appended", artifact_subject(aid), revision, {"received": rec["received"]})]

    def op_seal(self, env, auth):
        self.check_preconditions(env, auth)
        aid, revision, rec = self._existing(env, ("staged", "sealed"))
        d = rec["descriptor"]
        if rec["state"] == "sealed":
            # EVIDENCE 4 "Idempotent": unchanged revision, no event.
            return revision, {"artifact": artifact_subject(aid), "state": "sealed", "digest": d["digest"],
                              "size": d["size"], "already_sealed": True}, []
        if rec["received"] < d["size"]:
            raise ProtocolError("upload_incomplete", {"received": rec["received"], "size": d["size"]})
        data = self.blob(aid) or b""
        computed = V.digest(self._algorithm(rec), data)
        if computed != d["digest"]:
            raise ProtocolError("content_digest_mismatch", {"computed": computed})
        if d["media_type"] == MANIFEST_MEDIA_TYPE and parse_manifest(data) is None:
            raise ProtocolError("invalid_envelope", {"path": "/payload",
                                                     "reason": "the content is not a valid manifest of a supported format"})
        rec["state"] = "sealed"
        rec.pop("received", None)
        revision += 1
        self.store.put_json(ARTIFACT_KIND, aid, revision, rec)
        return revision, {"artifact": artifact_subject(aid), "state": "sealed", "digest": d["digest"],
                          "size": d["size"], "already_sealed": False}, [
            ("evidence.artifact.sealed", artifact_subject(aid), revision, {"digest": d["digest"], "size": d["size"]})]

    def op_abandon(self, env, auth):
        self.check_preconditions(env, auth)
        aid, revision, rec = self._existing(env, ("staged",))
        revision, changes = self._abandon(aid, revision, rec, "abandoned_by_producer", [])  # H3-ABANDON-REASON
        return revision, {"artifact": artifact_subject(aid), "state": "abandoned"}, changes

    def _abandon(self, aid, revision, rec, reason, changes):
        rec["state"] = "abandoned"
        rec["abandoned_reason"] = reason
        rec.pop("received", None)
        self.store.delete_blob(aid)  # staged bytes discarded; the descriptor stays as a tombstone
        revision += 1
        self.store.put_json(ARTIFACT_KIND, aid, revision, rec)
        self._emit(changes, "evidence.artifact.abandoned", artifact_subject(aid), revision, {"reason": reason})
        return revision, changes

    def op_hold(self, env, auth):
        self.check_preconditions(env, auth)
        payload = env["payload"]
        aid = payload["artifact"]["id"]
        row = self.load(aid)
        if row is None or row[1].get("purge") is not None:
            raise ProtocolError("not_found")  # H-HOLD-PLACE
        hid = env["subject"]["id"]
        hold = {"artifact": aid, "holder_ref": payload["holder_ref"], "reason": payload["reason"],
                "owner": self.p.principal, "state": "active"}
        if "expires_at" in payload:
            hold["expires_at"] = payload["expires_at"]
        self.store.put_json(HOLD_KIND, hid, 1, hold)
        revision, rec = row
        rec.setdefault("holds", []).append(hid)
        self.store.put_json(ARTIFACT_KIND, aid, revision, rec)  # bookkeeping only; the revision is unchanged
        return 1, {"hold": hold_subject(hid), "state": "active", "owner": self.p.principal}, [
            ("evidence.hold.placed", hold_subject(hid), 1, {"artifact": artifact_subject(aid),
                                                             "holder_ref": payload["holder_ref"]})]

    def op_release(self, env, auth):
        self.check_preconditions(env, auth)
        hid = env["subject"]["id"]
        row = self.load_hold(hid)
        if row is None or row[1]["state"] != "active":
            raise ProtocolError("not_found")  # H-RELEASE-AUTH
        changes = []
        self._release(hid, row[0], row[1], changes)
        return row[0] + 1, {"hold": hold_subject(hid), "state": "released"}, changes

    def _release(self, hid, revision, hold, changes) -> int:
        hold["state"] = "released"
        revision += 1
        self.store.put_json(HOLD_KIND, hid, revision, hold)
        self._emit(changes, "evidence.hold.released", hold_subject(hid), revision,
                   {"artifact": artifact_subject(hold["artifact"]), "holder_ref": hold["holder_ref"]})
        return revision

    def op_purge(self, env, auth):
        self.check_preconditions(env, auth)
        aid = env["subject"]["id"]
        row = self.load(aid)
        if row is None or row[1]["state"] != "sealed":
            raise ProtocolError("not_found")  # H-PURGE: no sealed bytes to lose
        revision, rec = row
        if rec.get("purge") is not None:
            # EVIDENCE 9 "Repeated purge".
            state = "purged" if "confirmed_at" in rec["purge"] else "purge_pending"
            return revision, {"artifact": artifact_subject(aid), "availability": state, "released_holds": []}, []
        named = list(env["payload"].get("release_holds", []))
        holds = {}
        for hid in named:
            hrow = self.load_hold(hid)
            if hrow is None or hrow[1]["artifact"] != aid or hrow[1]["state"] != "active":
                raise ProtocolError("not_found")
            holds[hid] = hrow
        blocking = [hid for hid in sorted(rec.get("holds", []))
                    if hid not in holds and self.load_hold(hid)[1]["state"] == "active"]
        if blocking:
            # H3-HOLD-ACTIVE-DETAILS (fixture-informed): hold IDs.
            visible = [h for h in blocking if self.hold_visible(auth, h, self.load_hold(h)[1])]
            raise ProtocolError("hold_active", {"holds": visible, "filtered": len(visible) < len(blocking)})
        now = self.now()
        changes = []
        revision += 1
        # CORE 16.3: the primary subject first (H-PURGE).
        rec["purge"] = {"requested_at": now, "released_holds": [hold_subject(h) for h in named],
                        "affected": self._dependencies(aid, rec)}
        self.store.put_json(ARTIFACT_KIND, aid, revision, rec)
        changes.append(("evidence.artifact.purge_requested", artifact_subject(aid), revision, {"requested_at": now}))
        availability = "purge_pending"
        if not self.deletion_delay:
            # H3-IMMEDIATE-DELETION (fixture-informed): without a deletion
            # delay the store confirms deletion in the purge's own transaction.
            rec["purge"]["confirmed_at"] = now
            self.store.delete_blob(aid)
            revision += 1
            self.store.put_json(ARTIFACT_KIND, aid, revision, rec)
            changes.append(("evidence.artifact.purged", artifact_subject(aid), revision, {"confirmed_at": now}))
            availability = "purged"
        for hid in named:
            hrev, hold = holds[hid]
            self._release(hid, hrev, hold, changes)
        return revision, {"artifact": artifact_subject(aid), "availability": availability,
                          "released_holds": [hold_subject(h) for h in named]}, changes

    def _dependencies(self, aid: str, rec: dict) -> list:
        """H-LOSS-RECORD: holds, manifests listing the artifact, packets citing it."""
        affected = [{"kind": HOLD_DEPENDENCY, "subject": hold_subject(h)} for h in sorted(rec.get("holds", []))]
        for mid in sorted(self.store.ids_of(ARTIFACT_KIND)):
            if mid == aid:
                continue
            mrow = self.load(mid)
            mrec = mrow[1]
            if mrec["state"] != "sealed" or mrec["descriptor"]["media_type"] != MANIFEST_MEDIA_TYPE:
                continue
            manifest = parse_manifest(self.blob(mid) or b"")
            if manifest is None:
                continue
            if any(c["evidence"]["artifact"]["id"] == aid and c["evidence"].get("provider", self.p.provider_id) ==
                   self.p.provider_id for c in manifest["children"]):
                affected.append({"kind": MANIFEST_DEPENDENCY, "subject": artifact_subject(mid)})
        return affected

    # -------------------------------------------------------- queries
    def op_inspect(self, env, auth) -> dict:
        aid = env["payload"]["artifact"]["id"]
        row = self.load(aid)
        if row is None:
            raise ProtocolError("not_found")
        revision, rec = row
        view = {"artifact": artifact_subject(aid), "revision": revision, "descriptor": rec["descriptor"],
                "state": rec["state"], "availability": self.availability(aid, rec)}
        if rec["state"] == "staged":
            view["received"] = rec["received"]
        if rec["state"] == "abandoned":
            view["abandoned_reason"] = rec["abandoned_reason"]
        holds = []
        for hid in sorted(rec.get("holds", [])):  # H3-HOLD-ORDER (fixture-informed): hold ID order
            hrow = self.load_hold(hid)
            if hrow is None or not self.hold_visible(auth, hid, hrow[1]):
                continue
            holds.append({"hold": hold_subject(hid), "revision": hrow[0], "state": hrow[1]["state"],
                          "owner": hrow[1]["owner"], "holder_ref": hrow[1]["holder_ref"]})
        view["holds"] = holds
        if rec.get("purge") is not None:
            view["loss"] = self._loss_view(aid, rec, auth)
        features = self.p.session.features()
        if ("evidence.manifests" in features and rec["state"] == "sealed"
                and rec["descriptor"]["media_type"] == MANIFEST_MEDIA_TYPE):
            manifest = parse_manifest(self.blob(aid) or b"")
            if manifest is not None:
                view["completeness"] = self._completeness(manifest, auth)
        return view

    def _dependency_visible(self, auth, entry: dict) -> bool:
        kind, subject = entry["kind"], entry["subject"]
        if kind == HOLD_DEPENDENCY:
            row = self.load_hold(subject["id"])
            return row is not None and self.hold_visible(auth, subject["id"], row[1])
        if kind == MANIFEST_DEPENDENCY:
            return self.artifact_readable(auth, subject["id"])
        return False

    def _loss_view(self, aid: str, rec: dict, auth) -> dict:
        purge = rec["purge"]
        affected = [e for e in purge["affected"] if self._dependency_visible(auth, e)]
        released = [h for h in purge["released_holds"]
                    if self._dependency_visible(auth, {"kind": HOLD_DEPENDENCY, "subject": h})]
        filtered = len(affected) < len(purge["affected"]) or len(released) < len(purge["released_holds"])
        loss = {"artifact": artifact_subject(aid), "digest": rec["descriptor"]["digest"],
                "requested_at": purge["requested_at"]}
        if "confirmed_at" in purge:
            loss["confirmed_at"] = purge["confirmed_at"]
        loss.update(released_holds=released, affected=affected,
                    coverage={"tracked": list(TRACKED_DEPENDENCIES), "filtered": filtered})
        return loss

    def child_state(self, child: dict, auth) -> str:
        """H-MANIFEST."""
        ref = child["evidence"]
        if ref.get("provider", self.p.provider_id) != self.p.provider_id:
            return "unverified"
        cid = ref["artifact"]["id"]
        if not self.artifact_readable(auth, cid):
            return "withheld"
        row = self.load(cid)
        if row is None:
            return "missing"
        rec = row[1]
        if rec["state"] != "sealed" or rec.get("purge") is not None or rec["descriptor"]["digest"] != ref["digest"]:
            return "missing"
        if self.availability(cid, rec)["state"] != "available":
            return "unverified"
        return "present"

    def _completeness(self, manifest: dict, auth) -> dict:
        children = []
        for child in manifest["children"]:
            children.append({"role": child["role"], "evidence": child["evidence"], "required": child["required"],
                             "state": self.child_state(child, auth)})
        required = [c for c in children if c["required"]]
        if all(c["state"] == "present" for c in required):
            state = "complete"
        elif any(c["state"] == "missing" for c in required):
            state = "incomplete"
        else:
            state = "undetermined"
        return {"state": state, "children": children}

    def op_query(self, env, auth) -> dict:
        payload = env["payload"]
        filters = payload.get("filters", {})
        limit = payload.get("limit", DEFAULT_QUERY_LIMIT)
        # H3-QUERY-ORDER (fixture-informed): artifact ID order; the cursor
        # names the last artifact returned.
        ids = sorted(self.store.ids_of(ARTIFACT_KIND))
        after = None
        if "cursor" in payload:
            m = re.fullmatch(r"eq1\.([A-Za-z0-9_-]+=*)", payload["cursor"])
            try:
                after = base64.urlsafe_b64decode(m.group(1)).decode("utf-8") if m else None
            except (binascii.Error, ValueError):
                after = None
            if after is None:
                raise ProtocolError("invalid_cursor", {"reason": "malformed"})
        items, more, last = [], False, None
        for aid in ids:
            if after is not None and aid <= after:
                continue
            rec = self.load(aid)[1]
            if not self.artifact_readable(auth, aid) or not self._matches(rec, filters):
                continue
            if len(items) == limit:
                more = True
                break
            last = aid
            items.append({"artifact": artifact_subject(aid), "digest": rec["descriptor"]["digest"],
                          "state": rec["state"], "availability": self.availability(aid, rec)})
        grant = auth.grant
        filtered = grant is not None and not ("evidence.read" in grant["rights"] and any(
            r["kind"] == ARTIFACT_KIND and "id" not in r and "id_prefix" not in r for r in grant["resources"]))
        result = {"items": items, "filtered": filtered}
        if more:
            result["next_cursor"] = "eq1." + base64.urlsafe_b64encode(last.encode("utf-8")).decode("ascii")
        return result

    @staticmethod
    def _matches(rec: dict, f: dict) -> bool:
        d = rec["descriptor"]
        if "producer_principal" in f and d["producer"]["principal"] != f["producer_principal"]:
            return False
        for member in ("media_type", "digest", "scope", "source", "work"):
            if member in f and d.get(member) != f[member]:
                return False
        return True

    def op_fetch(self, env, auth) -> dict:
        payload = env["payload"]
        aid = payload["artifact"]["id"]
        row = self.load(aid)
        if row is None or row[1]["state"] != "sealed":
            raise ProtocolError("not_found")  # EVIDENCE 5 "Not sealed"
        rec = row[1]
        d = rec["descriptor"]
        if payload["digest"] != d["digest"]:
            raise ProtocolError("artifact_digest_mismatch")
        offset = payload.get("offset", 0)
        availability = self.availability(aid, rec)
        base = {"artifact": artifact_subject(aid), "digest": d["digest"], "availability": availability,
                "offset": offset, "size": d["size"]}
        if availability["state"] != "available" or offset >= d["size"]:
            return dict(base, data_base64="", next_offset=offset)
        data = (self.blob(aid) or b"")[offset: offset + payload.get("max_bytes", DEFAULT_FETCH_BYTES)]
        return self.p.fit_bytes(lambda piece: dict(base, data_base64=base64.b64encode(piece).decode("ascii"),
                                                   next_offset=offset + len(piece)), data)

    # ----------------------------------------------------------- ticks
    def tick(self) -> bool:
        """Staging timeouts, hold expiry and deletion confirmation, each in
        its own provider transaction."""
        progressed = False
        now = self.now()
        for aid in self.store.ids_of(ARTIFACT_KIND):
            row = self.load(aid)
            rec = row[1]
            if (rec["state"] == "staged" and self.staging_timeout is not None
                    and now >= C.add_seconds(rec["staged_at"], self.staging_timeout)):
                self._in_tx(lambda aid=aid: self._expire_staging(aid))
                progressed = True
            elif (rec.get("purge") is not None and "confirmed_at" not in rec["purge"]
                    and now >= C.add_seconds(rec["purge"]["requested_at"], self.deletion_delay)):
                self._in_tx(lambda aid=aid: self._confirm_deletion(aid))
                progressed = True
        for hid in self.store.ids_of(HOLD_KIND):
            hrow = self.load_hold(hid)
            hold = hrow[1]
            if hold["state"] == "active" and "expires_at" in hold and now >= hold["expires_at"]:
                self._in_tx(lambda hid=hid: self._expire_hold(hid))  # H-HOLD-EXPIRY
                progressed = True
        return progressed

    def _in_tx(self, fn) -> None:
        st = self.store
        st.begin()
        try:
            fn()
            st.commit()
        except BaseException:
            st.rollback()
            raise

    def _expire_staging(self, aid: str) -> None:
        revision, rec = self.load(aid)
        self._abandon(aid, revision, rec, "staging_expired", None)

    def _confirm_deletion(self, aid: str) -> None:
        revision, rec = self.load(aid)
        now = self.now()
        rec["purge"]["confirmed_at"] = now
        self.store.delete_blob(aid)
        revision += 1
        self.store.put_json(ARTIFACT_KIND, aid, revision, rec)
        self._emit(None, "evidence.artifact.purged", artifact_subject(aid), revision, {"confirmed_at": now})

    def _expire_hold(self, hid: str) -> None:
        revision, hold = self.load_hold(hid)
        self._release(hid, revision, hold, None)

    # ------------------------------------------- packets (CONTEXT 5, M4-Q2)
    def seal_internal(self, aid: str, data: bytes, media_type: str, source: dict, scope: str, coverage: dict,
                      work: dict | None = None) -> str:
        """Seal bytes this provider produced itself as an ordinary artifact,
        with provider-origin staged, appended and sealed events. Call inside
        a transaction. Returns the digest."""
        digest = V.digest("sha256", data)
        descriptor = {"digest": digest, "size": len(data), "media_type": media_type,
                      "source": source, "scope": scope,
                      "capture": {"captured_at": self.now(), "anchors": []}, "coverage": coverage,
                      "retention_class": "default",
                      # H3-PACKET-PRODUCER (fixture-informed): the provider itself produced these bytes.
                      "producer": {"principal": self.p.provider_id}}
        if work is not None:
            descriptor["work"] = work
        subject = artifact_subject(aid)
        self.store.set_blob(aid, data)
        self._emit(None, "evidence.artifact.staged", subject, 1, {"descriptor": descriptor})
        self._emit(None, "evidence.artifact.appended", subject, 2, {"received": len(data)})
        self._emit(None, "evidence.artifact.sealed", subject, 3, {"digest": digest, "size": len(data)})
        self.store.put_json(ARTIFACT_KIND, aid, 3, {"descriptor": descriptor, "state": "sealed",
                                                    "staged_at": self.now(), "holds": []})
        return digest
