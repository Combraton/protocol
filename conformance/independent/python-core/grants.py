"""Grant coverage and delegation bounds (CORE 15). Pure functions."""

from __future__ import annotations


def covers(resource: dict, subject: dict) -> bool:
    """A grant resource covers a subject: same kind, and the id matches when
    the resource is narrowed by ``id`` or ``id_prefix``."""
    if resource["kind"] != subject["kind"]:
        return False
    if "id" in resource:
        return subject["id"] == resource["id"]
    if "id_prefix" in resource:
        return subject["id"].startswith(resource["id_prefix"])
    return True


def resource_within(child: dict, parent: dict) -> bool:
    """Every subject the child resource covers is covered by the parent."""
    if child["kind"] != parent["kind"]:
        return False
    if "id" in parent:
        return child.get("id") == parent["id"]
    if "id_prefix" in parent:
        if "id" in child:
            return child["id"].startswith(parent["id_prefix"])
        if "id_prefix" in child:
            return child["id_prefix"].startswith(parent["id_prefix"])
        return False
    return True


def grant_covers(record: dict, subject: dict) -> bool:
    return any(covers(r, subject) for r in record["resources"])


def usable_problem(record: dict, now: str, epoch_of) -> str | None:
    """Why a grant cannot authorize right now, in the order CORE 15.5 lists
    the reasons, or None."""
    if record["state"] != "active":
        return "revoked"
    if "expires_at" in record and now >= record["expires_at"]:
        # Instants share one fixed-width UTC format, so string order is time
        # order. At exactly expires_at the grant is expired, matching the
        # issuing rule "expires_at not after the provider's current time".
        return "expired"
    binding = record.get("authority_binding")
    if binding is not None and epoch_of(binding["scope"]) != binding["epoch"]:
        return "authority_epoch_stale"
    return None


def delegation_exceeded(child: dict, parent: dict) -> bool:
    """CORE 15.3: the child MUST NOT exceed its parent."""
    pd, cd = parent["delegation"], child["delegation"]
    if not pd["allowed"] or pd["max_depth"] < 1:
        return True
    if not set(child["rights"]) <= set(parent["rights"]):
        return True
    if not all(any(resource_within(c, p) for p in parent["resources"]) for c in child["resources"]):
        return True
    if "expires_at" in parent and ("expires_at" not in child or child["expires_at"] > parent["expires_at"]):
        return True
    if cd["max_depth"] > pd["max_depth"] - 1:
        return True
    if "authority_binding" in parent and child.get("authority_binding") != parent["authority_binding"]:
        return True
    return False
