"""Closed-object and type validation of Core and core-test envelopes.

Hand-written from schemas/core/1/*.json, schemas/core-test/1/*.json and
CORE.md sections 4, 5, 13 (no JSON Schema library). Every violation raises
``Invalid(path, reason)``; ``path`` is a JSON Pointer relative to the
JSON-RPC ``params`` object.
"""

from __future__ import annotations

import re

from valuedomain import DIGEST_GRAMMAR, KNOWN_LENGTHS, MAX_SAFE

IDENTIFIER = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:~-]{0,127}")
DOTTED_NAME = re.compile(r"[a-z][a-z0-9-]*(?:\.[a-z][a-z0-9_-]*)+")
PROFILE_NAME = re.compile(r"[a-z][a-z0-9-]{0,63}")
EXTENSION_KEY = re.compile(r"[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+/[a-z][a-z0-9_.-]{0,63}")

AUTHORITY_SUBJECT = {"kind": "core-test.authority", "id": "core-test"}
TEST_SUBJECT_KIND = "core-test.subject"


class Invalid(Exception):
    def __init__(self, path: str, reason: str):
        super().__init__(f"{path}: {reason}")
        self.path = path
        self.reason = reason


def _esc(token: str) -> str:
    return token.replace("~", "~0").replace("/", "~1")


def ptr(base: str, token) -> str:
    return f"{base}/{_esc(str(token))}"


# ------------------------------------------------------------ primitives

def is_int(v) -> bool:
    return isinstance(v, int) and not isinstance(v, bool)


def closed(obj, path: str, required: tuple, optional: tuple = ()) -> dict:
    if not isinstance(obj, dict):
        raise Invalid(path, "must be an object")
    allowed = set(required) | set(optional)
    unknown = sorted(k for k in obj if k not in allowed)
    if unknown:
        raise Invalid(ptr(path, unknown[0]), "unknown field")
    for k in required:
        if k not in obj:
            raise Invalid(ptr(path, k), "required field missing")
    return obj


def string(v, path: str, min_len: int = 0, max_len: int | None = None) -> str:
    if not isinstance(v, str):
        raise Invalid(path, "must be a string")
    # JSON Schema minLength/maxLength count code points.
    if len(v) < min_len or (max_len is not None and len(v) > max_len):
        raise Invalid(path, "string length out of range")
    return v


def pattern(v, path: str, regex: re.Pattern, what: str, max_len: int | None = None) -> str:
    if not isinstance(v, str) or not regex.fullmatch(v) or (max_len is not None and len(v) > max_len):
        raise Invalid(path, f"must be a valid {what}")
    return v


def identifier(v, path: str) -> str:
    return pattern(v, path, IDENTIFIER, "identifier")


def dotted_name(v, path: str, what: str = "dotted name") -> str:
    return pattern(v, path, DOTTED_NAME, what, 128)


def integer(v, path: str, minimum: int = 0, maximum: int = MAX_SAFE) -> int:
    if not is_int(v):
        raise Invalid(path, "must be an integer")
    if v < minimum or v > maximum:
        raise Invalid(path, "integer out of range")
    return v


def array(v, path: str, min_items: int = 0, max_items: int | None = None, unique: bool = False) -> list:
    if not isinstance(v, list):
        raise Invalid(path, "must be an array")
    if len(v) < min_items or (max_items is not None and len(v) > max_items):
        raise Invalid(path, "array length out of range")
    if unique:
        seen = set()
        for idx, item in enumerate(v):
            key = _hashable(item)
            if key in seen:
                raise Invalid(ptr(path, idx), "duplicate array item")
            seen.add(key)
    return v


def _hashable(v):
    # JSON Schema uniqueItems: 1 and true are different values.
    if isinstance(v, bool):
        return ("b", v)
    if isinstance(v, (int, str)) or v is None:
        return (type(v).__name__, v)
    if isinstance(v, list):
        return ("a", tuple(_hashable(x) for x in v))
    return ("o", tuple(sorted((k, _hashable(x)) for k, x in v.items())))


