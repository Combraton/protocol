#!/usr/bin/env python3
"""minimal-publisher: a one-shot independent Evidence publisher (client only).

For each configured artifact: digest the decoded bytes, prepare with that
digest and size, append in chunks, seal, fetch the sealed bytes back and check
their digest. Exit 0 on success, 1 when the work failed.

Written from docs/spec (CORE, STREAM, ENCODING, EVIDENCE), schemas/** and
conformance/thirdparty/README.md only.
"""

import argparse
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(HERE), "common"))

import combraton_client as cc  # noqa: E402

CALLER = "thirdparty-minimal-publisher"
MUTANTS = {"uploads-bytes-differing-from-digest"}
CONFIG_FORMAT = "combraton-thirdparty-publisher-config/1"
RESULT_FORMAT = "combraton-thirdparty-publisher-result/1"

log = cc.Log("publisher")


class Failure(Exception):
    pass


def load_config(path):
    try:
        with open(path, "rb") as f:
            cfg = cc.strict_loads(f.read())
    except (OSError, cc.DecodeError) as e:
        raise Failure("cannot read configuration: %s" % e.__class__.__name__)
    if not isinstance(cfg, dict) or cfg.get("format") != CONFIG_FORMAT:
        raise Failure("configuration format is not %s" % CONFIG_FORMAT)
    for key in ("principal", "clock_file", "providers", "evidence", "artifacts"):
        if key not in cfg:
            raise Failure("configuration lacks %s" % key)
    providers = cfg["providers"]
    if not isinstance(providers, dict):
        raise Failure("providers is not an object")
    for pid, entry in providers.items():
        if not isinstance(entry, dict) or not isinstance(entry.get("socket"), str) \
                or not isinstance(entry.get("credential"), str):
            raise Failure("provider %s needs socket and credential" % pid)
        log.add_secret(entry["credential"])
    ev = cfg["evidence"]
    if not isinstance(ev, dict) or ev.get("provider") not in providers or not isinstance(ev.get("grant"), str):
        raise Failure("evidence needs a known provider and a grant")
    if not isinstance(cfg["artifacts"], list):
        raise Failure("artifacts is not a list")
    for a in cfg["artifacts"]:
        if not isinstance(a, dict) or not isinstance(a.get("id"), str) \
                or not cc.IDENTIFIER_RE.match(a["id"]) \
                or not isinstance(a.get("media_type"), str) \
                or not isinstance(a.get("content_base64"), str):
            raise Failure("artifact entry needs id, media_type and content_base64")
        cb = a.get("chunk_bytes")
        if cb is not None and (not isinstance(cb, int) or cb <= 0):
            raise Failure("artifact %s: chunk_bytes must be a positive integer" % a["id"])
    return cfg


def read_now(clock, attempts=200, delay=0.025):
    for _ in range(attempts):
        now = clock.now()
        if now is not None:
            return now
        time.sleep(delay)
    raise Failure("clock file unreadable")


def alter_one_byte(data):
    if not data:
        return data
    altered = bytearray(data)
    altered[-1] ^= 0x01
    return bytes(altered)


def run(cfg, mutant):
    clock = cc.ClockFile(cfg["clock_file"])
    pid = cfg["evidence"]["provider"]
    grant = cfg["evidence"]["grant"]
    entry = cfg["providers"][pid]
    session = cc.Session(log, "evidence@%s" % pid, entry["socket"], entry["credential"],
                         {"evidence": {"major": 1}}, CALLER)
    results = []
    try:
        session.connect()
        for a in cfg["artifacts"]:
            aid = a["id"]
            try:
                content = cc.b64decode_strict(a["content_base64"])
            except ValueError:
                raise Failure("artifact %s: content_base64 is not valid base64" % aid)
            digest = cc.digest_of_bytes(content)
            appended = content
            if mutant == "uploads-bytes-differing-from-digest":
                appended = alter_one_byte(content)
            now = read_now(clock)
            fields = {
                "media_type": a["media_type"],
                "producer": {"principal": session.principal},
                "source": {"kind": "thirdparty.publisher", "id": aid},
                "scope": "thirdparty",
                "capture": {"captured_at": cc.format_instant(now)},
                "coverage": {"completeness": "complete"},
                "retention_class": "standard",
            }
            log("publishing %s (%d bytes, %s)" % (aid, len(content), digest))
            try:
                seal = cc.publish_artifact(session, grant, aid, content, fields, "tp-publisher",
                                           chunk_bytes=a.get("chunk_bytes"),
                                           append_bytes=appended, log=log)
            except cc.ProtocolError as e:
                raise Failure("artifact %s refused: %s at %s" % (aid, e.code, e.operation))
            outcome = seal.get("outcome") or {}
            if outcome.get("state") != "sealed" or outcome.get("digest") != digest \
                    or outcome.get("size") != len(content):
                raise Failure("artifact %s: seal outcome does not match the declared content" % aid)

            # Fetch back and hash exactly the bytes received.
            try:
                fetched = cc.fetch_exact(session, grant, aid, digest)
            except cc.ProtocolError as e:
                raise Failure("artifact %s: fetch refused: %s" % (aid, e.code))
            except (cc.FetchError, ValueError) as e:
                raise Failure("artifact %s: fetch failed: %s" % (aid, e))
            if not cc.digest_matches_bytes(digest, fetched):
                raise Failure("artifact %s: fetched bytes do not match the computed digest" % aid)
            log("verified %s" % aid)
            results.append({"id": aid, "digest": digest, "size": len(content)})
    finally:
        session.close()
    return results


def main(argv):
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", required=True)
    parser.add_argument("--mutant")
    args = parser.parse_args(argv)
    if args.mutant is not None and args.mutant not in MUTANTS:
        log("unknown mutant")
        return 1
    try:
        cfg = load_config(args.config)
        results = run(cfg, args.mutant)
    except Failure as e:
        log("failed: %s" % e)
        return 1
    except (cc.TransportError, cc.NegotiationError) as e:
        log("failed: %s" % e)
        return 1
    except cc.ProtocolError as e:
        log("failed: %s refused with %s" % (e.operation, e.code))
        return 1
    result_file = cfg.get("result_file")
    if isinstance(result_file, str):
        doc = {"format": RESULT_FORMAT, "artifacts": results}
        tmp = result_file + ".tmp"
        with open(tmp, "wb") as f:
            f.write(cc.canonical_bytes(doc))
        os.replace(tmp, result_file)
    log("published and verified %d artifacts" % len(results))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