def subject(v, path: str, kind_const: str | None = None, id_const: str | None = None) -> dict:
    closed(v, path, ("kind", "id"))
    if kind_const is not None:
        if v["kind"] != kind_const:
            raise Invalid(ptr(path, "kind"), f"must be {kind_const}")
    else:
        dotted_name(v["kind"], ptr(path, "kind"), "subject kind")
    if id_const is not None:
        if v["id"] != id_const:
            raise Invalid(ptr(path, "id"), f"must be {id_const}")
    else:
        identifier(v["id"], ptr(path, "id"))
    return v


def digest_string(v, path: str) -> str:
    if not isinstance(v, str) or len(v) > 256 or not DIGEST_GRAMMAR.fullmatch(v):
        raise Invalid(path, "must be a digest string <algorithm>:<lowercase hex>")
    algorithm, hexpart = v.split(":", 1)
    expected = KNOWN_LENGTHS.get(algorithm)
    if expected is not None and len(hexpart) != expected:
        raise Invalid(path, f"{algorithm} digest must have {expected} hex characters")
    return v


def requires_and_extensions(env: dict) -> None:
    """CORE 5.1: requires entries unique; a slash entry must be in extensions."""
    exts = env.get("extensions")
    if "extensions" in env:
        if not isinstance(exts, dict):
            raise Invalid("/extensions", "must be an object")
        if len(exts) > 64:
            raise Invalid("/extensions", "too many extensions")
        for k in exts:
            if not EXTENSION_KEY.fullmatch(k):
                raise Invalid(ptr("/extensions", k), "extension key must be <domain>/<name>")
    if "requires" in env:
        req = array(env["requires"], "/requires", max_items=64, unique=True)
        for idx, item in enumerate(req):
            p = ptr("/requires", idx)
            if not isinstance(item, str):
                raise Invalid(p, "must be a feature name or extension key")
            if "/" in item:
                if not EXTENSION_KEY.fullmatch(item):
                    raise Invalid(p, "invalid extension key")
                if not isinstance(exts, dict) or item not in exts:
                    raise Invalid(p, "required extension is not present in extensions")
            elif not (DOTTED_NAME.fullmatch(item) and len(item) <= 128):
                raise Invalid(p, "invalid feature name")


# ------------------------------------------------------------- envelopes

QUERY_FIELDS = ("operation", "message_id", "payload")
QUERY_OPTIONAL = ("requires", "extensions")
COMMAND_FIELDS = ("operation", "message_id", "command_id", "dedupe_generation", "subject",
                  "preconditions", "requires", "command_digest", "payload")
COMMAND_OPTIONAL = ("authority_epoch", "correlation", "caused_by", "extensions")
# CORE 5: `grant` is "Allowed only when feature core.grants is negotiated". In a
# session without it the field is not defined, so CORE 5.1's closed-object rule
# makes it invalid_envelope (DIVERGENCES.md E-GRANT-FIELD).
GRANT_FIELD = ("grant",)


def _operation_matches(env: dict, method: str) -> None:
    # STREAM 3: "The method ... MUST equal the envelope's operation."
    if not isinstance(env, dict):
        raise Invalid("", "params must be an object")
    if "operation" in env and env["operation"] != method:
        raise Invalid("/operation", "operation must equal the JSON-RPC method")


def query_envelope(env: dict, method: str, allow_grant: bool = False) -> dict:
    _operation_matches(env, method)
    closed(env, "", QUERY_FIELDS, QUERY_OPTIONAL + (GRANT_FIELD if allow_grant else ()))
    dotted_name(env["operation"], "/operation", "operation name")
    identifier(env["message_id"], "/message_id")
    if "grant" in env:
        identifier(env["grant"], "/grant")
    requires_and_extensions(env)
    if not isinstance(env["payload"], dict):
        raise Invalid("/payload", "must be an object")
    return env


def command_envelope(env: dict, method: str, allow_grant: bool = False) -> dict:
    _operation_matches(env, method)
    closed(env, "", COMMAND_FIELDS, COMMAND_OPTIONAL + (GRANT_FIELD if allow_grant else ()))
    dotted_name(env["operation"], "/operation", "operation name")
    identifier(env["message_id"], "/message_id")
    identifier(env["command_id"], "/command_id")
    integer(env["dedupe_generation"], "/dedupe_generation")
    subject(env["subject"], "/subject")
    pre = array(env["preconditions"], "/preconditions", max_items=16)
    for idx, entry in enumerate(pre):
        p = ptr("/preconditions", idx)
        closed(entry, p, ("subject", "revision"))
        subject(entry["subject"], ptr(p, "subject"))
        integer(entry["revision"], ptr(p, "revision"))
    # CORE 7 (resolution of D-PRE-DUP): two entries naming the same subject
    # are invalid_envelope.
    seen_subjects = set()
    for idx, entry in enumerate(pre):
        key = (entry["subject"]["kind"], entry["subject"]["id"])
        if key in seen_subjects:
            raise Invalid(ptr(ptr("/preconditions", idx), "subject"), "subject named by more than one precondition")
        seen_subjects.add(key)
    if "authority_epoch" in env:
        integer(env["authority_epoch"], "/authority_epoch")
    requires_and_extensions(env)
    if "correlation" in env:
        if not isinstance(env["correlation"], dict) or len(env["correlation"]) > 64:
            raise Invalid("/correlation", "must be an object with at most 64 members")
    if "caused_by" in env:
        refs = array(env["caused_by"], "/caused_by", max_items=64)
        for idx, ref in enumerate(refs):
            identifier(ref, ptr("/caused_by", idx))
    if "grant" in env:
        identifier(env["grant"], "/grant")
    digest_string(env["command_digest"], "/command_digest")
    if not isinstance(env["payload"], dict):
        raise Invalid("/payload", "must be an object")
    return env


# ------------------------------------------------------------- operations

def describe_params(env: dict, method: str, allow_grant: bool = False) -> None:
    query_envelope(env, method, allow_grant)
    closed(env["payload"], "/payload", ())


def negotiate_params(env: dict, method: str, allow_grant: bool = False) -> None:
    query_envelope(env, method, allow_grant)
    payload = closed(env["payload"], "/payload", ("caller", "receive_limits", "profiles"))
    caller = closed(payload["caller"], "/payload/caller", ("name", "version"))
    string(caller["name"], "/payload/caller/name", 1, 128)
    string(caller["version"], "/payload/caller/version", 1, 64)
    rl = closed(payload["receive_limits"], "/payload/receive_limits", ("max_frame_bytes",))
    integer(rl["max_frame_bytes"], "/payload/receive_limits/max_frame_bytes", 1048576)
    profiles = array(payload["profiles"], "/payload/profiles", 1, 64)
    seen = set()
    for idx, prof in enumerate(profiles):
        p = ptr("/payload/profiles", idx)
        closed(prof, p, ("name", "majors", "required", "required_features", "optional_features"))
        pattern(prof["name"], ptr(p, "name"), PROFILE_NAME, "profile name")
        majors = array(prof["majors"], ptr(p, "majors"), 1, unique=True)
        for j, major in enumerate(majors):
            integer(major, ptr(ptr(p, "majors"), j), 1, 65535)
        if not isinstance(prof["required"], bool):
            raise Invalid(ptr(p, "required"), "must be a boolean")
        for field in ("required_features", "optional_features"):
            feats = array(prof[field], ptr(p, field), max_items=64, unique=True)
            for j, feat in enumerate(feats):
                dotted_name(feat, ptr(ptr(p, field), j), "feature name")
        # Not in the schema: the same profile listed twice has no single
        # meaning (see DIVERGENCES.md, D-NEG-DUP).
        if prof["name"] in seen:
            raise Invalid(ptr(p, "name"), "profile listed more than once")
        seen.add(prof["name"])


def authenticate_params(env: dict, method: str, allow_grant: bool = False) -> None:
    # schemas/core/1/core.authenticate.params.schema.json: any 1-512 code point
    # string. A credential that does not match the ccred1 format is not an
    # invalid envelope; CORE 18.2 makes it authentication_failed.
    query_envelope(env, method, allow_grant)
    payload = closed(env["payload"], "/payload", ("credential",))
    string(payload["credential"], "/payload/credential", 1, 512)


def claim_params(env: dict, method: str, allow_grant: bool = False) -> None:
    command_envelope(env, method, allow_grant)
    closed(env["payload"], "/payload", ())
    subject(env["subject"], "/subject", "core-test.authority", "core-test")
    if len(env["preconditions"]) != 1:
        raise Invalid("/preconditions", "exactly one precondition is required")
    if "authority_epoch" in env:
        raise Invalid("/authority_epoch", "core-test.authority.claim takes no authority_epoch")
    # CORE 13: "Precondition on the authority subject's revision".
    if env["preconditions"][0]["subject"] != AUTHORITY_SUBJECT:
        raise Invalid("/preconditions/0/subject", "the precondition must name the authority subject")


def put_params(env: dict, method: str, allow_grant: bool = False) -> None:
    command_envelope(env, method, allow_grant)
    payload = closed(env["payload"], "/payload", ("value",), ("labels",))
    string(payload["value"], "/payload/value")
    if "labels" in payload:
        labels = payload["labels"]
        if not isinstance(labels, dict) or len(labels) > 32:
            raise Invalid("/payload/labels", "must be an object with at most 32 members")
        for k, v in labels.items():
            string(v, ptr("/payload/labels", k))
    subject(env["subject"], "/subject", TEST_SUBJECT_KIND)
    identifier(env["subject"]["id"], "/subject/id")
    if not env["preconditions"]:
        raise Invalid("/preconditions", "at least one precondition is required")
    if "authority_epoch" not in env:
        raise Invalid("/authority_epoch", "required for core-test.subject.put")
    # CORE 13: "The primary subject's precondition entry is mandatory".
    if not any(e["subject"] == env["subject"] for e in env["preconditions"]):
        raise Invalid("/preconditions", "a precondition on the primary subject is required")


def subject_query_params(env: dict, method: str, allow_grant: bool = False) -> None:
    query_envelope(env, method, allow_grant)
    payload = closed(env["payload"], "/payload", ("subject",))
    subject(payload["subject"], "/payload/subject", TEST_SUBJECT_KIND)
    identifier(payload["subject"]["id"], "/payload/subject/id")


# ------------------------------------------------------ CORE 15 grants

INSTANT = re.compile(r"[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]Z")
ID_PREFIX = re.compile(r"[A-Za-z0-9._:~-]{1,128}")
GRANT_KIND = "core.grant"


def instant(v, path: str) -> str:
    return pattern(v, path, INSTANT, "instant YYYY-MM-DDTHH:MM:SSZ")


def resource(v, path: str) -> dict:
    closed(v, path, ("kind",), ("id", "id_prefix"))
    dotted_name(v["kind"], ptr(path, "kind"), "subject kind")
    if "id" in v and "id_prefix" in v:
        raise Invalid(path, "a resource names at most one of id and id_prefix")
    if "id" in v:
        identifier(v["id"], ptr(path, "id"))
    if "id_prefix" in v:
        pattern(v["id_prefix"], ptr(path, "id_prefix"), ID_PREFIX, "id prefix")
    return v


def grant_terms(v, path: str) -> dict:
    closed(v, path, ("holder", "audience", "rights", "resources", "delegation"),
           ("expires_at", "authority_binding", "parent"))
    identifier(v["holder"], ptr(path, "holder"))
    identifier(v["audience"], ptr(path, "audience"))
    rights = array(v["rights"], ptr(path, "rights"), 1, 32, unique=True)
    for idx, right in enumerate(rights):
        dotted_name(right, ptr(ptr(path, "rights"), idx), "right name")
    resources = array(v["resources"], ptr(path, "resources"), 1, 32)
    for idx, res in enumerate(resources):
        resource(res, ptr(ptr(path, "resources"), idx))
    if "expires_at" in v:
        instant(v["expires_at"], ptr(path, "expires_at"))
    if "authority_binding" in v:
        b = closed(v["authority_binding"], ptr(path, "authority_binding"), ("scope", "epoch"))
        identifier(b["scope"], ptr(ptr(path, "authority_binding"), "scope"))
        integer(b["epoch"], ptr(ptr(path, "authority_binding"), "epoch"))
    d = closed(v["delegation"], ptr(path, "delegation"), ("allowed", "max_depth"))
    if not isinstance(d["allowed"], bool):
        raise Invalid(ptr(ptr(path, "delegation"), "allowed"), "must be a boolean")
    integer(d["max_depth"], ptr(ptr(path, "delegation"), "max_depth"), 0, 16)
    if "parent" in v:
        identifier(v["parent"], ptr(path, "parent"))
    return v


def _single_primary_precondition(env: dict) -> dict:
    if len(env["preconditions"]) != 1:
        raise Invalid("/preconditions", "exactly one precondition is required")
    if env["preconditions"][0]["subject"] != env["subject"]:
        raise Invalid("/preconditions/0/subject", "the precondition must name the grant")
    return env["preconditions"][0]


def grant_issue_params(env: dict, method: str, allow_grant: bool = False) -> None:
    command_envelope(env, method, allow_grant)
    subject(env["subject"], "/subject", GRANT_KIND)
    # CORE 15.3: "Subject ... with precondition revision 0" (E-GRANT-PRE).
    if _single_primary_precondition(env)["revision"] != 0:
        raise Invalid("/preconditions/0/revision", "core.grant.issue requires precondition revision 0")
    grant_terms(env["payload"], "/payload")


def grant_revoke_params(env: dict, method: str, allow_grant: bool = False) -> None:
    command_envelope(env, method, allow_grant)
    subject(env["subject"], "/subject", GRANT_KIND)
    closed(env["payload"], "/payload", ())
    # CORE 15.3: "with a precondition on its current revision"; an existing
    # grant's revision is never 0 (E-GRANT-PRE).
    if _single_primary_precondition(env)["revision"] == 0:
        raise Invalid("/preconditions/0/revision", "core.grant.revoke needs the grant's current revision")


def grant_get_params(env: dict, method: str, allow_grant: bool = False) -> None:
    query_envelope(env, method, allow_grant)
    payload = closed(env["payload"], "/payload", ("grant",))
    identifier(payload["grant"], "/payload/grant")


# ------------------------------------------------------ CORE 16 events

def _stream_position_payload(env: dict, with_limit: bool) -> dict:
    required = ("limit",) if with_limit else ()
    payload = closed(env["payload"], "/payload", required, ("cursor", "from", "kinds"))
    if ("cursor" in payload) == ("from" in payload):
        raise Invalid("/payload", "exactly one of cursor and from is required")
    if "cursor" in payload:
        string(payload["cursor"], "/payload/cursor", 1, 512)
    if "from" in payload and payload["from"] not in ("start", "now"):
        raise Invalid("/payload/from", "must be start or now")
    if "kinds" in payload:
        kinds = array(payload["kinds"], "/payload/kinds", 1, 32, unique=True)
        for idx, kind in enumerate(kinds):
            dotted_name(kind, ptr("/payload/kinds", idx), "subject kind")
    if with_limit:
        integer(payload["limit"], "/payload/limit", 1, 1000)
    return payload


def events_read_params(env: dict, method: str, allow_grant: bool = False) -> None:
    query_envelope(env, method, allow_grant)
    # CORE 16.4 and the schema: `limit` is required.
    _stream_position_payload(env, with_limit=True)


def events_subscribe_params(env: dict, method: str, allow_grant: bool = False) -> None:
    query_envelope(env, method, allow_grant)
    _stream_position_payload(env, with_limit=False)


def events_unsubscribe_params(env: dict, method: str, allow_grant: bool = False) -> None:
    query_envelope(env, method, allow_grant)
    payload = closed(env["payload"], "/payload", ("subscription",))
    identifier(payload["subscription"], "/payload/subscription")


# ------------------------------------------------- CORE 17 capabilities

def capabilities_params(env: dict, method: str, allow_grant: bool = False) -> None:
    query_envelope(env, method, allow_grant)
    closed(env["payload"], "/payload", ())
